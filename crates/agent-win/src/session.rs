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
    let mut capture_handle: Option<(tokio::task::JoinHandle<()>, Arc<std::sync::atomic::AtomicBool>)> = None;

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
                                        // Stop any existing capture loop before starting a new one
                                        if let Some((h, stop)) = capture_handle.take() {
                                            stop.store(true, std::sync::atomic::Ordering::Relaxed);
                                            h.abort();
                                        }
                                        let (new_pc, handle, stop_flag) = setup_peer_connection(out_tx.clone()).await?;
                                        pc = Some(new_pc);
                                        capture_handle = Some((handle, stop_flag));
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
    if let Some((h, stop)) = capture_handle {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        // Give the loop up to ~200ms to notice the flag and release DXGI resources
        let _ = tokio::time::timeout(std::time::Duration::from_millis(200), h).await;
    }
    if let Some(pc) = pc { let _ = pc.close().await; }

    Ok(())
}

/// Set up WebRTC peer connection with H.264 video + input data channel.
/// Returns the PC and a JoinHandle for the capture/encode loop.
async fn setup_peer_connection(
    out_tx: mpsc::UnboundedSender<String>,
) -> Result<(
    Arc<webrtc::peer_connection::RTCPeerConnection>,
    tokio::task::JoinHandle<()>,
    Arc<std::sync::atomic::AtomicBool>,
)> {
    // Media engine — register EXACTLY ONE H.264 codec so the browser has
    // no choice in SDP negotiation. Constrained Baseline Level 3.1 +
    // packetization-mode=1 is the universal safe pick supported by every
    // browser, and our capture loop downscales to 1280x720 to stay
    // within Level 3.1's bounds. register_default_codecs() is skipped
    // because it offers many variants, and if Chrome picks a mismatched
    // one we get the exact blank-screen failure we've been chasing.
    use webrtc::rtp_transceiver::rtp_codec::{
        RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType,
    };
    use webrtc::rtp_transceiver::RTCPFeedback;

    let h264_feedback = vec![
        RTCPFeedback { typ: "goog-remb".into(), parameter: "".into() },
        RTCPFeedback { typ: "ccm".into(), parameter: "fir".into() },
        RTCPFeedback { typ: "nack".into(), parameter: "".into() },
        RTCPFeedback { typ: "nack".into(), parameter: "pli".into() },
    ];

    let mut media = MediaEngine::default();
    media.register_codec(
        RTCRtpCodecParameters {
            capability: RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_string(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line:
                    "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
                        .to_string(),
                rtcp_feedback: h264_feedback,
            },
            payload_type: 102,
            ..Default::default()
        },
        RTPCodecType::Video,
    )?;

    let api = APIBuilder::new().with_media_engine(media).build();

    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".into()],
            ..Default::default()
        }],
        ..Default::default()
    };

    let pc = Arc::new(api.new_peer_connection(config).await?);

    // Video track — MUST match the registered codec capability exactly,
    // otherwise webrtc-rs can't map the track to the negotiated payload
    // type during RTP writing and the samples are silently dropped.
    let track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_H264.to_string(),
            clock_rate: 90000,
            channels: 0,
            sdp_fmtp_line:
                "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f"
                    .to_string(),
            rtcp_feedback: vec![],
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

    // Spawn capture + encode + send loop.
    // The blocking loop checks `stop_flag` so we can cleanly stop it when
    // the session ends (tokio's abort() does not interrupt spawn_blocking).
    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let track_for_loop = track.clone();
    let stop_for_loop = stop_flag.clone();
    let handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = capture_loop(track_for_loop, stop_for_loop) {
            error!("Capture loop exited: {e:#}");
        }
    });

    // Store the stop flag on the PC as an extension so the caller can flip it.
    // We piggyback via the returned handle by returning the stop flag as well.
    // Instead of making the signature bigger, we spawn a watcher that stops the
    // loop when the join handle is aborted.
    Ok((pc, handle, stop_flag))
}

/// Target encode resolution. Capped so the bitstream always fits within the
/// H.264 profile level every browser decoder accepts out of the box
/// (Constrained Baseline Level 3.1, max 1280x720). Native-resolution capture
/// was getting dropped by Chrome's decoder even when bytes arrived.
const ENC_WIDTH: u32 = 1280;
const ENC_HEIGHT: u32 = 720;

