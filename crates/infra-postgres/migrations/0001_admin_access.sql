-- Admin identities, factors and restricted authentication state (data-model §5).
-- Capabilities, phone numbers, OTP codes and provisioning secrets are never stored raw.

CREATE TABLE admins (
    id UUID PRIMARY KEY,
    phone_key_id TEXT NOT NULL,
    phone_nonce BYTEA NOT NULL CHECK (octet_length(phone_nonce) > 0),
    phone_ciphertext BYTEA NOT NULL CHECK (octet_length(phone_ciphertext) > 0),
    phone_lookup_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(phone_lookup_digest) = 32),
    eligibility TEXT NOT NULL CHECK (
        eligibility IN ('invited', 'active', 'recovering', 'deactivated', 'revoked')
    ),
    security_epoch NUMERIC(20,0) NOT NULL CHECK (security_epoch BETWEEN 0 AND 18446744073709551615),
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    enrollment_purpose TEXT CHECK (enrollment_purpose IN ('addition', 'recovery')),
    enrollment_authorizer UUID,
    enrollment_authorized_at TIMESTAMPTZ,
    enrollment_expires_at TIMESTAMPTZ,
    enrollment_epoch NUMERIC(20,0) CHECK (enrollment_epoch BETWEEN 0 AND 18446744073709551615),
    enrollment_status TEXT CHECK (enrollment_status IN ('pending', 'consumed', 'revoked')),
    CONSTRAINT admins_enrollment_complete CHECK (
        num_nonnulls(
            enrollment_purpose, enrollment_authorizer, enrollment_authorized_at,
            enrollment_expires_at, enrollment_epoch, enrollment_status
        ) IN (0, 6)
    ),
    CONSTRAINT admins_enrollment_finite CHECK (
        enrollment_expires_at IS NULL OR enrollment_expires_at > enrollment_authorized_at
    ),
    CONSTRAINT admins_enrollment_epoch_current CHECK (
        enrollment_epoch IS NULL
        OR enrollment_status <> 'pending'
        OR enrollment_epoch = security_epoch
    ),
    CONSTRAINT admins_enrollment_not_self CHECK (
        enrollment_authorizer IS NULL OR enrollment_authorizer <> id
    ),
    CONSTRAINT admins_enrollment_matches_eligibility CHECK (
        enrollment_purpose IS NULL
        OR enrollment_status <> 'pending'
        OR (eligibility = 'invited' AND enrollment_purpose = 'addition')
        OR (eligibility = 'recovering' AND enrollment_purpose = 'recovery')
    )
);

