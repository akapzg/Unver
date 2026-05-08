# Unver — 内部开发文档

> 轻量反向代理管理面板。源码仓库（闭源）。

---

## 项目结构

```
Unver/
├── backend/                    # Rust 后端（Axum + Tokio + SQLite）
│   ├── src/
│   │   ├── main.rs             # 入口：启动 Web 面板、代理引擎、DDNS、SSL worker
│   │   ├── proxy.rs            # HTTP/HTTPS 反向代理核心
│   │   │                       #   - TLS 终止 + SNI 动态匹配
│   │   │                       #   - HTTP/2 协商（h2 preface 探测）
│   │   │                       #   - TCP 隧道（TLS ClientHello 转发）
│   │   │                       #   - HSTS 响应头
│   │   │                       #   - 连接级 force_https（301 跳转）
│   │   │                       #   - Hop-by-hop header 清理
│   │   ├── ssl.rs              # ACME DNS-01 证书签发（Let's Encrypt）
│   │   │                       #   - Cloudflare DNS TXT 记录管理
│   │   │                       #   - key_pem AES-256-GCM 加密存储
│   │   │                       #   - 证书 PEM 解析 / 验证
│   │   ├── ssl_worker.rs       # SSL 签发专用 tokio 任务
│   │   ├── ddns/
│   │   │   ├── mod.rs          # DDNS 管理器（定时同步、IP 检测）
│   │   │   └── providers/
│   │   │       └── cloudflare.rs  # Cloudflare DNS Provider
│   │   │                          #   - Zone ID 自动探测 / 验证
│   │   │                          #   - A/AAAA 记录 upsert
│   │   │                          #   - 删除时同步清理 CF 记录
│   │   ├── api/                # REST API
│   │   │   ├── mod.rs          # 路由注册、CORS、JWT 中间件
│   │   │   ├── auth.rs         # 登录/刷新/登出
│   │   │   ├── settings.rs     # 系统设置、导入导出、日志查询
│   │   │   ├── proxies.rs      # 代理规则 CRUD
│   │   │   └── port_groups.rs  # 端口组 CRUD
│   │   ├── security.rs         # 密码哈希、JWT 签发/验证、API Key
│   │   ├── middleware.rs       # JWT Bearer 认证中间件
│   │   ├── logger.rs           # 分类日志（DB 持久化）
│   │   ├── network.rs          # 系统监控（CPU/内存/网络）
│   │   ├── config.rs           # 配置文件解析
│   │   ├── state.rs            # 全局状态（AppState）
│   │   ├── models.rs           # 数据结构定义
│   │   └── errors.rs           # 错误类型
│   ├── migrations/
│   │   └── 001_init.sql        # 初始数据库 schema
│   └── Cargo.toml
├── frontend/                   # React 前端（Vite + Zustand + Axios）
│   ├── src/
│   │   ├── views/              # 页面组件
│   │   │   ├── Dashboard.jsx   # 仪表盘（实时流量/分类日志/时钟）
│   │   │   ├── Proxies.jsx     # 代理规则管理
│   │   │   ├── Ssl.jsx         # SSL 证书管理
│   │   │   ├── Ddns.jsx        # DDNS 配置
│   │   │   └── Settings.jsx    # 系统设置
│   │   ├── components/         # 通用组件（Layout, Toast, CollapsibleCard）
│   │   ├── store/              # Zustand 状态管理
│   │   ├── api/                # Axios API 客户端
│   │   └── i18n.js             # 中/英文翻译
│   └── vite.config.js
├── static/ → frontend/dist/   # 开发符号链接；CI/Docker: 真实目录随二进制分发
├── docker-compose.yml          # 开发环境（本地 build）
├── docker-compose.prod.yml     # 生产环境（拉镜像）
├── Dockerfile                  # 多架构构建（amd64/arm64）
├── .github/workflows/
│   └── release.yml             # Release 自动构建 + 发布到公开仓库
├── scripts/
│   ├── install.sh              # Linux 一键安装脚本
│   └── install-openwrt.sh      # OpenWrt 一键安装脚本
├── docs/
│   ├── API.md                  # API 文档（英文）
│   └── API.zh.md               # API 文档（中文）
├── LICENSE
├── Dockerfile
└── README.md                   # 本文件
```

