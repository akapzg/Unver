# Unver

> 一个专为家庭 / 个人设计的、极简且高性能的反向代理面板。

---

## ✨ 功能亮点

- ✅ **自动 SSL** — Let's Encrypt ACME DNS-01，证书自动签发/续签，SNI 动态匹配
- ✅ **跨平台** — x86_64、ARM64、ARMv7 全支持，OpenWrt 路由器也能跑
- ✅ **现代 Web 界面** — React 暗色主题、中英文切换、实时流量监控、分类日志
- ✅ **Docker 一键部署** — 一行命令拉起，数据持久化，自动重启
- ✅ **DDNS 自动同步** — Cloudflare A/AAAA 记录，IP 变化自动更新
- ✅ **HTTP → HTTPS** — 连接层 301 跳转 + HSTS，零明文泄漏
- ✅ **HTTP/2** — TLS 连接自动协商 h2

---

## 🚀 快速开始

### Docker（推荐）

```bash
mkdir unver && cd unver
curl -fsSLO https://raw.githubusercontent.com/akapzg/Unver/main/docker-compose.prod.yml
docker compose up -d
```

打开 `http://<你的IP>:19688` 进入管理面板。

### 二进制安装

**Linux：**
```bash
curl -fsSL https://raw.githubusercontent.com/akapzg/Unver/main/scripts/install.sh | bash
```

**OpenWrt：**
```bash
curl -fsSL https://raw.githubusercontent.com/akapzg/Unver/main/scripts/install-openwrt.sh | sh
```

---

## 📖 文档

- [API 文档](docs/API.md)
- [中文 API 文档](docs/API.zh.md)

---

## 📄 许可证

本软件使用专有许可证，禁止反编译、修改及未经授权的再分发。

详见 [LICENSE](LICENSE)。