CREATE TABLE admin_credentials (
    id UUID PRIMARY KEY,
    admin_id UUID NOT NULL REFERENCES admins(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL,
    webauthn_credential_id BYTEA,
    webauthn_public_key BYTEA,
    totp_key_id TEXT,
    totp_nonce BYTEA,
    totp_ciphertext BYTEA,
    credential_revision NUMERIC(20,0) NOT NULL CHECK (credential_revision BETWEEN 1 AND 18446744073709551615),
    security_epoch NUMERIC(20,0) NOT NULL CHECK (security_epoch BETWEEN 0 AND 18446744073709551615),
    last_totp_step NUMERIC(20,0) CHECK (last_totp_step BETWEEN 0 AND 18446744073709551615),
    revoked_at TIMESTAMPTZ,
    CONSTRAINT admin_credentials_kind_material_xor CHECK (
        (
            kind = 'passkey'
            AND webauthn_credential_id IS NOT NULL
            AND webauthn_public_key IS NOT NULL
            AND totp_key_id IS NULL
            AND totp_nonce IS NULL
            AND totp_ciphertext IS NULL
            AND last_totp_step IS NULL
        )
        OR (
            kind = 'totp'
            AND webauthn_credential_id IS NULL
            AND webauthn_public_key IS NULL
            AND totp_key_id IS NOT NULL
            AND totp_nonce IS NOT NULL
            AND totp_ciphertext IS NOT NULL
        )
    ),
    CONSTRAINT admin_credentials_webauthn_material_nonempty CHECK (
        webauthn_credential_id IS NULL
        OR (
            octet_length(webauthn_credential_id) > 0
            AND octet_length(webauthn_public_key) > 0
        )
    ),
    CONSTRAINT admin_credentials_totp_envelope_nonempty CHECK (
        totp_key_id IS NULL
        OR (octet_length(totp_nonce) > 0 AND octet_length(totp_ciphertext) > 0)
    )
);

CREATE UNIQUE INDEX admin_credentials_webauthn_id_unique
    ON admin_credentials (webauthn_credential_id)
    WHERE webauthn_credential_id IS NOT NULL;
CREATE INDEX admin_credentials_active_admin
    ON admin_credentials (admin_id, id)
    WHERE revoked_at IS NULL;

CREATE TABLE admin_sessions (
    id UUID PRIMARY KEY,
    token_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    admin_id UUID NOT NULL REFERENCES admins(id) ON DELETE RESTRICT,
    security_epoch NUMERIC(20,0) NOT NULL CHECK (security_epoch BETWEEN 0 AND 18446744073709551615),
    primary_verified BOOLEAN NOT NULL DEFAULT TRUE,
    authenticated_factor TEXT CHECK (authenticated_factor IN ('passkey', 'totp')),
    issued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    idle_timeout_seconds BIGINT NOT NULL DEFAULT 120 CHECK (idle_timeout_seconds > 0),
    last_activity_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    CONSTRAINT admin_sessions_finite_expiry CHECK (expires_at > issued_at),
    CONSTRAINT admin_sessions_complete_authentication CHECK (
        primary_verified = (authenticated_factor IS NOT NULL)
    ),
    CONSTRAINT admin_sessions_activity_not_before_issue CHECK (last_activity_at >= issued_at)
);

CREATE INDEX admin_sessions_active_admin
    ON admin_sessions (admin_id, id)
    WHERE revoked_at IS NULL;

CREATE TABLE admin_session_states (
    id UUID PRIMARY KEY,
    reference_digest BYTEA NOT NULL UNIQUE CHECK (octet_length(reference_digest) = 32),
    admin_id UUID REFERENCES admins(id) ON DELETE RESTRICT,
    browser_digest BYTEA NOT NULL CHECK (octet_length(browser_digest) = 32),
    session_id UUID NOT NULL,
    purpose TEXT NOT NULL CHECK (purpose IN ('sign_in', 'addition', 'recovery')),
    phase TEXT NOT NULL CHECK (phase IN ('primary_verified', 'passkey', 'totp')),
    security_epoch NUMERIC(20,0) NOT NULL CHECK (security_epoch BETWEEN 0 AND 18446744073709551615),
    primary_verified_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    superseded_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    credential_revision NUMERIC(20,0) CHECK (credential_revision BETWEEN 1 AND 18446744073709551615),
    verification_key_id TEXT,
    verification_nonce BYTEA,
    verification_ciphertext BYTEA,
    provisioning_key_id TEXT,
    provisioning_nonce BYTEA,
    provisioning_ciphertext BYTEA,
    provisioning_presented_at TIMESTAMPTZ,
    CONSTRAINT admin_session_states_finite_expiry CHECK (
        expires_at > created_at AND primary_verified_at <= created_at
    ),
    CONSTRAINT admin_session_states_one_terminal_transition CHECK (
        num_nonnulls(consumed_at, superseded_at, revoked_at) <= 1
    ),
    CONSTRAINT admin_session_states_verification_envelope_complete CHECK (
        num_nonnulls(verification_key_id, verification_nonce, verification_ciphertext) IN (0, 3)
    ),
    CONSTRAINT admin_session_states_provisioning_envelope_complete CHECK (
        num_nonnulls(provisioning_key_id, provisioning_nonce, provisioning_ciphertext) IN (0, 3)
    ),
    CONSTRAINT admin_session_states_phase_payload CHECK (
        (
            phase = 'primary_verified'
            AND verification_key_id IS NULL
            AND provisioning_key_id IS NULL
            AND provisioning_presented_at IS NULL
        )
        OR (phase IN ('passkey', 'totp') AND verification_key_id IS NOT NULL)
    ),
    CONSTRAINT admin_session_states_provisioning_phase CHECK (
        provisioning_key_id IS NULL AND provisioning_presented_at IS NULL
        OR (
            phase = 'totp'
            AND purpose IN ('addition', 'recovery')
        )
    ),
    CONSTRAINT admin_session_states_verification_envelope_nonempty CHECK (
        verification_key_id IS NULL
        OR (
            octet_length(verification_nonce) > 0
            AND octet_length(verification_ciphertext) > 0
        )
    ),
    CONSTRAINT admin_session_states_provisioning_envelope_nonempty CHECK (
        provisioning_key_id IS NULL
        OR (
            octet_length(provisioning_nonce) > 0
            AND octet_length(provisioning_ciphertext) > 0
        )
    )
);

CREATE INDEX admin_session_states_active_admin
    ON admin_session_states (admin_id, id)
    WHERE consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL;
CREATE INDEX admin_session_states_active_session
    ON admin_session_states (session_id, id)
    WHERE consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL;

CREATE TABLE admin_otp_challenges (
    id UUID PRIMARY KEY,
    admin_id UUID NOT NULL REFERENCES admins(id) ON DELETE RESTRICT,
    browser_digest BYTEA NOT NULL CHECK (octet_length(browser_digest) = 32),
    purpose TEXT NOT NULL CHECK (purpose IN ('sign_in', 'addition', 'recovery')),
    security_epoch NUMERIC(20,0) NOT NULL CHECK (security_epoch BETWEEN 0 AND 18446744073709551615),
    code_digest BYTEA NOT NULL CHECK (octet_length(code_digest) = 32),
    issued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    failed_attempts INTEGER NOT NULL CHECK (failed_attempts >= 0),
    max_attempts INTEGER NOT NULL CHECK (max_attempts > 0),
    consumed_at TIMESTAMPTZ,
    superseded_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    submission_category TEXT NOT NULL CHECK (
        submission_category IN ('accepted', 'rejected', 'unavailable', 'unknown')
    ),
    predecessor_id UUID REFERENCES admin_otp_challenges(id) ON DELETE RESTRICT,
    CONSTRAINT admin_otp_challenges_finite_expiry CHECK (expires_at > issued_at),
    CONSTRAINT admin_otp_challenges_attempt_budget CHECK (failed_attempts <= max_attempts),
    CONSTRAINT admin_otp_challenges_one_terminal_transition CHECK (
        num_nonnulls(consumed_at, superseded_at, revoked_at) <= 1
    )
);

CREATE INDEX admin_otp_challenges_active_binding
    ON admin_otp_challenges (admin_id, browser_digest, purpose, security_epoch, id)
    WHERE consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL;

CREATE TABLE admin_rate_limits (
    scope TEXT NOT NULL CHECK (scope IN ('subject', 'origin')),
    purpose TEXT NOT NULL CHECK (purpose IN ('send', 'verify')),
    bucket_digest BYTEA NOT NULL CHECK (octet_length(bucket_digest) = 32),
    window_started_at TIMESTAMPTZ NOT NULL,
    window_expires_at TIMESTAMPTZ NOT NULL,
    count BIGINT NOT NULL CHECK (count >= 0),
    max_attempts BIGINT NOT NULL CHECK (max_attempts > 0),
    blocked_until TIMESTAMPTZ,
    CONSTRAINT admin_rate_limits_finite_window CHECK (window_expires_at > window_started_at),
    PRIMARY KEY (scope, purpose, bucket_digest, window_started_at)
);

-- Cluster roles are provisioned by the fixture/operator, never by a per-database migration.
GRANT SELECT, INSERT, UPDATE ON admins TO tankhor_fixture_runtime;
GRANT SELECT, INSERT, UPDATE ON admin_credentials TO tankhor_fixture_runtime;
GRANT SELECT, INSERT, UPDATE ON admin_sessions TO tankhor_fixture_runtime;
GRANT SELECT, INSERT, UPDATE ON admin_session_states TO tankhor_fixture_runtime;
GRANT SELECT, INSERT, UPDATE ON admin_otp_challenges TO tankhor_fixture_runtime;
GRANT SELECT, INSERT, UPDATE ON admin_rate_limits TO tankhor_fixture_runtime;
