#!/bin/sh
# ── Unver One-Click Install — OpenWrt ──────────────────────────────────────
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/akapzg/Unver/main/scripts/install-openwrt.sh | sh
#
# Environment overrides:
#   VERSION=1.0.0   Install a specific version (default: latest)
#
set -e

VERSION="${VERSION:-latest}"
INSTALL_DIR="/usr/bin"
DATA_DIR="/etc/unver/data"
CONFIG_DIR="/etc/unver"

log()  { echo "[Unver] $1"; }
err()  { echo "[Unver] ERROR: $1" >&2; exit 1; }

# ── Cleanup on exit ────────────────────────────────────────────────────────
TMPDIR="/tmp/unver_install_$$"
cleanup() { rm -rf "$TMPDIR"; }
trap cleanup EXIT INT TERM
mkdir -p "$TMPDIR"

# ── Detect architecture ────────────────────────────────────────────────────
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  ARCH_NAME="amd64" ;;
    aarch64) ARCH_NAME="arm64" ;;
    *) err "Unsupported architecture: $ARCH. Supported: x86_64, aarch64." ;;
esac
log "Architecture: $ARCH ($ARCH_NAME)"

# ── Validate VERSION format ────────────────────────────────────────────────
case "$VERSION" in
    latest) : ;;
    [0-9]*.[0-9]*.[0-9]*) : ;;
    *) err "Invalid VERSION: '$VERSION'. Use 'latest' or semver (e.g. 1.0.0)" ;;
esac

# ── Build download URL ─────────────────────────────────────────────────────
if [ "$VERSION" = "latest" ]; then
    URL="https://github.com/akapzg/Unver/releases/latest/download/unver-openwrt-${ARCH_NAME}.tar.gz"
else
    URL="https://github.com/akapzg/Unver/releases/download/v${VERSION}/unver-openwrt-${ARCH_NAME}.tar.gz"
fi

# ── Download ───────────────────────────────────────────────────────────────
log "Downloading: $URL"
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$URL" -o "$TMPDIR/unver.tar.gz" \
        || err "Download failed. Check network connection or VERSION."
elif command -v wget >/dev/null 2>&1; then
    wget -q "$URL" -O "$TMPDIR/unver.tar.gz" \
        || err "Download failed. Check network connection or VERSION."
else
    err "Neither curl nor wget found. Run: opkg update && opkg install curl"
fi

# ── Extract and validate ───────────────────────────────────────────────────
tar -xzf "$TMPDIR/unver.tar.gz" -C "$TMPDIR" || err "Failed to extract archive"
[ -f "$TMPDIR/unver" ] || err "Binary 'unver' not found in archive"

# ── Install binary ─────────────────────────────────────────────────────────
# /usr/bin is SquashFS underneath, but overlayfs transparently redirects
# writes to /overlay/upper/usr/bin — the same mechanism opkg uses.
chmod +x "$TMPDIR/unver"
mv "$TMPDIR/unver" "$INSTALL_DIR/unver" \
    || err "Failed to install to $INSTALL_DIR. Overlay may be full (check: df /overlay)"
log "Binary installed: $INSTALL_DIR/unver"

# ── Install static files ────────────────────────────────────────────────────
if [ -d "$TMPDIR/static" ]; then
    rm -rf "$INSTALL_DIR/static"
    mv "$TMPDIR/static" "$INSTALL_DIR/static" \
        || err "Failed to install static files to $INSTALL_DIR"
    log "Static files installed: $INSTALL_DIR/static/"
else
    log "Note: No static/ directory in archive (frontend served from container)"
fi

# ── Create directories ─────────────────────────────────────────────────────
mkdir -p "$DATA_DIR" "$CONFIG_DIR" || err "Failed to create directories"

# ── procd init script ──────────────────────────────────────────────────────
log "Installing procd service..."
cat > /etc/init.d/unver <<INITEOF
#!/bin/sh /etc/rc.common

START=90
STOP=10
USE_PROCD=1

PROG=/usr/bin/unver
DATA_DIR=/etc/unver/data

start_service() {
    procd_open_instance
    procd_set_param command "\$PROG" "start"
    procd_set_param env DATABASE_URL="sqlite:\$DATA_DIR/unver.db"
    procd_set_param env RUST_LOG="unver=info,tower_http=warn"
    procd_set_param limits nofile="1024 1024"
    procd_set_param respawn 3600 5 0
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

log ""
log "Install complete!"
log ""
log "  Start :  /etc/init.d/unver start"
log "  Stop  :  /etc/init.d/unver stop"
log "  Status:  /etc/init.d/unver status"
log "  Logs  :  logread | grep unver"
log "  UI    :  http://<router-ip>:19688"
log ""
log "  Database: $DATA_DIR/unver.db"
