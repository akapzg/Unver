# Unver

[中文文档](README.zh.md)

> A minimalist, high-performance reverse proxy panel designed for home / personal use.

---

## ✨ Features

- ✅ **Auto SSL** — Let's Encrypt ACME DNS-01, auto issuance & renewal, SNI dynamic matching
- ✅ **Cross-Platform** — x86_64, ARM64, ARMv7, runs on OpenWrt routers
- ✅ **Modern Web UI** — React dark theme, CN/EN i18n, real-time traffic, categorized logs
- ✅ **Docker One-Click** — single command deploy, persistent data, auto-restart
- ✅ **DDNS Auto-Sync** — Cloudflare A/AAAA records, auto-update on IP change
- ✅ **HTTP → HTTPS** — connection-level 301 redirect + HSTS, zero plaintext leak
- ✅ **HTTP/2** — auto h2 negotiation on TLS connections
- ✅ **Rust-Powered** — sub-10 MB memory footprint, single static binary, no GC pauses

---

## 🚀 Quick Start

### Docker (Recommended)

```bash
mkdir unver && cd unver
curl -fsSLO https://raw.githubusercontent.com/akapzg/Unver/main/docker-compose.prod.yml
docker compose up -d
```

Open `http://<your-ip>:19688` to access the management panel.

### Binary Install

**Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/akapzg/Unver/main/scripts/install.sh | bash
```

**OpenWrt:**
```bash
curl -fsSL https://raw.githubusercontent.com/akapzg/Unver/main/scripts/install-openwrt.sh | sh
```

---

## 🖥 CLI Commands

```bash
unver         # Interactive menu
unver serve   # Start the service
unver version # Show version
unver update  # Self-update to latest release
unver restart # Restart the service
unver status  # Check if running
```

---

## 📖 Documentation

- [API Documentation](docs/API.md)
- [中文 API 文档](docs/API.zh.md)

---

## 📄 License

Proprietary license. No decompilation, modification, or unauthorized redistribution.

See [LICENSE](LICENSE).

## 🔗 Third-Party Licenses

- [instant-acme](https://github.com/instant-acme/instant-acme) — Apache 2.0
