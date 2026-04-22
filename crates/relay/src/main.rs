mod auth;

use anyhow::Result;
use callmor_shared::protocol::{Role, SignalMessage};
use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::DecodingKey;
use sqlx::PgPool;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

type ClientTx = mpsc::UnboundedSender<String>;

struct Room {
    /// Agent stored with its conn_id so disconnect only removes the current one.
    agent: Option<(u64, ClientTx)>,
    browsers: HashMap<u64, ClientTx>,
}

impl Room {
    fn new() -> Self {
        Self { agent: None, browsers: HashMap::new() }
    }
    fn is_empty(&self) -> bool {
        self.agent.is_none() && self.browsers.is_empty()
    }
}

type Rooms = Arc<RwLock<HashMap<String, Room>>>;

#[derive(Clone)]
struct RelayState {
    rooms: Rooms,
    pool: PgPool,
    jwt_key: Arc<DecodingKey>,
}

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

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET must be set (same value as API server)");
    if jwt_secret.len() < 32 {
        panic!("JWT_SECRET must be at least 32 characters");
    }

    let pool = callmor_shared::db::create_pool(&database_url).await?;
    info!("Relay connected to PostgreSQL");

    let state = RelayState {
        rooms: Arc::new(RwLock::new(HashMap::new())),
        pool,
        jwt_key: Arc::new(DecodingKey::from_secret(jwt_secret.as_bytes())),
    };

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    info!("Callmor Relay listening on {addr} (auth always required)");

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let state = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, peer, state).await {
                        warn!("Connection {peer} error: {e}");
                    }
                });
            }
            Err(e) => warn!("Accept error: {e}"),
        }
    }
}

async fn send_error_and_close(
    ws_tx: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<TcpStream>,
        Message,
    >,
    msg: &str,
) {
    let err = SignalMessage::Error { message: msg.into() };
    if let Ok(json) = serde_json::to_string(&err) {
        let _ = ws_tx.send(Message::Text(json.into())).await;
    }
    let _ = ws_tx.send(Message::Close(None)).await;
}

async fn handle_connection(stream: TcpStream, peer: SocketAddr, state: RelayState) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    info!("{peer}: WebSocket connected, waiting for Hello");

    // Wait for Hello
    let (role, machine_id, token) = loop {
        match ws_rx.next().await {
            Some(Ok(Message::Text(text))) => {
                let text_str = text.to_string();
                match serde_json::from_str::<SignalMessage>(&text_str) {
                    Ok(SignalMessage::Hello { role, machine_id, token }) => {
                        break (role, machine_id, token);
                    }
                    _ => {
                        send_error_and_close(&mut ws_tx, "First message must be a Hello").await;
                        return Ok(());
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None => return Ok(()),
            Some(Ok(_)) => {}
            Some(Err(e)) => return Err(e.into()),
        }
    };

    // --- Authentication (always required) ---
    let token = match token.as_deref() {
        Some(t) => t,
        None => {
            warn!("{peer}: Missing token for {role:?}");
            send_error_and_close(&mut ws_tx, "Missing auth token").await;
            return Ok(());
        }
    };

    let validation_result = match role {
        Role::Agent => auth::validate_agent_token(&state.pool, token, &machine_id).await,
        Role::Browser => {
            auth::validate_session_token(&state.jwt_key, token, &machine_id).map(|_| uuid::Uuid::nil())
        }
    };

    if let Err(e) = validation_result {
        warn!("{peer}: Auth failed for {role:?} machine={machine_id}: {e}");
        send_error_and_close(&mut ws_tx, "Authentication failed").await;
        return Ok(());
    }

    info!("{peer}: Hello as {role:?} for machine {machine_id} (authenticated)");

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let conn_id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    // Register in room
    {
        let mut rooms_guard = state.rooms.write().await;
        let room = rooms_guard.entry(machine_id.clone()).or_insert_with(Room::new);
        match role {
            Role::Agent => {
                if room.agent.is_some() {
                    info!("{peer}: Replacing existing agent for machine {machine_id}");
                }
                room.agent = Some((conn_id, tx.clone()));
            }
            Role::Browser => {
                room.browsers.insert(conn_id, tx.clone());
            }
        }
    }

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
                        None => break,
                    }
                }
                _ = &mut done_rx => break,
            }
        }
    });

    // Read loop
    loop {
        match ws_rx.next().await {
            Some(Ok(Message::Text(text))) => {
                let text_str = text.to_string();
                match serde_json::from_str::<SignalMessage>(&text_str) {
                    Ok(SignalMessage::Relay { .. }) => {
                        let rooms_guard = state.rooms.read().await;
                        if let Some(room) = rooms_guard.get(&machine_id) {
                            match role {
                                Role::Agent => {
                                    for browser_tx in room.browsers.values() {
                                        let _ = browser_tx.send(text_str.clone());
                                    }
                                }
                                Role::Browser => {
                                    if let Some((_, agent_tx)) = &room.agent {
                                        let _ = agent_tx.send(text_str.clone());
                                    }
                                }
                            }
                        }
                    }
                    Ok(SignalMessage::Hello { .. }) => {
                        let err = SignalMessage::Error {
                            message: "Already registered".into(),
                        };
                        let _ = tx.send(serde_json::to_string(&err)?);
                    }
                    Ok(SignalMessage::Error { .. }) => {}
                    Err(e) => warn!("{peer}: Invalid message: {e}"),
                }
            }
            Some(Ok(Message::Close(_))) | None => break,
            Some(Ok(_)) => {}
            Some(Err(e)) => {
                warn!("{peer}: WebSocket error: {e}");
                break;
            }
        }
    }

    info!("{peer}: Disconnected ({role:?} for machine {machine_id})");

    // Cleanup
    {
        let mut rooms_guard = state.rooms.write().await;
        if let Some(room) = rooms_guard.get_mut(&machine_id) {
            match role {
                Role::Agent => {
                    // Only clear if this IS the current agent (not a replacement).
                    // If a newer agent replaced us, leave them in place.
                    if let Some((current_id, _)) = &room.agent {
                        if *current_id == conn_id {
                            room.agent = None;
                        }
                    }
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

    let _ = done_tx.send(());
    let _ = send_task.await;

    Ok(())
}
