-- Persist MLS key packages with rotation history
CREATE TABLE IF NOT EXISTS mls_key_packages (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    identity TEXT NOT NULL,
    ciphersuite TEXT NOT NULL,
    signing_key TEXT NOT NULL,
    signature_key TEXT NOT NULL,
    hpke_public_key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_mls_key_packages_identity_created_at
    ON mls_key_packages (identity, created_at DESC);
