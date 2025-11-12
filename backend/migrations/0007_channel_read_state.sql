CREATE TABLE IF NOT EXISTS channel_read_state (
    channel_id UUID NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    last_read_sequence BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (channel_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_channel_read_state_user
    ON channel_read_state (user_id);
