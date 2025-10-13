CREATE TABLE IF NOT EXISTS refresh_sessions (
    refresh_id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    session_id UUID NOT NULL,
    device_id TEXT NOT NULL,
    device_name TEXT,
    user_agent TEXT,
    ip_address TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    CONSTRAINT fk_refresh_sessions_user FOREIGN KEY (user_id) REFERENCES users (user_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS refresh_sessions_user_idx ON refresh_sessions (user_id);
CREATE UNIQUE INDEX IF NOT EXISTS refresh_sessions_device_idx ON refresh_sessions (user_id, device_id);
