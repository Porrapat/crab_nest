-- Add voice message support to messages table
-- This migration adds message_type and voice_path columns

ALTER TABLE messages ADD COLUMN message_type TEXT DEFAULT 'text';
ALTER TABLE messages ADD COLUMN voice_path TEXT;
