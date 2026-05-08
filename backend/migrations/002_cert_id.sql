-- Unver Database Schema, Migration 002
-- Adds cert_id column to proxy_rules for explicit certificate binding.

ALTER TABLE proxy_rules ADD COLUMN cert_id TEXT;
