-- Unver Database Schema, Migration 002
-- Adds cert_id to proxy_rules, ddns_domains default setting

ALTER TABLE proxy_rules ADD COLUMN cert_id TEXT;

INSERT OR IGNORE INTO settings (key, value) VALUES ('ddns_domains', '');
