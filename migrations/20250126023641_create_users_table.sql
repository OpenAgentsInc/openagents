CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    scramble_id VARCHAR(255) UNIQUE NOT NULL,  -- Pseudonymous ID from Scramble OIDC
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
);

-- Add index for faster scramble_id lookups
CREATE INDEX idx_users_scramble_id ON users(scramble_id);


