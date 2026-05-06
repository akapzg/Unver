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
  "expires_in": 86400
}
```

### POST /api/auth/refresh

Refresh an expiring JWT token.

**Response:** `200 OK`
```json
{
  "token": "string",
  "expires_in": 86400
}
```

### POST /api/auth/logout

Invalidate the current JWT token.

**Response:** `200 OK`

---

## System

### GET /api/system/dashboard

Returns dashboard statistics.

**Response:** `200 OK`
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

All proxy rule endpoints are scoped to a port group.

### GET /api/port-groups/:id/rules

List all rules for a port group.

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
    "ssl_enabled": true,
    "force_https": true,
    "enabled": true,
    "status": "online"
  }
]
```

### POST /api/port-groups/:id/rules

Create a new proxy rule.

**Request:**
```json
{
  "name": "My App",
  "domain": "app.example.com",
  "target_url": "http://localhost:3000",
  "rule_type": "proxy",
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
| `ssl_enabled` | bool | Enable SSL for this domain |
| `force_https` | bool | Redirect HTTP to HTTPS |
| `enabled` | bool | Enable/disable the rule |

**Response:** `201 Created` — returns the created rule.

### PATCH /api/port-groups/:id/rules/:rule_id

Update a proxy rule. All fields are optional.

**Response:** `200 OK` — returns the updated rule.

### DELETE /api/port-groups/:id/rules/:rule_id

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
    { "level": "info", "message": "ACME 账户就绪" },
    { "level": "info", "message": "📋 订单: example.com" }
  ]
}
```

Status values: `"in_progress"` | `"success"` | `"failed"`

### DELETE /api/certificates/:id

Delete a certificate and clean up Cloudflare DNS TXT records.

**Response:** `200 OK`

### GET /api/certificates/:id/download

Download the certificate PEM file.

**Response:** `200 OK` — PEM content.

### POST /api/certificates/upload

Upload an existing certificate.

**Request:** `multipart/form-data`
- `cert`: PEM certificate file
- `key`: PEM private key file

**Response:** `201 Created`

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

Test Cloudflare API connection.

**Response:** `200 OK`
```json
{
  "success": true,
  "message": "Connection successful"
}
```

### GET /api/ddns/zones

List Cloudflare zones.

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

Delete a domain from DDNS and remove DNS records from Cloudflare.

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
  "ddns_cf_token": "new-token",
  "ddns_cf_zone_id": "zone-id",
  "ddns_domain": "home.example.com",
  "ddns_interval": 300,
  "ddns_ip_source": "public"
}
```

**Response:** `200 OK`

### POST /api/settings/password

Change admin password.

**Request:**
```json
{
  "current_password": "string",
  "new_password": "string"
}
```

**Response:** `200 OK`

### POST /api/settings/import

Import configuration.

**Request:** `multipart/form-data`
- `file`: JSON export file

**Response:** `200 OK`

### GET /api/settings/export

Export configuration (secrets excluded).

**Response:** `200 OK` — JSON file download.

---

## Logs

### GET /api/logs

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

### GET /api/logs/:category

Logs filtered by category.

| Category | Description |
|---|---|
| `ddns` | DDNS sync events |
| `ssl` | SSL certificate events |
| `login` | Authentication events |
| `proxy` | Proxy access/error events |

**Response:** same format as above, filtered.
