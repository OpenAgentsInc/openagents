-- Add pseudonym column to users table
ALTER TABLE users ADD COLUMN pseudonym TEXT;

-- Create index for faster lookups
CREATE INDEX idx_users_pseudonym ON users(pseudonym);
