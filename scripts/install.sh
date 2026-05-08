#!/bin/sh
# ── Unver One-Click Install — Linux ────────────────────────────────────────
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/akapzg/Unver/main/scripts/install.sh | bash
#
# Or install a specific version:
#   curl -fsSL https://raw.githubusercontent.com/akapzg/Unver/main/scripts/install.sh | VERSION=1.0.0 bash
#
set -e

VERSION="${VERSION:-latest}"
INSTALL_DIR="/usr/local/bin"
DATA_DIR="/var/lib/unver"
CONFIG_DIR="/etc/unver"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

log()  { echo "${GREEN}[Unver]${NC} $1"; }
err()  { echo "${RED}[Unver]${NC} $1"; exit 1; }

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  ARCH_NAME="amd64" ;;
    aarch64) ARCH_NAME="arm64" ;;
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
log "Downloading: $URL"
curl -fsSL "$URL" -o "$TMPDIR/unver.tar.gz" || err "Download failed"

# ── Install binary ─────────────────────────────────────────────────────────
tar -xzf "$TMPDIR/unver.tar.gz" -C "$TMPDIR"
sudo mv "$TMPDIR/unver" "$INSTALL_DIR/unver"
sudo chmod +x "$INSTALL_DIR/unver"
log "Binary installed to $INSTALL_DIR/unver"

# ── Install static files ────────────────────────────────────────────────────
if [ -d "$TMPDIR/static" ]; then
    sudo rm -rf "$INSTALL_DIR/static"
    sudo mv "$TMPDIR/static" "$INSTALL_DIR/static"
    log "Static files installed to $INSTALL_DIR/static/"
fi

# ── Capability for privileged ports ────────────────────────────────────────
if command -v setcap >/dev/null 2>&1; then
    sudo setcap cap_net_bind_service=+ep "$INSTALL_DIR/unver"
    log "Granted CAP_NET_BIND_SERVICE for ports 80/443"
fi

# ── Create directories ─────────────────────────────────────────────────────
sudo mkdir -p "$DATA_DIR" "$CONFIG_DIR"
sudo chown -R "$(id -u):$(id -g)" "$DATA_DIR" "$CONFIG_DIR"

# ── systemd service ────────────────────────────────────────────────────────
if command -v systemctl >/dev/null 2>&1; then
    log "Setting up systemd service"
    sudo tee /etc/systemd/system/unver.service > /dev/null <<EOF
[Unit]
Description=Unver Reverse Proxy Manager
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=$INSTALL_DIR/unver start
Environment=DATABASE_URL=sqlite:$DATA_DIR/unver.db
Environment=RUST_LOG=unver=info,tower_http=warn
Restart=on-failure
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF
    sudo systemctl daemon-reload
    sudo systemctl enable unver
    log "Service installed. Start with: sudo systemctl start unver"
else
    log "No systemd detected. Run manually: unver start"
fi

rm -rf "$TMPDIR"
log "Install complete!"
log ""
log "  Start:   sudo systemctl start unver"
log "  Status:  sudo systemctl status unver"
log "  Logs:    journalctl -u unver -f"
log "  UI:      http://localhost:19688"
log "  Uninstall: sudo unver uninstall"
