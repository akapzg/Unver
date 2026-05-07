# Unver API Documentation

Base URL: `http://<host>:<port>/api`

All endpoints except `/api/auth/*` and `/api/setup*` require a JWT Bearer token:

```
Authorization: Bearer <token>
```

---

## Authentication

### POST /api/auth/login

Login and obtain a JWT token.

**Request:**
```json
{
  "username": "string",
  "password": "string"
}
```

**Response:** `200 OK`
```json
{
  "token": "string",
  "expires_in": 3600
}
```

### POST /api/auth/refresh

Refresh an expiring JWT token.

**Response:** `200 OK`
```json
{
  "token": "string",
  "expires_in": 3600
}
```

### POST /api/auth/logout

Invalidate the current JWT token.

**Response:** `200 OK`

### POST /api/auth/change-password

Change admin password.

**Request:**
```json
{
  "current_password": "string",
  "new_password": "string"
}
```

**Response:** `200 OK`

---

## System

### GET /api/system/stats

Returns dashboard statistics.

**Response:** `200 OK`
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

| Field | Type | Description |
|---|---|---|
| `proxy_rules` | int | Total proxy rules |
| `active_proxies` | int | Enabled proxy rules |
| `certificates` | int | Total certificates |
| `auto_renew_certs` | int | Certificates with auto-renew enabled |
| `version` | string | Unver version |
| `cpu_percent` | int | CPU usage (0-100) |
| `mem_percent` | int | Memory usage (0-100) |
| `mem_used` | int | Used memory in bytes |
| `mem_total` | int | Total memory in bytes |
| `disk_percent` | int | Disk usage (0-100) |
| `uptime_seconds` | int | Uptime in seconds |
| `db_size_bytes` | int | Database file size in bytes |

### GET /api/system/network

Real-time network traffic.

**Response:** `200 OK`
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

Get the server's public IP address.

**Response:** `200 OK`
```json
{
  "ipv4": "1.2.3.4",
  "ipv6": "2001:db8::1"
}
```

### GET /api/system/logs

Recent logs (last 100 entries).

**Response:** `200 OK`
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

### GET /api/system/logs/:category

Logs filtered by category.

| Category | Description |
|---|---|
| `ddns` | DDNS sync events |
| `ssl` | SSL certificate events |
| `login` | Authentication events |
| `proxy` | Proxy access/error events |

**Response:** same format as above, filtered.

### POST /api/system/restart

Restart the Unver service.

**Request:**
```json
{
  "confirm": true
}
```

**Response:** `200 OK` — service restarts immediately.

### POST /api/system/renew-ssl

Manually trigger renewal of all SSL certificates.

**Response:** `200 OK`

---

## Settings

### GET /api/settings

Get all settings (secrets masked).

**Response:** `200 OK`

### PATCH /api/settings

Update settings. All fields are optional.

**Request:**
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

**Response:** `200 OK`

### GET /api/settings/api-keys

List all API keys.

**Response:** `200 OK`

### POST /api/settings/api-keys

Create a new API key.

**Response:** `201 Created`

### DELETE /api/settings/api-keys/:id

Delete an API key.

**Response:** `200 OK`

### GET /api/system/backup

Export configuration (secrets excluded).

**Response:** `200 OK` — JSON file download.

### POST /api/system/restore

Import configuration.

**Request:** `multipart/form-data`
- `file`: JSON export file

**Response:** `200 OK`

---

## Port Groups

### GET /api/port-groups

List all port groups.

**Response:** `200 OK`
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

Create a new port group.

**Request:**
```json
{
  "name": "HTTPS",
  "listen_port": 443,
  "enabled": true,
  "skip_tls_verify": false,
  "force_https": false
}
```

**Response:** `201 Created` — returns the created port group.

### PATCH /api/port-groups/:id

Update a port group. All fields are optional.

**Request:**
```json
{
  "name": "New Name",
  "enabled": false,
  "force_https": true
}
```

**Response:** `200 OK` — returns the updated port group.

### DELETE /api/port-groups/:id

Delete a port group and all associated proxy rules.

**Response:** `200 OK`

---

## Proxy Rules

### GET /api/proxies

List all proxy rules.

