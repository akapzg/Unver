#!/bin/sh
# ── Unver One-Click Install — OpenWrt ──────────────────────────────────────
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/akapzg/Unver/main/scripts/install-openwrt.sh | sh
#
# Or install a specific version:
#   curl -fsSL https://raw.githubusercontent.com/akapzg/Unver/main/scripts/install-openwrt.sh | VERSION=1.0.0 sh
#
set -e

VERSION="${VERSION:-latest}"
INSTALL_DIR="/usr/bin"
DATA_DIR="/var/lib/unver"
CONFIG_DIR="/etc/unver"

log() { echo "[Unver] $1"; }
err() { echo "[Unver] ERROR: $1"; exit 1; }

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  ARCH_NAME="amd64" ;;
    aarch64) ARCH_NAME="arm64" ;;
    armv7l)  ARCH_NAME="arm64" ;;  # 32-bit ARM also use arm64 binary (compatible via kernel)
    *) err "Unsupported architecture: $ARCH" ;;
esac

log "Detected: $ARCH ($ARCH_NAME)"

# ── Download ───────────────────────────────────────────────────────────────
if [ "$VERSION" = "latest" ]; then
    URL="https://github.com/akapzg/Unver/releases/latest/download/unver-linux-${ARCH_NAME}.tar.gz"
else
    URL="https://github.com/akapzg/Unver/releases/download/v${VERSION}/unver-linux-${ARCH_NAME}.tar.gz"
fi

TMPDIR=$(mktemp -d)

if command -v curl >/dev/null 2>&1; then
    log "Downloading: $URL"
    curl -fsSL "$URL" -o "$TMPDIR/unver.tar.gz" || err "Download failed"
elif command -v wget >/dev/null 2>&1; then
    log "Downloading: $URL"
    wget -q "$URL" -O "$TMPDIR/unver.tar.gz" || err "Download failed"
else
    err "Neither curl nor wget found. Install one first: opkg install curl"
fi

# ── Install binary ─────────────────────────────────────────────────────────
tar -xzf "$TMPDIR/unver.tar.gz" -C "$TMPDIR"
mv "$TMPDIR/unver" "$INSTALL_DIR/unver"
chmod +x "$INSTALL_DIR/unver"
log "Binary installed to $INSTALL_DIR/unver"

# ── Create directories ─────────────────────────────────────────────────────
mkdir -p "$DATA_DIR" "$CONFIG_DIR"

# ── procd init script ──────────────────────────────────────────────────────
log "Setting up procd service"
cat > /etc/init.d/unver <<'INITEOF'
#!/bin/sh /etc/rc.common

START=90
STOP=10
USE_PROCD=1

PROG=/usr/bin/unver
DATA_DIR=/var/lib/unver

start_service() {
    procd_open_instance
    procd_set_param command "$PROG" "serve"
    procd_set_param env DATABASE_URL="sqlite:$DATA_DIR/unver.db"
    procd_set_param env RUST_LOG="unver=info,tower_http=warn"
    procd_set_param limits nofile="65536 65536"
    procd_set_param respawn 3600 5 15
    procd_set_param stdout 1
    procd_set_param stderr 1
    procd_close_instance
}

service_triggers() {
    procd_add_reload_trigger "unver"
}
INITEOF

chmod +x /etc/init.d/unver
/etc/init.d/unver enable

rm -rf "$TMPDIR"
log "Install complete!"
log ""
log "  Start:   /etc/init.d/unver start"
log "  Stop:    /etc/init.d/unver stop"
log "  Status:  /etc/init.d/unver status"
log "  Logs:    logread | grep unver"
log "  UI:      http://<router-ip>:19688"
