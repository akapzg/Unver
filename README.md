# Unver — Internal Developer Documentation

[中文文档](README.zh.internal.md)

> Lightweight reverse proxy management panel. Source repository (closed-source).

---

## Project Structure

```
Unver/
├── backend/                    # Rust backend (Axum + Tokio + SQLite)
│   ├── src/
│   │   ├── main.rs             # Entry point: Web panel, proxy engine, DDNS, SSL worker
│   │   ├── proxy.rs            # HTTP/HTTPS reverse proxy core
│   │   │                       #   - TLS termination + SNI dynamic matching
│   │   │                       #   - HTTP/2 negotiation (h2 preface detection)
│   │   │                       #   - TCP tunneling (TLS SNI + plain TCP for SSH)
│   │   │                       #   - HSTS response header
│   │   │                       #   - Connection-level force_https (301 redirect)
│   │   │                       #   - Hop-by-hop header cleanup
│   │   ├── ssl.rs              # ACME DNS-01 certificate issuance (Let's Encrypt)
│   │   │                       #   - DNS-01 TXT record management (Cloudflare / Aliyun)
│   │   │                       #   - key_pem AES-256-GCM encrypted storage
│   │   │                       #   - Certificate PEM parsing / validation
│   │   ├── ssl_worker.rs       # SSL issuance dedicated tokio task
│   │   ├── ddns/
│   │   │   ├── mod.rs          # DDNS manager (scheduled sync, IP detection)
│   │   │   └── providers/
│   │       ├── cloudflare.rs  # Cloudflare DNS Provider
│   │       └── aliyun.rs       # Alibaba Cloud DNS Provider
│   │                          #   - Zone ID auto-detection / validation
│   │                          #   - HMAC-SHA1 signature auth
│   │   │                          #   - A/AAAA record upsert
│   │   │                          #   - Delete with CF record cleanup
│   │   ├── api/                # REST API
│   │   │   ├── mod.rs          # Route registration, CORS, JWT middleware
│   │   │   ├── auth.rs         # Login/refresh/logout
│   │   │   ├── settings.rs     # System settings, import/export, log queries
│   │   │   ├── proxies.rs      # Proxy rule CRUD
│   │   │   └── port_groups.rs  # Port group CRUD
│   │   ├── security.rs         # Password hashing, JWT issue/verify, API key
│   │   ├── middleware.rs       # JWT Bearer auth middleware
│   │   ├── logger.rs           # Categorized logging (DB persistence)
│   │   ├── network.rs          # System monitoring (CPU/MEM/NET)
│   │   ├── config.rs           # Config file parsing
│   │   ├── state.rs            # Global state (AppState)
│   │   ├── models.rs           # Data structures
│   │   └── errors.rs           # Error types
│   ├── migrations/
│   │   └── 001_init.sql        # Initial database schema
│   └── Cargo.toml
├── frontend/                   # React frontend (Vite + Zustand + Axios)
│   ├── src/
│   │   ├── views/              # Page components
│   │   │   ├── Dashboard.jsx   # Dashboard (real-time traffic/categorized logs/clock)
│   │   │   ├── Proxies.jsx     # Proxy rule management
│   │   │   ├── Ssl.jsx         # SSL certificate management
│   │   │   ├── Ddns.jsx        # DDNS configuration
│   │   │   └── Settings.jsx    # System settings
│   │   ├── components/         # Shared components (Layout, Toast, CollapsibleCard)
│   │   ├── store/              # Zustand state management
│   │   ├── api/                # Axios API client
│   │   └── i18n.js             # CN/EN translations
│   └── vite.config.js
├── docker-compose.yml          # Dev environment (local build)
├── docker-compose.prod.yml     # Production environment (pull image)
├── Dockerfile                  # Multi-arch build (amd64/arm64/armv7)
├── .github/workflows/
│   └── release.yml             # Release auto-build + publish to public repo
├── scripts/
│   ├── install.sh              # Linux one-click install script
│   └── install-openwrt.sh      # OpenWrt one-click install script
├── docs/
│   ├── API.md                  # API docs (English)
│   └── API.zh.md               # API docs (Chinese)
├── LICENSE
└── README.md                   # This file
```

