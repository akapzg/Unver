# Unver

[English](README.md)

> 一个专为家庭 / 个人设计的、极简且高性能的反向代理面板。

---

## ✨ 功能亮点

- ✅ **自动 SSL** — Let's Encrypt ACME DNS-01，证书自动签发/续签，SNI 动态匹配
- ✅ **跨平台** — x86_64、ARM64 全支持，OpenWrt 路由器也能跑（musl 静态编译）
- ✅ **现代 Web 界面** — React 暗色主题、中英文切换、实时流量监控、分类日志
- ✅ **Docker 一键部署** — 一行命令拉起，数据持久化，自动重启
- ✅ **DDNS 自动同步** — 支持 Cloudflare / 阿里云，IP 变化自动更新
- ✅ **HTTP → HTTPS** — 连接层 301 跳转 + HSTS，零明文泄漏
- ✅ **TCP 代理** — 支持 SSH、MySQL 等任意 TCP 协议隧道
- ✅ **HTTP/2** — TLS 连接自动协商 h2
- ✅ **闲置保护** — 15 分钟无操作自动退出登录
- ✅ **Rust 编写** — 内存占用不到 10MB，单文件静态编译，无 GC 卡顿

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

## 🖥 命令行

```bash
unver         # 交互式菜单
unver start   # 启动服务
unver version # 显示版本
unver update  # 自动更新到最新版
unver restart # 重启服务
unver status  # 检查运行状态
unver uninstall # 一键卸载（保留数据）
```

---

## 📖 文档

- [API 文档](docs/API.md)
- [中文 API 文档](docs/API.zh.md)

---

## 📄 许可证

本软件使用专有许可证，禁止反编译、修改及未经授权的再分发。

详见 [LICENSE](LICENSE)。

## 🔗 第三方许可

- [instant-acme](https://github.com/instant-acme/instant-acme) — Apache 2.0
