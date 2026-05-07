# Unver API 文档

基地址：`http://<主机>:<端口>/api`

除 `/api/auth/*` 和 `/api/setup*` 外的所有接口均需 JWT Bearer Token：

```
Authorization: Bearer <token>
```

---

## 身份认证

### POST /api/auth/login

登录获取 JWT Token。

**请求：**
```json
{
  "username": "string",
  "password": "string"
}
```

**响应：** `200 OK`
```json
{
  "token": "string",
  "expires_in": 3600
}
```

### POST /api/auth/refresh

刷新即将过期的 JWT Token。

**响应：** `200 OK`
```json
{
  "token": "string",
  "expires_in": 3600
}
```

### POST /api/auth/logout

注销当前 JWT Token。

**响应：** `200 OK`

### POST /api/auth/change-password

修改管理员密码。

**请求：**
```json
{
  "current_password": "string",
  "new_password": "string"
}
```

**响应：** `200 OK`

---

## 系统

### GET /api/system/stats

获取仪表盘统计数据。

**响应：** `200 OK`
```json
{
  "proxy_rules": 5,
  "active_proxies": 3,
  "certificates": 1,
  "auto_renew_certs": 1,
  "version": "1.0.0",
  "cpu_percent": 12,
  "mem_percent": 48,
  "mem_used": 503316480,
  "mem_total": 1073741824,
  "disk_percent": 35,
  "uptime_seconds": 3600,
  "db_size_bytes": 20480
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `proxy_rules` | int | 代理规则总数 |
| `active_proxies` | int | 已启用的代理规则数 |
| `certificates` | int | 证书总数 |
| `auto_renew_certs` | int | 启用自动续签的证书数 |
| `version` | string | Unver 版本号 |
| `cpu_percent` | int | CPU 使用率 (0-100) |
| `mem_percent` | int | 内存使用率 (0-100) |
| `mem_used` | int | 已用内存（字节） |
| `mem_total` | int | 总内存（字节） |
| `disk_percent` | int | 磁盘使用率 (0-100) |
| `uptime_seconds` | int | 运行时长（秒） |
| `db_size_bytes` | int | 数据库文件大小（字节） |

### GET /api/system/network

实时网络流量。

**响应：** `200 OK`
```json
{
  "rx_rate": 1024,
  "tx_rate": 512,
  "total_rx": 1048576,
  "total_tx": 524288,
  "rx_rate_str": "1.0 KB/s",
  "tx_rate_str": "0.5 KB/s",
  "total_rx_str": "1.0 MB",
  "total_tx_str": "0.5 MB",
  "container_mode": false
}
```

### GET /api/system/public-ip

获取服务器公网 IP。

**响应：** `200 OK`
```json
{
  "ipv4": "1.2.3.4",
  "ipv6": "2001:db8::1"
}
```

### GET /api/system/logs

最近日志（最近 100 条）。

**响应：** `200 OK`
```json
[
  {
    "id": 1,
    "level": "INFO",
    "message": "DDNS: 已更新 A home.example.com -> 1.2.3.4",
    "created_at": "2025-01-01 12:00:00"
  }
]
```

### GET /api/system/logs/:category

按分类筛选日志。

| 分类 | 说明 |
|---|---|
| `ddns` | DDNS 同步事件 |
| `ssl` | SSL 证书事件 |
| `login` | 认证事件 |
| `proxy` | 代理访问/错误事件 |

**响应：** 格式同上，按分类过滤。

### POST /api/system/restart

重启 Unver 服务。

**请求：**
```json
{
  "confirm": true
}
```

**响应：** `200 OK` — 服务立即重启。

### POST /api/system/renew-ssl

手动触发所有 SSL 证书续签。

**响应：** `200 OK`

---

## 设置

### GET /api/settings

获取所有设置（敏感字段已脱敏）。

**响应：** `200 OK`

### PATCH /api/settings

更新设置。所有字段均为可选。

**请求：**
```json
{
  "ddns_enabled": true,
  "ddns_provider": "cloudflare",
  "ddns_cf_token": "cf-api-token",
  "ddns_cf_zone_id": "zone-id",
  "ddns_aliyun_access_key_id": "ak-id",
  "ddns_aliyun_access_key_secret": "ak-secret",
  "ddns_domain": "home.example.com",
  "ddns_interval": 300,
  "ddns_ip_source": "public"
}
```

**响应：** `200 OK`

### GET /api/settings/api-keys

列出所有 API 密钥。

**响应：** `200 OK`

### POST /api/settings/api-keys

创建新 API 密钥。

**响应：** `201 Created`

### DELETE /api/settings/api-keys/:id

删除 API 密钥。

**响应：** `200 OK`

### GET /api/system/backup

导出配置（不含敏感字段）。

**响应：** `200 OK` — JSON 文件下载。

### POST /api/system/restore

导入配置。

**请求：** `multipart/form-data`
- `file`: JSON 导出文件

**响应：** `200 OK`

---

## 端口组

### GET /api/port-groups

列出所有端口组。

**响应：** `200 OK`
```json
[
  {
    "id": "uuid",
    "name": "HTTPS",
    "listen_port": 443,
    "enabled": true,
    "skip_tls_verify": false,
    "force_https": false,
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z"
  }
]
```

### POST /api/port-groups

创建新端口组。

**请求：**
```json
{
  "name": "HTTPS",
  "listen_port": 443,
  "enabled": true,
  "skip_tls_verify": false,
  "force_https": false
}
```

**响应：** `201 Created` — 返回创建的端口组。

### PATCH /api/port-groups/:id

更新端口组。所有字段可选。

**请求：**
```json
{
  "name": "新名称",
  "enabled": false,
  "force_https": true
}
```

**响应：** `200 OK` — 返回更新后的端口组。

### DELETE /api/port-groups/:id

删除端口组及关联的所有代理规则。

**响应：** `200 OK`

---

## 代理规则

### GET /api/proxies

列出所有代理规则。

**响应：** `200 OK`
```json
[
  {
    "id": "uuid",
    "name": "我的应用",
    "domain": "app.example.com",
    "target_url": "http://localhost:3000",
    "rule_type": "proxy",
    "redirect_code": null,
    "port_group_id": "uuid",
    "ssl_enabled": true,
    "force_https": true,
    "enabled": true,
    "status": "online",
    "last_checked_at": "2025-01-01T00:00:00Z",
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z"
  }
]
```

### POST /api/proxies

创建新代理规则。

**请求：**
```json
{
  "name": "我的应用",
  "domain": "app.example.com",
  "target_url": "http://localhost:3000",
  "rule_type": "proxy",
  "port_group_id": "uuid",
  "ssl_enabled": true,
  "force_https": true,
  "enabled": true
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `name` | string | 规则名称 |
| `domain` | string | 匹配域名 (Host 头) |
| `target_url` | string | 上游目标地址 |
| `rule_type` | string | `"proxy"` \| `"redirect"` \| `"tcp"` |
| `redirect_code` | int? | 重定向 HTTP 状态码 (301/302/307/308) |
| `port_group_id` | string | 所属端口组 UUID |
| `ssl_enabled` | bool | 为该域名启用 SSL |
| `force_https` | bool | HTTP 强制跳转 HTTPS |
| `enabled` | bool | 启用/禁用该规则 |

**响应：** `201 Created` — 返回创建的规则。

### PATCH /api/proxies/:id

更新代理规则。所有字段可选。

**响应：** `200 OK` — 返回更新后的规则。

### DELETE /api/proxies/:id

删除代理规则。

**响应：** `200 OK`

---

## SSL 证书

### GET /api/certificates

列出所有证书。

**响应：** `200 OK`
```json
[
  {
    "id": "uuid",
    "domain": "example.com",
    "expires_at": "2025-04-01T00:00:00Z",
    "auto_renew": true,
    "source": "acme",
    "created_at": "2025-01-01T00:00:00Z",
    "updated_at": "2025-01-01T00:00:00Z"
  }
]
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | string | 证书 UUID |
| `domain` | string | 证书域名 |
| `expires_at` | string | 过期时间 |
| `auto_renew` | bool | 是否自动续签 |
| `source` | string | 来源：`"acme"`（Let's Encrypt）或 `"manual"`（手动上传） |

### POST /api/certificates

签发新证书（异步）。返回任务 ID 用于追踪。

**请求：**
```json
{
  "domains": ["example.com", "www.example.com"],
  "staging": false
}
```

**响应：** `202 Accepted`
```json
{
  "job_id": "uuid"
}
```

### GET /api/certificates/status/:job_id

轮询证书签发进度。

**响应：** `200 OK`
```json
{
  "status": "in_progress",
  "logs": [
    { "level": "info", "message": "ACME 账户就绪" },
    { "level": "info", "message": "📋 订单: example.com" }
  ]
}
```

状态值：`"in_progress"` | `"success"` | `"failed"`

### PATCH /api/certificates/:id

更新证书记录。

**响应：** `200 OK`

### DELETE /api/certificates/:id

删除证书并清理 DNS TXT 记录。

**响应：** `200 OK`

### GET /api/certificates/:id/download

下载证书 PEM 文件。

**响应：** `200 OK` — PEM 内容。

### POST /api/certificates/upload

上传已有证书（手动证书，不会自动续签，立即推送到 SNI 缓存）。

**请求：**
```json
{
  "domain": "example.com",
  "cert_pem": "-----BEGIN CERTIFICATE-----\n...",
  "key_pem": "-----BEGIN PRIVATE KEY-----\n..."
}
```

**响应：** `200 OK`
```json
{
  "message": "Certificate uploaded",
  "id": "uuid",
  "domain": "example.com",
  "expires_at": "2025-04-01T00:00:00Z"
}
```

### POST /api/certificates/test

测试证书配置（DNS 提供商连通性、域名验证）。

**响应：** `200 OK`

---

## 系统日志

### GET /api/system/logs

获取最近 200 条系统日志。

**响应：** `200 OK`
```json
[
  {
    "id": 1,
    "level": "INFO",
    "message": "DDNS: Created A record example.com -> 1.2.3.4",
    "created_at": "2025-01-01T00:00:00Z"
  }
]
```

### GET /api/system/logs/:category

按分类获取日志。`category` 可选值：`ddns`、`ssl`、`login`、`proxy`。

**响应：** `200 OK`（同上格式）

---

## DDNS

### GET /api/ddns/status

获取 DDNS 域名解析状态。

**响应：** `200 OK`
```json
[
  {
    "domain": "home.example.com",
    "record_type": "A",
    "current_ip": "1.2.3.4",
    "last_synced": "2025-01-01T00:00:00Z",
    "enabled": true
  }
]
```

### PATCH /api/ddns/toggle/:domain

启用或禁用域名的 DDNS。

**请求：**
```json
{
  "enabled": false
}
```

**响应：** `200 OK`

### POST /api/ddns/test

测试 DNS 提供商 API 连接。

**响应：** `200 OK`
```json
{
  "success": true,
  "message": "连接成功"
}
```

### GET /api/ddns/zones

列出 DNS 区域（仅 Cloudflare 支持，阿里云自动识别）。

**响应：** `200 OK`
```json
[
  {
    "id": "zone_id",
    "name": "example.com",
    "status": "active"
  }
]
```

### DELETE /api/ddns/domain/:domain

从 DDNS 中删除域名并从 DNS 提供商移除 DNS 记录。

**响应：** `200 OK`