---

## 技术栈

| 层 | 技术 |
|---|---|
| 后端 | Rust 2021, Axum 0.7, Tokio, SQLx 0.7, hyper 1.x, rustls |
| 前端 | React 18, Vite, Zustand, Axios, Lucide Icons |
| 数据库 | SQLite (WAL 模式) |
| 容器 | Docker, Buildx (多架构) |
| CI/CD | GitHub Actions |

---

## 加密与安全

### 私钥存储

SSL 证书私钥（`key_pem`）使用 AES-256-GCM 加密后存入数据库。

- **密钥派生**：从 JWT Secret 通过 SHA-256 派生 32 字节 AES 密钥
- **加密**：随机 12 字节 nonce，密文前附加 nonce 后 base64 编码
- **格式**：`base64(nonce || ciphertext)`
- **向后兼容**：如果没有配置 JWT Secret（首次安装），明文存储，设置密码后自动升级为加密

### 密码存储

管理员密码使用 argon2id 哈希（`salt = 16 bytes, hash_len = 32`），不存储明文。

### JWT

- HS256 签名
- 24 小时过期
- JWT Secret 存储在数据库 settings 表中

### API Key

- 32 字节随机生成，仅创建时返回一次完整 Key
- 数据库存储 SHA-256 哈希，无法逆向
- 支持多 Key，可独立删除

---

## 开发环境

### 前置依赖

- Rust 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Node.js 18+
- SQLite 3

### 后端

```bash
cd backend
# 首次运行：初始化数据库
DATABASE_URL="sqlite:../data/unver.db" sqlite3 ../data/unver.db < migrations/001_init.sql

# 开发运行
DATABASE_URL="sqlite:../data/unver.db" cargo run

# 测试
DATABASE_URL="sqlite:../data/unver.db" cargo test

# 编译检查（不运行）
DATABASE_URL="sqlite:../data/unver.db" cargo check
```

### 前端

```bash
cd frontend
npm install
npm run dev        # 开发服务器（API 代理到后端 19688）
npm run build      # 生产构建 → dist/
```

### Docker 本地构建

```bash
# 单架构
docker compose build && docker compose up -d

# 多架构（需要 buildx）
docker buildx create --use --name multiarch
docker buildx build --platform linux/amd64,linux/arm64 -t unver:latest .
```

---

## 构建与发布

### 手动构建

```bash
# 后端
cd backend
DATABASE_URL="sqlite:../data/unver.db" cargo build --release

# 前端
cd frontend
npm install && npm run build

# 运行（二进制根据自身位置自动定位 static/）
cd backend
DATABASE_URL="sqlite:../data/unver.db" ./target/release/unver start
```

### Release 流程

1. 推送代码到私有仓库 `main` 分支
2. 打 tag：`git tag v1.0.0 && git push origin v1.0.0`
3. GitHub Actions 自动：
   - 构建多架构 Docker 镜像 → `ghcr.io/akapzg/unver:1.0.0`
   - 编译 `x86_64` / `arm64` 二进制
   - 在公开仓库 `akapzg/Unver` 创建 Release 并上传二进制

### 前置准备

在私有仓库 Settings → Secrets 中配置：

| Secret | 说明 |
|---|---|
| `PUBLIC_REPO_TOKEN` | 有 `repo` 权限的 GitHub PAT，用于在公开仓库创建 release |

---

## 配置

`data/config.toml`：

```toml
[server]
port = 19688          # Web 管理面板端口
host = "0.0.0.0"

[database]
url = "sqlite:data/unver.db"

[proxy]
default_port = 8443
```

### 路径解析

`static_dir` 和 `data_dir` 基于**二进制自身位置**解析（不再依赖 CWD）。
即 `/usr/bin/unver` 会在 `/usr/bin/static/` 寻找前端文件，数据路径由 `$DATABASE_URL`
决定，无论 systemd/procd 将工作目录设在哪里都不会出错。

Docker 中二进制位于 `/app/unver`，前端文件位于 `/app/static/`。

---

## 许可证

专有许可证。详见 [LICENSE](LICENSE)。

禁止反编译、修改、未经授权的再分发或售卖。
