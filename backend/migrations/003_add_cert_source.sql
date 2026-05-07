-- Unver Database Schema, Migration 003
-- Adds source column to certificates (acme/manual) and sets existing to 'acme'

ALTER TABLE certificates ADD COLUMN source TEXT NOT NULL DEFAULT 'acme';
