-- Unver Database Schema
-- Migration: 001_init.sql (complete schema)

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    username      TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS refresh_tokens (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_hash ON refresh_tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user ON refresh_tokens(user_id);

CREATE TABLE IF NOT EXISTS port_groups (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    listen_port     INTEGER NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1,
    skip_tls_verify INTEGER NOT NULL DEFAULT 0,
    force_https     INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS proxy_rules (
    id            TEXT PRIMARY KEY,
    port_group_id TEXT,
    name          TEXT NOT NULL,
    domain        TEXT NOT NULL,
    target_url    TEXT NOT NULL,
    rule_type     TEXT NOT NULL DEFAULT 'proxy',
    redirect_code INTEGER DEFAULT 301,
    ssl_enabled   INTEGER NOT NULL DEFAULT 0,
    force_https   INTEGER NOT NULL DEFAULT 0,
    enabled       INTEGER NOT NULL DEFAULT 1,
    status        TEXT NOT NULL DEFAULT 'unknown',
    last_checked_at TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Default port group: HTTPS (8443) — disabled by default
INSERT OR IGNORE INTO port_groups (id, name, listen_port, enabled, skip_tls_verify, force_https)
VALUES ('pg-default', 'HTTPS', 8443, 0, 0, 0);

-- Default port group: HTTP (80) redirect to HTTPS — disabled by default
INSERT OR IGNORE INTO port_groups (id, name, listen_port, enabled, skip_tls_verify, force_https)
VALUES ('pg-redirect-80', 'HTTP', 80, 0, 0, 1);

CREATE TABLE IF NOT EXISTS certificates (
    id         TEXT PRIMARY KEY,
    domain     TEXT UNIQUE NOT NULL,
    cert_pem   TEXT NOT NULL,
    key_pem    TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    auto_renew INTEGER NOT NULL DEFAULT 1,
    source     TEXT NOT NULL DEFAULT 'acme',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS logs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    level      TEXT NOT NULL,
    message    TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS api_keys (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    key_hash     TEXT NOT NULL,
    key_prefix   TEXT NOT NULL,
    enabled      INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at TEXT
);

-- Default settings (idempotent)
INSERT OR IGNORE INTO settings (key, value) VALUES
    ('setup_complete',   'false'),
    ('api_auth_enabled', 'false'),
    ('jwt_secret',       ''),
    ('acme_email',       ''),
    ('ddns_enabled',     'false'),
    ('ddns_provider',    'cloudflare'),
    ('ddns_cf_token',    ''),
    ('ddns_cf_zone_id',  ''),
    ('ddns_domain',      ''),
    ('ddns_interval',    '300'),
    ('ddns_ip_source',   'public');
