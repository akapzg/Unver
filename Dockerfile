# syntax=docker/dockerfile:1
# ── Multi-arch build: amd64 / arm64 / arm/v7 (OpenWRT) ─────────────────────

# --- Frontend Build (runs on build host arch) ---
FROM --platform=$BUILDPLATFORM node:20-alpine AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm install
COPY frontend/ ./
RUN npm run build

# --- Backend Build (cross-compile to target arch) ---
FROM --platform=$BUILDPLATFORM rust:1.88-slim AS backend-builder
WORKDIR /app/backend

ARG TARGETPLATFORM
RUN case "${TARGETPLATFORM}" in \
      "linux/amd64")   RUST_TARGET="x86_64-unknown-linux-gnu" ;; \
      "linux/arm64")   RUST_TARGET="aarch64-unknown-linux-gnu" ;; \
      "linux/arm/v7")  RUST_TARGET="armv7-unknown-linux-gnueabihf" ;; \
      *)               echo "Unsupported platform: ${TARGETPLATFORM}" && exit 1 ;; \
    esac && \
    rustup target add "${RUST_TARGET}" && \
    echo "RUST_TARGET=${RUST_TARGET}" > /tmp/target.env

# Install target-specific cross-compilation deps
RUN case "${TARGETPLATFORM}" in \
      "linux/arm64") \
        apt-get update && apt-get install -y gcc-aarch64-linux-gnu libssl-dev:arm64 2>/dev/null || \
        apt-get update && apt-get install -y crossbuild-essential-arm64 libssl-dev 2>/dev/null || true ;; \
      "linux/arm/v7") \
        dpkg --add-architecture armhf && apt-get update && \
        apt-get install -y gcc-arm-linux-gnueabihf libssl-dev:armhf 2>/dev/null || \
        apt-get update && apt-get install -y crossbuild-essential-armhf libssl-dev 2>/dev/null || true ;; \
    esac

# Install build deps
RUN apt-get update && apt-get install -y pkg-config libssl-dev sqlite3 libsqlite3-dev && rm -rf /var/lib/apt/lists/*

COPY backend/Cargo.toml ./
COPY backend/migrations ./migrations

# Cache dependencies with a dummy build
ENV DATABASE_URL=sqlite:///tmp/unver-build.db
RUN sqlite3 /tmp/unver-build.db < migrations/001_init.sql
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && \
    . /tmp/target.env && \
    cargo build --release --target "${RUST_TARGET}" && rm -rf src

# Copy real source and force recompilation
COPY backend/src ./src
RUN . /tmp/target.env && \
    rm -rf target/${RUST_TARGET}/release/.fingerprint/unver-* && \
    touch src/main.rs && \
    cargo build --release --target "${RUST_TARGET}" && \
    cp target/${RUST_TARGET}/release/unver /tmp/unver-bin

# --- Runtime Stage (target arch) ---
FROM --platform=$TARGETPLATFORM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y openssl ca-certificates sqlite3 libsqlite3-0 && rm -rf /var/lib/apt/lists/*

# Copy binary and frontend artifacts
COPY --from=backend-builder /tmp/unver-bin /app/unver
COPY --from=frontend-builder /app/frontend/dist /app/dist

RUN mkdir -p /app/data

EXPOSE 80 443 19688
CMD ["/app/unver"]
