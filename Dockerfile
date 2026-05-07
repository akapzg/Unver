# syntax=docker/dockerfile:1
# ── Multi-arch build: amd64 / arm64 ─────────────────────────────────────────

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
      "linux/amd64")   RUST_TARGET="x86_64-unknown-linux-gnu" ; echo "RUST_TARGET=${RUST_TARGET}" > /tmp/target.env ;; \
      "linux/arm64")   RUST_TARGET="aarch64-unknown-linux-gnu" ; \
                       echo "RUST_TARGET=${RUST_TARGET}" > /tmp/target.env ; \
                       echo "export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc" >> /tmp/target.env ;; \
      *)               echo "Unsupported platform: ${TARGETPLATFORM}" && exit 1 ;; \
    esac && \
    rustup target add "${RUST_TARGET}"

# Install target-specific cross-compilation deps (linker only; vendored openssl handles TLS)
RUN case "${TARGETPLATFORM}" in \
      "linux/arm64") \
        apt-get update && apt-get install -y gcc-aarch64-linux-gnu ;; \
    esac

# Install build deps (perl + make for vendored OpenSSL, mold for fast x86_64 linking)
RUN apt-get update && apt-get install -y perl make pkg-config mold libssl-dev sqlite3 libsqlite3-dev && rm -rf /var/lib/apt/lists/*

COPY backend/Cargo.toml ./
COPY backend/migrations ./migrations
COPY vendor ../vendor

# Cache dependencies with a dummy build
ENV DATABASE_URL=sqlite:///tmp/unver-build.db
RUN sqlite3 /tmp/unver-build.db < migrations/001_init.sql
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && \
    . /tmp/target.env && \
    if [ "${TARGETPLATFORM}" = "linux/amd64" ]; then \
      export RUSTFLAGS="-C link-arg=-fuse-ld=mold"; \
    fi && \
    cargo build --release --target "${RUST_TARGET}" && rm -rf src

# Copy real source and force recompilation
COPY backend/src ./src
RUN . /tmp/target.env && \
    rm -rf target/${RUST_TARGET}/release/.fingerprint/unver-* && \
    touch src/main.rs && \
    if [ "${TARGETPLATFORM}" = "linux/amd64" ]; then \
      export RUSTFLAGS="-C link-arg=-fuse-ld=mold"; \
    fi && \
    cargo build --release --target "${RUST_TARGET}" && \
    cp target/${RUST_TARGET}/release/unver /tmp/unver-bin

# --- Runtime Stage (target arch) ---
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y openssl ca-certificates sqlite3 libsqlite3-0 && rm -rf /var/lib/apt/lists/*

# Copy binary and frontend artifacts
COPY --from=backend-builder /tmp/unver-bin /app/unver
COPY --from=frontend-builder /app/frontend/dist /app/dist

RUN mkdir -p /app/data

EXPOSE 80 443 19688
CMD ["/app/unver"]