**Response:** `200 OK`
```json
[
  {
    "id": "uuid",
    "name": "My App",
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

Create a new proxy rule.

**Request:**
```json
{
  "name": "My App",
  "domain": "app.example.com",
  "target_url": "http://localhost:3000",
  "rule_type": "proxy",
  "port_group_id": "uuid",
  "ssl_enabled": true,
  "force_https": true,
  "enabled": true
}
```

| Field | Type | Description |
|---|---|---|
| `name` | string | Rule name |
| `domain` | string | Domain to match (Host header) |
| `target_url` | string | Upstream target URL |
| `rule_type` | string | `"proxy"` \| `"redirect"` \| `"tcp"` |
| `redirect_code` | int? | Redirect HTTP code (301/302/307/308) |
| `port_group_id` | string | Port group UUID |
| `ssl_enabled` | bool | Enable SSL for this domain |
| `force_https` | bool | Redirect HTTP to HTTPS |
| `enabled` | bool | Enable/disable the rule |

**Response:** `201 Created` — returns the created rule.

### PATCH /api/proxies/:id

Update a proxy rule. All fields are optional.

**Response:** `200 OK` — returns the updated rule.

### DELETE /api/proxies/:id

Delete a proxy rule.

**Response:** `200 OK`

---

## SSL Certificates

### GET /api/certificates

List all certificates.

**Response:** `200 OK`
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

| Field | Type | Description |
|---|---|---|
| `id` | string | Certificate UUID |
| `domain` | string | Domain name |
| `expires_at` | string | Expiration time |
| `auto_renew` | bool | Auto-renew enabled |
| `source` | string | `"acme"` (Let's Encrypt) or `"manual"` (uploaded) |

### POST /api/certificates

Issue a new certificate (async). Returns a job ID for tracking.

**Request:**
```json
{
  "domains": ["example.com", "www.example.com"],
  "staging": false
}
```

**Response:** `202 Accepted`
```json
{
  "job_id": "uuid"
}
```

### GET /api/certificates/status/:job_id

Poll certificate issuance progress.

**Response:** `200 OK`
```json
{
  "status": "in_progress",
  "logs": [
    { "level": "info", "message": "ACME account ready" },
    { "level": "info", "message": "Order: example.com" }
  ]
}
```

Status values: `"in_progress"` | `"success"` | `"failed"`

### PATCH /api/certificates/:id

Update a certificate record.

**Response:** `200 OK`

### DELETE /api/certificates/:id

Delete certificate and clean up DNS TXT records.

**Response:** `200 OK`

### GET /api/certificates/:id/download

Download the certificate PEM file.

**Response:** `200 OK` — PEM content.

### POST /api/certificates/upload

Upload an existing certificate (manual, no auto-renew, pushed to SNI cache immediately).

**Request:**
```json
{
  "domain": "example.com",
  "cert_pem": "-----BEGIN CERTIFICATE-----\n...",
  "key_pem": "-----BEGIN PRIVATE KEY-----\n..."
}
```

**Response:** `200 OK`
```json
{
  "message": "Certificate uploaded",
  "id": "uuid",
  "domain": "example.com",
  "expires_at": "2025-04-01T00:00:00Z"
}
```

### POST /api/certificates/test

Test certificate configuration (DNS provider connectivity, domain validation).

**Response:** `200 OK`

---

## System Logs

### GET /api/system/logs

Get recent 200 system log entries.

**Response:** `200 OK`
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

Get logs by category. Valid categories: `ddns`, `ssl`, `login`, `proxy`.

**Response:** `200 OK` (same format)

---

## DDNS

### GET /api/ddns/status

Get DDNS domain resolution status.

**Response:** `200 OK`
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

Enable or disable DDNS for a domain.

**Request:**
```json
{
  "enabled": false
}
```

**Response:** `200 OK`

### POST /api/ddns/test

Test DNS provider API connection.

**Response:** `200 OK`
```json
{
  "success": true,
  "message": "Connection successful"
}
```

### GET /api/ddns/zones

List DNS zones (Cloudflare only; Aliyun auto-detects).

**Response:** `200 OK`
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

Remove domain from DDNS and delete DNS records from DNS provider.

**Response:** `200 OK`