---

## Tech Stack

| Layer | Technology |
|---|---|
| Backend | Rust 2021, Axum 0.7, Tokio, SQLx 0.7, hyper 1.x, rustls |
| Frontend | React 18, Vite, Zustand, Axios, Lucide Icons |
| Database | SQLite (WAL mode) |
| Container | Docker, Buildx (multi-arch) |
| CI/CD | GitHub Actions |

---

## Encryption & Security

### Private Key Storage

SSL certificate private keys (`key_pem`) are stored in the database encrypted with AES-256-GCM.

- **Key derivation**: 32-byte AES key derived from JWT Secret via SHA-256
- **Encryption**: Random 12-byte nonce, ciphertext prepended with nonce, then base64-encoded
- **Format**: `base64(nonce || ciphertext)`
- **Backward compatibility**: If no JWT Secret is configured (first install), stored in plaintext. Automatically upgraded to encrypted after password is set.

### Password Storage

Admin passwords use argon2id hashing (`salt = 16 bytes, hash_len = 32`). Never stored in plaintext.

### JWT

- HS256 signing
- 24-hour expiration
- JWT Secret stored in the database settings table

### API Key

- 32-byte random generation, full key returned only once at creation
- Database stores SHA-256 hash — cannot be reversed
- Supports multiple keys with independent revocation

---

## Development Environment

### Prerequisites

- Rust 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Node.js 18+
- SQLite 3

### Backend

```bash
cd backend
# First run: initialize database
DATABASE_URL="sqlite:../data/unver.db" sqlite3 ../data/unver.db < migrations/001_init.sql

# Dev run
DATABASE_URL="sqlite:../data/unver.db" cargo run

# Tests
DATABASE_URL="sqlite:../data/unver.db" cargo test

# Check (no run)
DATABASE_URL="sqlite:../data/unver.db" cargo check
```

### Frontend

```bash
cd frontend
npm install
npm run dev        # Dev server (API proxy to backend 19688)
npm run build      # Production build → dist/
```

### Docker Local Build

```bash
# Single arch
docker compose build && docker compose up -d

# Multi-arch (requires buildx)
docker buildx create --use --name multiarch
docker buildx build --platform linux/amd64,linux/arm64,linux/arm/v7 -t unver:latest .
```

---

## Build & Release

### Manual Build

```bash
# Backend
cd backend
DATABASE_URL="sqlite:../data/unver.db" cargo build --release

# Frontend
cd frontend
npm install && npm run build
ln -sf ../frontend/dist backend/static

# Run
cd backend
DATABASE_URL="sqlite:../data/unver.db" ./target/release/unver
```

### Release Process

1. Push code to private repo `main` branch
2. Tag: `git tag v1.0.0 && git push origin v1.0.0`
3. GitHub Actions automatically:
   - Builds multi-arch Docker image → `ghcr.io/akapzg/unver:1.0.0`
   - Compiles `x86_64` / `arm64` binaries
   - Creates release on public repo `akapzg/Unver` with binaries attached

### Prerequisites

In private repo Settings → Secrets:

| Secret | Description |
|---|---|
| `PUBLIC_REPO_TOKEN` | GitHub PAT with `repo` scope, used to create releases on the public repo |

---

## Configuration

`data/config.toml`:

```toml
[server]
port = 19688          # Web management panel port
host = "0.0.0.0"

[database]
url = "sqlite:data/unver.db"

[proxy]
default_port = 8443
```

---

## License

Proprietary license. See [LICENSE](LICENSE).

No decompilation, modification, unauthorized redistribution, or resale.
