//! WebRTC session: connect to relay, negotiate, stream H.264, accept input.

use anyhow::{Context, Result};
use bytes::Bytes;
use callmor_agent_core::config::AgentConfig;
use callmor_agent_core::input::InputEvent;
use callmor_shared::protocol::{Role, SignalMessage};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264};
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

use crate::capture::{Capturer, Frame};
use crate::input::Injector;

/// Run a single session: connect to relay, wait for browser, stream, tear down.
pub async fn run(config: &AgentConfig) -> Result<()> {
    // 1. Connect to relay
    let (ws, _) = tokio_tungstenite::connect_async(&config.relay_url)
        .await
        .context("Connect to relay")?;
    let (mut ws_tx, mut ws_rx) = ws.split();

    // 2. Send Hello with agent token
    let hello = SignalMessage::Hello {
        role: Role::Agent,
        machine_id: config.machine_id.clone(),
        token: Some(config.agent_token.clone()),
    };
    ws_tx.send(Message::Text(serde_json::to_string(&hello)?.into())).await?;
    info!("Connected to relay as agent");

    // Channel for messages to send OUT on the WebSocket
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();

    // We create the peer connection lazily — only when browser sends "ready"
    let mut pc: Option<Arc<webrtc::peer_connection::RTCPeerConnection>> = None;
    let mut capture_handle: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        tokio::select! {
            Some(out) = out_rx.recv() => {
                ws_tx.send(Message::Text(out.into())).await?;
            }
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str = text.to_string();
                        let signal: SignalMessage = match serde_json::from_str(&text_str) {
                            Ok(s) => s,
                            Err(e) => { warn!("Bad message: {e}"); continue; }
                        };
                        match signal {
                            SignalMessage::Relay { payload } => {
                                let sig = payload.get("signal").and_then(|s| s.as_str()).unwrap_or("");
                                match sig {
                                    "ready" => {
                                        info!("Browser ready — creating peer connection");
                                        let (new_pc, handle) = setup_peer_connection(out_tx.clone()).await?;
                                        pc = Some(new_pc);
                                        capture_handle = Some(handle);
                                    }
                                    "answer" => {
                                        if let Some(pc) = &pc {
                                            let sdp = payload["sdp"].as_str().context("answer missing sdp")?;
                                            let answer = RTCSessionDescription::answer(sdp.to_string())?;
                                            pc.set_remote_description(answer).await?;
                                            info!("Remote description (answer) set");
                                        }
                                    }
                                    "ice-candidate" => {
                                        if let Some(pc) = &pc {
                                            let c = &payload["candidate"];
                                            let init = RTCIceCandidateInit {
                                                candidate: c["candidate"].as_str().unwrap_or("").to_string(),
                                                sdp_mid: c.get("sdpMid").and_then(|v| v.as_str()).map(String::from),
                                                sdp_mline_index: c.get("sdpMLineIndex").and_then(|v| v.as_u64()).map(|v| v as u16),
                                                ..Default::default()
                                            };
                                            if let Err(e) = pc.add_ice_candidate(init).await {
                                                warn!("add_ice_candidate: {e}");
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            SignalMessage::Error { message } => {
                                warn!("Relay error: {message}");
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!("Relay closed");
                        break;
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {e}");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    if let Some(h) = capture_handle { h.abort(); }
    if let Some(pc) = pc { let _ = pc.close().await; }

    Ok(())
}

/// Set up WebRTC peer connection with H.264 video + input data channel.
/// Returns the PC and a JoinHandle for the capture/encode loop.
async fn setup_peer_connection(
    out_tx: mpsc::UnboundedSender<String>,
) -> Result<(Arc<webrtc::peer_connection::RTCPeerConnection>, tokio::task::JoinHandle<()>)> {
    // Media engine with H.264 codec
    let mut media = MediaEngine::default();
    media.register_default_codecs()?;

    let api = APIBuilder::new().with_media_engine(media).build();

    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".into()],
            ..Default::default()
        }],
        ..Default::default()
    };

    let pc = Arc::new(api.new_peer_connection(config).await?);

    // Video track (H.264)
    let track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_H264.to_string(),
            ..Default::default()
        },
        "video".into(),
        "callmor".into(),
    ));

    let _ = pc
        .add_track(Arc::clone(&track) as Arc<dyn TrackLocal + Send + Sync>)
        .await?;

    // Data channel for input
    let dc_init = webrtc::data_channel::data_channel_init::RTCDataChannelInit {
        ordered: Some(true),
        ..Default::default()
    };
    let input_dc = pc.create_data_channel("input", Some(dc_init)).await?;

    // Set up input injection + permission handling
    let injector = Arc::new(Injector::new());
    let permission_view_only = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let (dc_w, dc_h) = injector.screen_size();
    let dc_for_open = input_dc.clone();
    input_dc.on_open(Box::new(move || {
        Box::pin(async move {
            let msg = serde_json::json!({"type":"screen-size","width":dc_w,"height":dc_h}).to_string();
            let _ = dc_for_open.send_text(msg).await;
            info!("Input data channel open; sent screen size {dc_w}x{dc_h}");
        })
    }));

    let inj_clone = injector.clone();
    let perm_clone = permission_view_only.clone();
    input_dc.on_message(Box::new(move |msg: DataChannelMessage| {
        let inj = inj_clone.clone();
        let perm = perm_clone.clone();
        Box::pin(async move {
            let Ok(text) = std::str::from_utf8(&msg.data) else { return };
            let Ok(val) = serde_json::from_str::<serde_json::Value>(text) else { return };

            // Check for permission message
            if val.get("type").and_then(|v| v.as_str()) == Some("permission") {
                let view_only = val.get("value").and_then(|v| v.as_str()) == Some("view_only");
                perm.store(view_only, std::sync::atomic::Ordering::Relaxed);
                info!("Permission: {}", if view_only { "view_only" } else { "full_control" });
                return;
            }

            if perm.load(std::sync::atomic::Ordering::Relaxed) {
                return; // view-only: drop input
            }

            if let Ok(event) = serde_json::from_str::<InputEvent>(text) {
                inj.handle(&event);
            }
        })
    }));

    // ICE candidate -> relay
    let tx_ice = out_tx.clone();
    pc.on_ice_candidate(Box::new(move |candidate| {
        let tx = tx_ice.clone();
        Box::pin(async move {
            if let Some(c) = candidate {
                let init = c.to_json().unwrap_or_default();
                let payload = serde_json::json!({
                    "signal": "ice-candidate",
                    "candidate": {
                        "candidate": init.candidate,
                        "sdpMid": init.sdp_mid,
                        "sdpMLineIndex": init.sdp_mline_index,
                    }
                });
                let msg = SignalMessage::Relay { payload };
                if let Ok(s) = serde_json::to_string(&msg) {
                    let _ = tx.send(s);
                }
            }
        })
    }));

    pc.on_ice_connection_state_change(Box::new(|state: RTCIceConnectionState| {
        info!("ICE state: {state}");
        Box::pin(async {})
    }));

    // Create and send offer
    let offer = pc.create_offer(None).await?;
    pc.set_local_description(offer.clone()).await?;

    let payload = serde_json::json!({"signal":"offer","sdp": offer.sdp});
    let msg = SignalMessage::Relay { payload };
    out_tx.send(serde_json::to_string(&msg)?)?;
    info!("Sent SDP offer");

    // Spawn capture + encode + send loop
    let track_for_loop = track.clone();
    let handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = capture_loop(track_for_loop) {
            error!("Capture loop exited: {e:#}");
        }
    });

    Ok((pc, handle))
}

