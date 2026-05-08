-- Migration no-op: 004 previously renamed default port groups (001 now uses correct names directly).
-- Kept as placeholder to avoid sqlx "missing migration" errors on upgrades from older versions.
SELECT 1;
