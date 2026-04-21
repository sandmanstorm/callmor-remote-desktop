# Callmor Remote Desktop

A self-hosted, multi-tenant remote access platform (ScreenConnect-style) with WebRTC-based screen sharing, browser-based control, and a modern React dashboard.

## Architecture

```
Browser (React)  ←→  relay.callmor.ai (WebSocket signaling)  ←→  Agent (on managed machine)
       ↕                                                              ↕
  api.callmor.ai (REST API + Auth)                          Screen capture + H.264
       ↕
  PostgreSQL / Redis / MinIO
```

### Components

| Component | Technology | Port | Public URL |
|-----------|-----------|------|------------|
| Relay server | Rust + Tokio + tokio-tungstenite | 8080 | relay.callmor.ai |
| API server | Rust + Axum | 3000 | api.callmor.ai |
| Web frontend | React + TypeScript + Tailwind | 5173 | remote.callmor.ai |
| Agent | Rust (cross-platform) | — | Runs on managed machines |
| TURN server | coturn (Docker) | 3478 | turn.callmor.ai (UDP direct) |
| PostgreSQL | 16-alpine (Docker) | 5432 | localhost only |
| Redis | 7-alpine (Docker) | 6379 | localhost only |
| MinIO | latest (Docker) | 9000/9001 | localhost only |

### Network Topology

- **NPM (Nginx Proxy Manager)** on 98.189.108.123 terminates SSL and proxies HTTP/WS to 10.10.100.34
- **coturn** UDP traffic is port-forwarded directly from router (bypasses NPM)
- All infra services (Postgres, Redis, MinIO) bind to 127.0.0.1 only

## Prerequisites

- Debian (latest stable)
- Rust (installed via rustup)
- Node.js 20+
- Docker + Docker Compose

## Quick Start

### 1. Generate secrets

```bash
./scripts/generate-secrets.sh
```

This creates:
- `.env` with random passwords for Postgres, Redis, MinIO, coturn
- `keys/jwt_private.pem` and `keys/jwt_public.pem` for JWT signing

### 2. Start infrastructure

```bash
docker compose up -d
```

Starts Postgres, Redis, MinIO, and coturn.

### 3. Build Rust services

```bash
cargo build
```

### 4. Run relay server

```bash
cargo run -p callmor-relay
# Listening on 0.0.0.0:8080
```

### 5. Run API server

```bash
cargo run -p callmor-api
# Listening on 0.0.0.0:3000
# Health check: curl http://localhost:3000/health
```

### 6. Run web frontend

```bash
cd web
npm install
npm run dev
# Listening on http://localhost:5173
```

## NPM Proxy Hosts

| Domain | Forward To | WebSockets | Force SSL |
|--------|-----------|------------|-----------|
| relay.callmor.ai | 10.10.100.34:8080 | YES | YES |
| api.callmor.ai | 10.10.100.34:3000 | NO | YES |
| remote.callmor.ai | 10.10.100.34:5173 | YES | YES |

## Router Port Forwards (coturn)

| Protocol | External Port | Internal | Purpose |
|----------|--------------|----------|---------|
| UDP+TCP | 3478 | 10.10.100.34:3478 | STUN/TURN |
| UDP | 49152-49252 | 10.10.100.34:49152-49252 | TURN relay |

## Systemd Services (Production)

```bash
# Build release binaries
cargo build --release

# Install service files
sudo cp deploy/callmor-relay.service /etc/systemd/system/
sudo cp deploy/callmor-api.service /etc/systemd/system/
sudo systemctl daemon-reload

# Enable and start
sudo systemctl enable --now callmor-relay
sudo systemctl enable --now callmor-api
```

## Project Structure

```
callmor-remote-desktop/
├── Cargo.toml              # Workspace root
├── docker-compose.yml      # Postgres, Redis, MinIO, coturn
├── .env                    # Secrets (gitignored)
├── crates/
│   ├── shared/             # Shared types, protocol messages
│   ├── relay/              # WebSocket signaling server
│   ├── api/                # REST API (Axum)
│   └── agent/              # Remote agent binary
├── web/                    # React + TypeScript + Tailwind
├── migrations/             # SQL migrations (sqlx)
├── config/coturn/          # coturn configuration
├── deploy/                 # systemd unit files
├── keys/                   # JWT keypair (gitignored)
└── scripts/                # Setup and utility scripts
```

## Build Order (Milestones)

1. **Project skeleton** (current) - Repo layout, Docker Compose, Cargo workspace
2. Minimal relay - WebSocket echo between two connections
3. WebRTC signaling - P2P between two browser tabs
4. Agent v0 - Linux screen capture + H.264 + WebRTC
5. Input injection - Mouse + keyboard from browser to agent
6. Auth + Dashboard - Login, machine list, session launch
7. Agent installer - .deb package, code signing
8. Multi-tenancy - Teams, permissions, row-level isolation
9. Polish - Session recording, audit logs, UX refinement