/// Capture frames, encode to H.264, send as samples.
fn capture_loop(track: Arc<TrackLocalStaticSample>) -> Result<()> {
    use openh264::encoder::Encoder;
    use openh264::formats::{BgraSliceU8, YUVBuffer};

    let mut capturer = Capturer::new().context("Failed to init DXGI capturer")?;
    let width = capturer.width;
    let height = capturer.height;
    info!("Capturer ready: {width}x{height}");

    let mut encoder = Encoder::new().context("openh264 encoder")?;
    let mut yuv = YUVBuffer::new(width as usize, height as usize);

    let target_fps = 30u64;
    let frame_duration = std::time::Duration::from_millis(1000 / target_fps);

    loop {
        let start = std::time::Instant::now();

        match capturer.grab(100) {
            Ok(Some(frame)) => {
                // Convert BGRA -> YUV via openh264's RGBSource impl
                let bgra = BgraSliceU8::new(&frame.data[..], (width as usize, height as usize));
                yuv.read_rgb(bgra);

                let bitstream = encoder.encode(&yuv).context("encode")?;
                let mut nals: Vec<u8> = Vec::new();
                bitstream.write(&mut nals).ok();

                if !nals.is_empty() {
                    let sample = webrtc::media::Sample {
                        data: Bytes::from(nals),
                        duration: frame_duration,
                        ..Default::default()
                    };
                    let track = track.clone();
                    tokio::runtime::Handle::current().block_on(async move {
                        let _ = track.write_sample(&sample).await;
                    });
                }
            }
            Ok(None) => { /* no new frame */ }
            Err(e) => {
                warn!("Capture error: {e}");
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        let elapsed = start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }
}

/// Suppressed non-Windows unused warning.
#[allow(dead_code)]
fn _frame_used(_f: &Frame) {}
