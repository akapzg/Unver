# Unver API 文档

Base URL: `http://<host>:<port>/api`

除 `/api/auth/*` 和 `/api/setup*` 外，所有接口需要 JWT Bearer Token：

```
Authorization: Bearer <token>
```

---

## 认证

### POST /api/auth/login

登录，获取 JWT Token。

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
  "expires_in": 86400
}
```

### POST /api/auth/refresh

刷新即将过期的 JWT Token。

**响应：** `200 OK`

### POST /api/auth/logout

登出，使当前 JWT Token 失效。

**响应：** `200 OK`

---

## 系统

### GET /api/system/dashboard

获取仪表盘统计数据。

**响应：** `200 OK`
```json
{
  "proxy_rules": 5,
  "port_groups": 2,
  "certificates": 1,
  "ddns_domains": 3,
  "uptime_seconds": 3600,
  "cpu_percent": 12.5,
  "memory_used_mb": 48,
  "memory_total_mb": 1024
}
```

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

## 端口组

### GET /api/port-groups

端口组列表。

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

新建端口组。

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

| 字段 | 类型 | 说明 |
|---|---|---|
| `name` | string | 端口组名称 |
| `listen_port` | int | 监听端口（1-65535） |
| `enabled` | bool | 是否启用 |
| `skip_tls_verify` | bool | 跳过上游 TLS 验证 |
| `force_https` | bool | 强制 HTTP→HTTPS 跳转 |

**响应：** `201 Created` — 返回创建的端口组。

### PATCH /api/port-groups/:id

更新端口组，所有字段可选。

**响应：** `200 OK`

### DELETE /api/port-groups/:id

删除端口组及所有关联规则。

**响应：** `200 OK`

---

## 代理规则

所有规则接口基于端口组。

### GET /api/port-groups/:id/rules

端口组内的规则列表。

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
    "ssl_enabled": true,
    "force_https": true,
    "enabled": true,
    "status": "online"
  }
]
```

### POST /api/port-groups/:id/rules

新建规则。

**请求：**
```json
{
  "name": "我的应用",
  "domain": "app.example.com",
  "target_url": "http://localhost:3000",
  "rule_type": "proxy",
  "ssl_enabled": true,
  "force_https": true,
  "enabled": true
}
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `name` | string | 规则名称 |
| `domain` | string | 匹配域名（Host header） |
| `target_url` | string | 上游目标 URL |
| `rule_type` | string | `"proxy"` \| `"redirect"` \| `"tcp"` |
| `redirect_code` | int? | 跳转 HTTP 状态码（301/302/307/308） |
| `ssl_enabled` | bool | 为该域名启用 SSL |
| `force_https` | bool | 强制 HTTP→HTTPS（规则级） |
| `enabled` | bool | 启用/禁用 |

**响应：** `201 Created` — 返回创建的规则。

### PATCH /api/port-groups/:id/rules/:rule_id

更新规则，所有字段可选。

**响应：** `200 OK`

### DELETE /api/port-groups/:id/rules/:rule_id

删除规则。

**响应：** `200 OK`

---

## SSL 证书

### GET /api/certificates

证书列表。

**响应：** `200 OK`
```json
[
  {
    "id": "uuid",
    "domains": ["example.com", "*.example.com"],
    "issuer": "Let's Encrypt",
    "not_before": "2025-01-01T00:00:00Z",
    "not_after": "2025-04-01T00:00:00Z",
    "status": "valid",
    "auto_renew": true
  }
]
```

### POST /api/certificates

签发新证书（异步），返回 job_id 用于跟踪进度。

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

查询签发进度。

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

### DELETE /api/certificates/:id

删除证书并清理 Cloudflare DNS TXT 记录。

**响应：** `200 OK`

### GET /api/certificates/:id/download

下载证书 PEM 文件。

**响应：** `200 OK` — PEM 内容。

### POST /api/certificates/upload

上传已有证书。

**请求：** `multipart/form-data`
- `cert`: PEM 证书文件
- `key`: PEM 私钥文件

**响应：** `201 Created`

---

## DDNS

### GET /api/ddns/status

域名解析状态。

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

启用/禁用域名的 DDNS。

**请求：**
```json
{
  "enabled": false
}
```

**响应：** `200 OK`

### POST /api/ddns/test

测试 Cloudflare API 连接。

**响应：** `200 OK`

### GET /api/ddns/zones

获取 Cloudflare Zone 列表。

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

删除域名（同步清除 Cloudflare 解析记录）。

**响应：** `200 OK`

---

## 设置

### GET /api/settings

获取所有设置（密钥已脱敏）。

**响应：** `200 OK`

### PATCH /api/settings

更新设置，所有字段可选。

**请求：**
```json
{
  "ddns_enabled": true,
  "ddns_provider": "cloudflare",
  "ddns_cf_token": "new-token",
  "ddns_cf_zone_id": "zone-id",
  "ddns_domain": "home.example.com",
  "ddns_interval": 300,
  "ddns_ip_source": "public"
}
```

**响应：** `200 OK`

### POST /api/settings/password

修改管理员密码。

**请求：**
```json
{
  "current_password": "string",
  "new_password": "string"
}
```

**响应：** `200 OK`

### POST /api/settings/import

导入配置。

**请求：** `multipart/form-data`
- `file`: JSON 导出文件

**响应：** `200 OK`

### GET /api/settings/export

导出配置（不含密钥）。

**响应：** `200 OK` — JSON 文件下载。

---

## 日志

### GET /api/logs

最近日志（最近 100 条）。

**响应：** `200 OK`
```json
[
  {
    "id": 1,
    "level": "INFO",
    "message": "DDNS: Updated A home.example.com -> 1.2.3.4",
    "created_at": "2025-01-01 12:00:00"
  }
]
```

### GET /api/logs/:category

按分类过滤日志。

| 分类 | 说明 |
|---|---|
| `ddns` | DDNS 同步事件 |
| `ssl` | SSL 证书事件 |
| `login` | 认证事件 |
| `proxy` | 代理访问/错误事件 |

**响应：** 同上格式，按分类过滤。