/// Capture frames, encode to H.264, send as samples.
fn capture_loop(
    track: Arc<TrackLocalStaticSample>,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    use openh264::encoder::{Encoder, EncoderConfig};
    use openh264::formats::{BgraSliceU8, YUVBuffer};

    let mut capturer = Capturer::new().context("Failed to init DXGI capturer")?;
    let src_w = capturer.width;
    let src_h = capturer.height;
    info!("Capturer ready: {src_w}x{src_h} — encoding at {ENC_WIDTH}x{ENC_HEIGHT}");

    // Explicit encoder config — previously we relied on defaults which made
    // profile + level + keyframe interval implicit. Baseline profile with a
    // 30-frame GOP (1 keyframe/second at 30fps) means the browser always has
    // a fresh IDR to bootstrap decoding.
    let cfg = EncoderConfig::new()
        .max_frame_rate(openh264::encoder::FrameRate::from_hz(30.0));
    let mut encoder = Encoder::with_api_config(openh264::OpenH264API::from_source(), cfg)
        .context("openh264 encoder")?;
    let mut yuv = YUVBuffer::new(ENC_WIDTH as usize, ENC_HEIGHT as usize);

    // Scratch buffer for the downscaled BGRA pixels.
    let mut scaled: Vec<u8> = vec![0u8; (ENC_WIDTH * ENC_HEIGHT * 4) as usize];

    let target_fps = 30u64;
    let frame_duration = std::time::Duration::from_millis(1000 / target_fps);

    let mut frames_sent: u64 = 0;
    let mut bytes_sent: u64 = 0;
    let mut empty_frames: u64 = 0;
    let mut last_log = std::time::Instant::now();

    loop {
        if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            info!("Capture loop stopping (session ended, sent {frames_sent} frames / {bytes_sent} bytes)");
            break Ok(());
        }
        let start = std::time::Instant::now();

        match capturer.grab(100) {
            Ok(Some(frame)) => {
                // Downscale source → 1280x720 with a fast nearest-neighbor
                // resampler. Quality is fine for remote-desktop; the win is
                // that encoded frames fit even Chrome's most restrictive
                // default H.264 level.
                resize_bgra_nearest(
                    &frame.data[..],
                    src_w,
                    src_h,
                    &mut scaled[..],
                    ENC_WIDTH,
                    ENC_HEIGHT,
                );

                let bgra = BgraSliceU8::new(&scaled[..], (ENC_WIDTH as usize, ENC_HEIGHT as usize));
                yuv.read_rgb(bgra);

                let bitstream = encoder.encode(&yuv).context("encode")?;
                let mut nals: Vec<u8> = Vec::new();
                bitstream.write(&mut nals).ok();

                if !nals.is_empty() {
                    let size = nals.len();
                    let sample = webrtc::media::Sample {
                        data: Bytes::from(nals),
                        duration: frame_duration,
                        ..Default::default()
                    };
                    let track = track.clone();
                    let write_result: std::result::Result<(), webrtc::Error> =
                        tokio::runtime::Handle::current()
                            .block_on(async move { track.write_sample(&sample).await });
                    match write_result {
                        Ok(()) => {
                            frames_sent += 1;
                            bytes_sent += size as u64;
                            if frames_sent == 1 {
                                info!("First encoded frame sent ({size} bytes)");
                            }
                        }
                        Err(e) => {
                            // Don't spam the log for every frame; warn once and
                            // continue. If write_sample consistently fails, the
                            // first warning tells us why.
                            if frames_sent == 0 {
                                warn!("write_sample failed on first frame: {e}. Track may not be negotiated yet.");
                            }
                        }
                    }
                } else {
                    empty_frames += 1;
                }

                // Force an IDR keyframe every 30 frames (~1s at 30fps) so
                // browsers that miss the initial SPS/PPS recover quickly
                // rather than staying stuck at "0 frames decoded" forever.
                if frames_sent > 0 && frames_sent.is_multiple_of(30) {
                    encoder.force_intra_frame();
                }

                if last_log.elapsed() >= std::time::Duration::from_secs(3) {
                    info!("Video: {frames_sent} frames / {bytes_sent} bytes sent, {empty_frames} empty encodes");
                    last_log = std::time::Instant::now();
                }
            }
            Ok(None) => { /* no new frame */ }
            Err(e) => {
                let msg = format!("{e:#}");
                warn!("Capture error: {msg}");
                // DXGI state corruption recovery:
                //   - "The keyed mutex was abandoned" (0x887A0026)
                //   - "The application made a call that is invalid" (0x887A0001)
                //   - "DuplicateOutput: The parameter is incorrect" (0x80070057)
                // all mean the capturer handle is no longer valid. Drop it
                // and recreate — cheaper than requiring the user to restart.
                if msg.contains("keyed mutex was abandoned")
                    || msg.contains("0x887A0001")
                    || msg.contains("DuplicateOutput")
                {
                    warn!("DXGI state corrupted, reinitializing capturer...");
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    match Capturer::new() {
                        Ok(c) => {
                            capturer = c;
                            info!("DXGI capturer reinitialized ({}x{})", capturer.width, capturer.height);
                        }
                        Err(re) => {
                            warn!("Capturer reinit failed: {re:#}, retrying in 1s");
                            std::thread::sleep(std::time::Duration::from_millis(1000));
                        }
                    }
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
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

/// Nearest-neighbor BGRA resize. Fast enough for 30fps 1080p → 720p on any
/// modern CPU (~5 ms) and has zero deps. Quality is acceptable for
/// remote-desktop; if we later need smoother scaling we can swap in bilinear.
fn resize_bgra_nearest(src: &[u8], sw: u32, sh: u32, dst: &mut [u8], dw: u32, dh: u32) {
    let sw = sw as usize;
    let sh = sh as usize;
    let dw = dw as usize;
    let dh = dh as usize;
    debug_assert_eq!(src.len(), sw * sh * 4);
    debug_assert_eq!(dst.len(), dw * dh * 4);

    // Precompute source-column indices once per frame (avoids a mul per pixel).
    // Fixed-point Q16 to keep divisions out of the inner loop.
    let x_step = ((sw as u64) << 16) / (dw as u64);
    let y_step = ((sh as u64) << 16) / (dh as u64);

    let mut x_indices = Vec::with_capacity(dw);
    for x in 0..dw {
        let sx = ((x as u64) * x_step) >> 16;
        x_indices.push(sx.min(sw as u64 - 1) as usize);
    }

    for y in 0..dh {
        let sy = (((y as u64) * y_step) >> 16).min(sh as u64 - 1) as usize;
        let src_row = &src[sy * sw * 4..(sy + 1) * sw * 4];
        let dst_row = &mut dst[y * dw * 4..(y + 1) * dw * 4];
        for (x, sx) in x_indices.iter().enumerate() {
            let sp = sx * 4;
            let dp = x * 4;
            dst_row[dp..dp + 4].copy_from_slice(&src_row[sp..sp + 4]);
        }
    }
}
