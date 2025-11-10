-- Enrich membership and role metadata for users, guilds, and channels.
CREATE TABLE
    IF NOT EXISTS guild_memberships (
        guild_id UUID NOT NULL REFERENCES guilds (guild_id) ON DELETE CASCADE,
        user_id UUID NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
        role TEXT NOT NULL DEFAULT 'member',
        joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
        PRIMARY KEY (guild_id, user_id)
    );

CREATE INDEX IF NOT EXISTS guild_memberships_user_idx ON guild_memberships (user_id);

ALTER TABLE channel_memberships
ALTER COLUMN role
SET
    NOT NULL;

ALTER TABLE channel_memberships ADD CONSTRAINT channel_memberships_user_fk FOREIGN KEY (user_id) REFERENCES users (user_id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS channel_memberships_user_idx ON channel_memberships (user_id);

CREATE TABLE
    IF NOT EXISTS user_roles (
        user_id UUID NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
        role TEXT NOT NULL,
        granted_at TIMESTAMPTZ NOT NULL DEFAULT NOW (),
        granted_by UUID NULL,
        PRIMARY KEY (user_id, role)
    );

CREATE INDEX IF NOT EXISTS user_roles_role_idx ON user_roles (role);
