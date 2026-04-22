mod input;

use anyhow::{Context, Result};
use callmor_shared::protocol::{Role, SignalMessage};
use futures_util::{SinkExt, StreamExt};
use gstreamer::glib;
use gstreamer::prelude::*;
use gstreamer_sdp as gst_sdp;
use gstreamer_webrtc as gst_webrtc;
use input::{InputEvent, InputInjector};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

/// Messages from GStreamer thread → WebSocket send loop
#[derive(Debug)]
enum OutgoingSignal {
    Offer { sdp: String },
    IceCandidate { candidate: String, sdp_mline_index: u32 },
}

/// Load config from /etc/callmor-agent/agent.conf if it exists.
/// Format: KEY=VALUE lines (like a .env file). Env vars take precedence.
fn load_config_file() {
    let config_path = std::path::Path::new("/etc/callmor-agent/agent.conf");
    if !config_path.exists() {
        return;
    }
    if let Ok(contents) = std::fs::read_to_string(config_path) {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');
                // Only set if not already in env (env vars take precedence)
                if std::env::var(key).is_err() {
                    // SAFETY: called before any threads are spawned (in main, before tokio runtime)
                    unsafe { std::env::set_var(key, value) };
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load config: /etc/callmor-agent/agent.conf → .env → env vars
    load_config_file();
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let relay_url = std::env::var("RELAY_URL").unwrap_or_else(|_| "ws://127.0.0.1:8080".into());
    let api_url = std::env::var("API_URL").unwrap_or_else(|_| "https://api.callmor.ai".into());
    let machine_id = std::env::var("MACHINE_ID").unwrap_or_else(|_| "agent-linux-1".into());
    let agent_token = std::env::var("AGENT_TOKEN").unwrap_or_default();

    info!("Callmor Agent v{}", env!("CARGO_PKG_VERSION"));
    info!("Relay: {relay_url}, API: {api_url}, Machine ID: {machine_id}");

    if agent_token.is_empty() || agent_token == "CHANGE_ME" {
        error!("AGENT_TOKEN is not configured. Set it in /etc/callmor-agent/agent.conf or env.");
        std::process::exit(1);
    }

    gstreamer::init()?;

    // Spawn heartbeat task
    {
        let api_url = api_url.clone();
        let token = agent_token.clone();
        let mid = machine_id.clone();
        tokio::spawn(async move {
            heartbeat_loop(api_url, token, mid).await;
        });
    }

    loop {
        info!("Connecting to relay...");
        match run_session(&relay_url, &machine_id, &agent_token).await {
            Ok(()) => info!("Session ended cleanly"),
            Err(e) => error!("Session error: {e:#}"),
        }
        info!("Reconnecting in 3 seconds...");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

async fn heartbeat_loop(api_url: String, agent_token: String, machine_id: String) {
    let client = reqwest::Client::new();
    let hostname = hostname_info();
    let os = "linux";

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
    loop {
        interval.tick().await;

        let result = client
            .post(format!("{api_url}/agent/heartbeat"))
            .header("X-Agent-Token", &agent_token)
            .json(&serde_json::json!({
                "machine_id": machine_id,
                "hostname": hostname,
                "os": os,
            }))
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().is_success() => {
                tracing::debug!("Heartbeat OK");
            }
            Ok(resp) => {
                warn!("Heartbeat failed: HTTP {}", resp.status());
            }
            Err(e) => {
                warn!("Heartbeat error: {e}");
            }
        }
    }
}

fn hostname_info() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

async fn run_session(relay_url: &str, machine_id: &str, agent_token: &str) -> Result<()> {
    let (ws_stream, _) = tokio_tungstenite::connect_async(relay_url)
        .await
        .context("Failed to connect to relay")?;

    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Send Hello with token
    let hello = SignalMessage::Hello {
        role: Role::Agent,
        machine_id: machine_id.to_string(),
        token: Some(agent_token.to_string()),
    };
    ws_tx
        .send(Message::Text(serde_json::to_string(&hello)?.into()))
        .await?;
    info!("Connected to relay as agent for machine '{machine_id}'");

    // Channel for GStreamer → WebSocket outgoing signals
    let (signal_tx, mut signal_rx) = mpsc::unbounded_channel::<OutgoingSignal>();

    // Wait for browser to send a "ready" signal, then create pipeline and send offer
    info!("Waiting for browser...");

    let pipeline: Arc<tokio::sync::Mutex<Option<gstreamer::Pipeline>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let pipeline_clone = pipeline.clone();

    loop {
        tokio::select! {
            Some(signal) = signal_rx.recv() => {
                let payload = match signal {
                    OutgoingSignal::Offer { sdp } => {
                        serde_json::json!({ "signal": "offer", "sdp": sdp })
                    }
                    OutgoingSignal::IceCandidate { candidate, sdp_mline_index } => {
                        serde_json::json!({
                            "signal": "ice-candidate",
                            "candidate": {
                                "candidate": candidate,
                                "sdpMLineIndex": sdp_mline_index
                            }
                        })
                    }
                };
                let msg = SignalMessage::Relay { payload };
                ws_tx.send(Message::Text(serde_json::to_string(&msg)?.into())).await?;
            }

            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str = text.to_string();
                        match serde_json::from_str::<SignalMessage>(&text_str) {
                            Ok(SignalMessage::Relay { payload }) => {
                                if let Some(signal_type) = payload.get("signal").and_then(|s| s.as_str()) {
                                    match signal_type {
                                        "ready" => {
                                            info!("Browser is ready, creating pipeline and offer...");
                                            // Clean up old pipeline if exists
                                            if let Some(old) = pipeline_clone.lock().await.take() {
                                                old.set_state(gstreamer::State::Null)?;
                                            }
                                            let pipe = create_pipeline(signal_tx.clone())?;
                                            *pipeline_clone.lock().await = Some(pipe);
                                        }
                                        "answer" => {
                                            let sdp = payload["sdp"].as_str()
                                                .context("answer missing sdp")?;
                                            info!("Received SDP answer ({} bytes)", sdp.len());

                                            if let Some(pipe) = pipeline_clone.lock().await.as_ref() {
                                                let webrtcbin = pipe.by_name("webrtcbin")
                                                    .context("webrtcbin not found")?;

                                                let sdp_msg = gst_sdp::SDPMessage::parse_buffer(sdp.as_bytes())
                                                    .map_err(|_| anyhow::anyhow!("Failed to parse SDP answer"))?;
                                                let answer = gst_webrtc::WebRTCSessionDescription::new(
                                                    gst_webrtc::WebRTCSDPType::Answer,
                                                    sdp_msg,
                                                );
                                                webrtcbin.emit_by_name::<()>(
                                                    "set-remote-description",
                                                    &[&answer, &None::<gstreamer::Promise>],
                                                );
                                                info!("Remote description (answer) set on webrtcbin");
                                            }
                                        }
                                        "ice-candidate" => {
                                            let candidate_obj = &payload["candidate"];
                                            let candidate_str = candidate_obj["candidate"]
                                                .as_str().unwrap_or("");
                                            let sdp_mline_index = candidate_obj["sdpMLineIndex"]
                                                .as_u64().unwrap_or(0) as u32;

                                            if let Some(pipe) = pipeline_clone.lock().await.as_ref() {
                                                let webrtcbin = pipe.by_name("webrtcbin")
                                                    .context("webrtcbin not found")?;
                                                webrtcbin.emit_by_name::<()>(
                                                    "add-ice-candidate",
                                                    &[&sdp_mline_index, &candidate_str],
                                                );
                                            }
                                        }
                                        _ => warn!("Unknown signal type: {signal_type}"),
                                    }
                                }
                            }
                            Ok(SignalMessage::Error { message }) => {
                                warn!("Relay error: {message}");
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!("Relay connection closed");
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

    // Cleanup pipeline
    if let Some(pipe) = pipeline.lock().await.take() {
        pipe.set_state(gstreamer::State::Null)?;
    }

    Ok(())
}

fn create_pipeline(
    signal_tx: mpsc::UnboundedSender<OutgoingSignal>,
) -> Result<gstreamer::Pipeline> {
    let pipeline_str = r#"
        ximagesrc use-damage=false show-pointer=true
        ! videoconvert
        ! video/x-raw,framerate=30/1
        ! x264enc
            tune=zerolatency
            bitrate=2000
            speed-preset=ultrafast
            key-int-max=60
            bframes=0
            byte-stream=true
        ! video/x-h264,profile=constrained-baseline,stream-format=byte-stream
        ! rtph264pay config-interval=-1 pt=96
        ! application/x-rtp,media=video,encoding-name=H264,payload=96
        ! webrtcbin name=webrtcbin bundle-policy=max-bundle stun-server=stun://stun.l.google.com:19302
    "#;

    let pipeline = gstreamer::parse::launch(pipeline_str)?
        .downcast::<gstreamer::Pipeline>()
        .map_err(|_| anyhow::anyhow!("Failed to downcast to Pipeline"))?;

    let webrtcbin = pipeline
        .by_name("webrtcbin")
        .context("webrtcbin element not found")?;

    // Handle ICE candidates
    let tx_ice = signal_tx.clone();
    webrtcbin.connect("on-ice-candidate", false, move |args| {
        let sdp_mline_index = args[1].get::<u32>().unwrap();
        let candidate = args[2].get::<String>().unwrap();
        let _ = tx_ice.send(OutgoingSignal::IceCandidate {
            candidate,
            sdp_mline_index,
        });
        None
    });

    // When negotiation is needed, create and send offer
    let tx_offer = signal_tx.clone();
    let webrtcbin_weak = webrtcbin.downgrade();
    webrtcbin.connect("on-negotiation-needed", false, move |_args| {
        let Some(webrtcbin) = webrtcbin_weak.upgrade() else { return None };

        info!("Negotiation needed, creating offer...");

        let tx = tx_offer.clone();
        let wb_weak = webrtcbin.downgrade();

        let promise = gstreamer::Promise::with_change_func(move |reply| {
            let Some(wb) = wb_weak.upgrade() else { return };

            let reply = match reply {
                Ok(Some(reply)) => reply,
                _ => {
                    error!("Failed to create offer");
                    return;
                }
            };

            let offer = match reply.value("offer") {
                Ok(offer) => offer
                    .get::<gst_webrtc::WebRTCSessionDescription>()
                    .unwrap(),
                Err(e) => {
                    error!("Failed to get offer: {e:?}");
                    return;
                }
            };

            let sdp_text = offer.sdp().to_string();
            info!("Created SDP offer ({} bytes)", sdp_text.len());

            wb.emit_by_name::<()>(
                "set-local-description",
                &[&offer, &None::<gstreamer::Promise>],
            );

            let _ = tx.send(OutgoingSignal::Offer { sdp: sdp_text });
        });

        webrtcbin.emit_by_name::<()>(
            "create-offer",
            &[&None::<gstreamer::Structure>, &promise],
        );

        None
    });

    // Start pipeline — this triggers on-negotiation-needed which creates the offer
    pipeline.set_state(gstreamer::State::Playing)?;
    info!("GStreamer pipeline started: ximagesrc → x264enc → webrtcbin");

    // Create data channel for input AFTER pipeline is playing.
    // This triggers a second on-negotiation-needed which is fine — the offer
    // will include the data channel. The browser receives it via ondatachannel.
    let dc_init = gstreamer::Structure::builder("application/x-datachannel")
        .field("ordered", true)
        .build();
    let input_dc: gst_webrtc::WebRTCDataChannel = webrtcbin
        .emit_by_name_with_values(
            "create-data-channel",
            &["input".into(), dc_init.to_value()],
        )
        .context("create-data-channel returned None")?
        .get::<gst_webrtc::WebRTCDataChannel>()
        .context("Failed to get WebRTCDataChannel from return value")?;
    info!("Created 'input' data channel");

    // Set up input injection on the data channel
    let injector = Arc::new(InputInjector::new()?);

    let inj = injector.clone();
    input_dc.connect("on-message-string", false, move |args| {
        let msg = args[1].get::<String>().expect("message must be string");
        match serde_json::from_str::<InputEvent>(&msg) {
            Ok(event) => {
                if let Err(e) = inj.handle_event(&event) {
                    warn!("Input injection error: {e}");
                }
            }
            Err(e) => {
                warn!("Failed to parse input event: {e}");
            }
        }
        None
    });

    let (w, h) = injector.screen_size();
    let dc_ref = input_dc.clone();
    input_dc.connect("on-open", false, move |_args| {
        let size_msg = serde_json::json!({"type":"screen-size","width":w,"height":h}).to_string();
        dc_ref.send_string(Some(&size_msg));
        info!("Input data channel open, sent screen size {w}x{h}");
        None
    });

    // Monitor bus
    let bus = pipeline.bus().context("No bus on pipeline")?;
    std::thread::spawn(move || {
        for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
            match msg.view() {
                gstreamer::MessageView::Error(err) => {
                    error!(
                        "GStreamer error from {:?}: {} ({:?})",
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    break;
                }
                gstreamer::MessageView::Warning(warn) => {
                    warn!("GStreamer warning: {} ({:?})", warn.error(), warn.debug());
                }
                gstreamer::MessageView::Eos(..) => {
                    info!("GStreamer: End of stream");
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(pipeline)
}
