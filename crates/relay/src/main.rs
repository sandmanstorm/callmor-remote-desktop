use anyhow::Result;
use callmor_shared::protocol::{Role, SignalMessage};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// A handle to send messages to a connected client.
type ClientTx = mpsc::UnboundedSender<String>;

/// One room = one machine. Has at most one agent and zero or more browsers.
struct Room {
    agent: Option<ClientTx>,
    browsers: HashMap<u64, ClientTx>,
}

impl Room {
    fn new() -> Self {
        Self {
            agent: None,
            browsers: HashMap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.agent.is_none() && self.browsers.is_empty()
    }
}

/// Global state: machine_id → Room
type Rooms = Arc<RwLock<HashMap<String, Room>>>;

/// Monotonically increasing ID for browser connections.
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let port: u16 = std::env::var("RELAY_PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()?;

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    let rooms: Rooms = Arc::new(RwLock::new(HashMap::new()));

    info!("Callmor Relay listening on {addr}");

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let rooms = rooms.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, peer, rooms).await {
                        warn!("Connection {peer} error: {e}");
                    }
                });
            }
            Err(e) => warn!("Accept error: {e}"),
        }
    }
}

async fn handle_connection(stream: TcpStream, peer: SocketAddr, rooms: Rooms) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    info!("{peer}: WebSocket connected, waiting for Hello");

    // Wait for the first message to be a Hello.
    let (role, machine_id) = loop {
        match ws_rx.next().await {
            Some(Ok(Message::Text(text))) => {
                let text_str = text.to_string();
                match serde_json::from_str::<SignalMessage>(&text_str) {
                    Ok(SignalMessage::Hello { role, machine_id }) => {
                        break (role, machine_id);
                    }
                    _ => {
                        let err = SignalMessage::Error {
                            message: "First message must be a Hello".into(),
                        };
                        let msg = serde_json::to_string(&err)?;
                        ws_tx.send(Message::Text(msg.into())).await?;
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None => return Ok(()),
            Some(Ok(_)) => {} // ignore ping/pong/binary
            Some(Err(e)) => return Err(e.into()),
        }
    };

    info!("{peer}: Hello as {role:?} for machine {machine_id}");

    // Create a channel for outbound messages to this client.
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let conn_id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    // Register in the room.
    {
        let mut rooms_guard = rooms.write().await;
        let room = rooms_guard.entry(machine_id.clone()).or_insert_with(Room::new);
        match role {
            Role::Agent => {
                if room.agent.is_some() {
                    info!("{peer}: Replacing existing agent for machine {machine_id}");
                }
                room.agent = Some(tx.clone());
            }
            Role::Browser => {
                room.browsers.insert(conn_id, tx.clone());
            }
        }
    }

    // Spawn a task to forward outbound channel messages to the WebSocket.
    let (done_tx, mut done_rx) = tokio::sync::oneshot::channel::<()>();
    let send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(text) => {
                            if ws_tx.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break, // channel closed
                    }
                }
                _ = &mut done_rx => break,
            }
        }
    });

    // Read loop: forward incoming messages to the peer(s).
    loop {
        match ws_rx.next().await {
            Some(Ok(Message::Text(text))) => {
                let text_str = text.to_string();
                match serde_json::from_str::<SignalMessage>(&text_str) {
                    Ok(SignalMessage::Relay { .. }) => {
                        let rooms_guard = rooms.read().await;
                        if let Some(room) = rooms_guard.get(&machine_id) {
                            match role {
                                Role::Agent => {
                                    // Agent → all browsers
                                    for browser_tx in room.browsers.values() {
                                        let _ = browser_tx.send(text_str.clone());
                                    }
                                }
                                Role::Browser => {
                                    // Browser → agent
                                    if let Some(agent_tx) = &room.agent {
                                        let _ = agent_tx.send(text_str.clone());
                                    }
                                }
                            }
                        }
                    }
                    Ok(SignalMessage::Hello { .. }) => {
                        let err = SignalMessage::Error {
                            message: "Already registered, Hello sent twice".into(),
                        };
                        let _ = tx.send(serde_json::to_string(&err)?);
                    }
                    Ok(SignalMessage::Error { .. }) => {} // ignore client errors
                    Err(e) => {
                        warn!("{peer}: Invalid message: {e}");
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None => break,
            Some(Ok(_)) => {} // ping/pong/binary handled by tungstenite
            Some(Err(e)) => {
                warn!("{peer}: WebSocket error: {e}");
                break;
            }
        }
    }

    info!("{peer}: Disconnected ({role:?} for machine {machine_id})");

    // Cleanup: remove from room.
    {
        let mut rooms_guard = rooms.write().await;
        if let Some(room) = rooms_guard.get_mut(&machine_id) {
            match role {
                Role::Agent => {
                    room.agent = None;
                }
                Role::Browser => {
                    room.browsers.remove(&conn_id);
                }
            }
            if room.is_empty() {
                rooms_guard.remove(&machine_id);
                info!("Room {machine_id} removed (empty)");
            }
        }
    }

    // Signal the send task to stop.
    let _ = done_tx.send(());
    let _ = send_task.await;

    Ok(())
}
