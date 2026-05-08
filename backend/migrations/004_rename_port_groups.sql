-- Update default port group names to English and disable HTTPS by default
UPDATE port_groups SET name = 'HTTPS', enabled = 0 WHERE id = 'pg-default';
UPDATE port_groups SET name = 'HTTP' WHERE id = 'pg-redirect-80';
