use std::collections::HashSet;

use sqlx::{PgPool, postgres::PgConnectOptions, postgres::PgPoolOptions};

const ADMIN: &str = "019a53a0-0000-7000-8000-000000000001";
const OTHER_ADMIN: &str = "019a53a0-0000-7000-8000-000000000002";
const CREDENTIAL: &str = "019a53a0-0000-7000-8000-000000000011";
const SESSION: &str = "019a53a0-0000-7000-8000-000000000021";
const PENDING: &str = "019a53a0-0000-7000-8000-000000000031";
const OTP: &str = "019a53a0-0000-7000-8000-000000000041";

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn auth_schema_enforces_identity_proof_and_credential_constraints(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let pool = admin_pool_options
        .max_connections(2)
        .connect_with(admin_options)
        .await
        .expect("connect to migrated fixture database");

    for table in [
        "admins",
        "admin_credentials",
        "admin_sessions",
        "admin_session_states",
        "admin_otp_challenges",
        "admin_rate_limits",
    ] {
        let present: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
            .bind(format!("public.{table}"))
            .fetch_one(&pool)
            .await
            .expect("inspect required table");
        assert_eq!(present.as_deref(), Some(table), "missing table {table}");
    }

    assert_columns(
        &pool,
        "admins",
        &[
            "id",
            "phone_key_id",
            "phone_nonce",
            "phone_ciphertext",
            "phone_lookup_digest",
            "eligibility",
            "security_epoch",
            "created_by",
            "created_at",
            "enrollment_purpose",
            "enrollment_authorizer",
            "enrollment_authorized_at",
            "enrollment_expires_at",
            "enrollment_epoch",
            "enrollment_status",
        ],
    )
    .await;
    assert_columns(
        &pool,
        "admin_credentials",
        &[
            "id",
            "admin_id",
            "kind",
            "webauthn_credential_id",
            "webauthn_public_key",
            "totp_key_id",
            "totp_nonce",
            "totp_ciphertext",
            "credential_revision",
            "security_epoch",
            "last_totp_step",
            "revoked_at",
        ],
    )
    .await;
    assert_columns(
        &pool,
        "admin_sessions",
        &[
            "id",
            "token_digest",
            "admin_id",
            "security_epoch",
            "authenticated_factor",
            "issued_at",
            "expires_at",
            "last_activity_at",
            "revoked_at",
        ],
    )
    .await;
    assert_columns(
        &pool,
        "admin_session_states",
        &[
            "id",
            "reference_digest",
            "admin_id",
            "browser_digest",
            "session_id",
            "purpose",
            "phase",
            "security_epoch",
            "primary_verified_at",
            "created_at",
            "expires_at",
            "consumed_at",
            "superseded_at",
            "revoked_at",
            "credential_revision",
            "verification_key_id",
            "verification_nonce",
            "verification_ciphertext",
            "provisioning_key_id",
            "provisioning_nonce",
            "provisioning_ciphertext",
            "provisioning_presented_at",
        ],
    )
    .await;
    assert_columns(
        &pool,
        "admin_otp_challenges",
        &[
            "id",
            "admin_id",
            "browser_digest",
            "purpose",
            "security_epoch",
            "code_digest",
            "issued_at",
            "expires_at",
            "failed_attempts",
            "max_attempts",
            "consumed_at",
            "superseded_at",
            "revoked_at",
            "submission_category",
        ],
    )
    .await;
    assert_columns(
        &pool,
        "admin_rate_limits",
        &[
            "scope",
            "purpose",
            "bucket_digest",
            "window_started_at",
            "window_expires_at",
            "count",
            "max_attempts",
            "blocked_until",
        ],
    )
    .await;

    let plaintext_columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name = ANY($1)
           AND column_name = ANY($2)",
    )
    .bind([
        "admins",
        "admin_credentials",
        "admin_sessions",
        "admin_session_states",
        "admin_otp_challenges",
    ])
    .bind([
        "phone",
        "phone_number",
        "code",
        "token",
        "raw_token",
        "secret",
        "provisioning_secret",
        "serialized_state",
    ])
    .fetch_one(&pool)
    .await
    .expect("inspect forbidden plaintext columns");
    assert_eq!(plaintext_columns, 0);

    seed_admin(&pool, ADMIN, 1).await;
    seed_admin(&pool, OTHER_ADMIN, 2).await;
    assert_rejected(
        &pool,
        "INSERT INTO admins (
             id, phone_key_id, phone_nonce, phone_ciphertext, phone_lookup_digest,
             eligibility, security_epoch, created_by, created_at
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000003', 'key', '\\x01'::bytea,
             '\\x02'::bytea, decode(repeat('01', 32), 'hex'), 'active', 0,
             '019a53a0-0000-7000-8000-000000000002', now()
         )",
        "duplicate admin lookup digest",
    )
    .await;
    assert_rejected(
        &pool,
        "UPDATE admins SET enrollment_purpose = 'addition',
             enrollment_authorizer = '019a53a0-0000-7000-8000-000000000002',
             enrollment_authorized_at = now(), enrollment_expires_at = now() + interval '1 hour',
             enrollment_epoch = 0, enrollment_status = 'pending'
         WHERE id = '019a53a0-0000-7000-8000-000000000001'",
        "authorization on an active rather than invited admin",
    )
    .await;
    sqlx::query(
        "UPDATE admins SET eligibility = 'invited', enrollment_purpose = 'addition',
             enrollment_authorizer = $2::uuid, enrollment_authorized_at = now(),
             enrollment_expires_at = now() + interval '1 hour', enrollment_epoch = 0,
             enrollment_status = 'pending' WHERE id = $1::uuid",
    )
    .bind(ADMIN)
    .bind(OTHER_ADMIN)
    .execute(&pool)
    .await
    .expect("record target-bound enrollment authorization");
    let consumed: u64 = sqlx::query(
        "UPDATE admins SET enrollment_status = 'consumed'
         WHERE id = $1::uuid AND enrollment_status = 'pending' AND enrollment_epoch = security_epoch",
    )
    .bind(ADMIN)
    .execute(&pool)
    .await
    .expect("conditionally consume authorization")
    .rows_affected();
    assert_eq!(consumed, 1);
    let replayed: u64 = sqlx::query(
        "UPDATE admins SET enrollment_status = 'consumed'
         WHERE id = $1::uuid AND enrollment_status = 'pending' AND enrollment_epoch = security_epoch",
    )
    .bind(ADMIN)
    .execute(&pool)
    .await
    .expect("reject repeated authorization consumption")
    .rows_affected();
    assert_eq!(replayed, 0);

    sqlx::query(
        "INSERT INTO admin_credentials (
             id, admin_id, kind, webauthn_credential_id, webauthn_public_key,
             credential_revision, security_epoch
         ) VALUES ($1::uuid, $2::uuid, 'passkey', decode(repeat('a1', 32), 'hex'), '\\x02'::bytea, 1, 0)",
    )
    .bind(CREDENTIAL)
    .bind(ADMIN)
    .execute(&pool)
    .await
    .expect("insert passkey credential");
    assert_rejected(
        &pool,
        "INSERT INTO admin_credentials (
             id, admin_id, kind, webauthn_credential_id, webauthn_public_key,
             credential_revision, security_epoch
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000012',
             '019a53a0-0000-7000-8000-000000000002', 'passkey',
             decode(repeat('a1', 32), 'hex'), '\\x02'::bytea, 1, 0
         )",
        "globally duplicated WebAuthn credential ID",
    )
    .await;
    assert_rejected(
        &pool,
        "INSERT INTO admin_credentials (
             id, admin_id, kind, credential_revision, security_epoch
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000013',
             '019a53a0-0000-7000-8000-000000000001', 'passkey', 1, 0
         )",
        "passkey without public material",
    )
    .await;
    assert_rejected(
        &pool,
        "INSERT INTO admin_credentials (
             id, admin_id, kind, webauthn_credential_id, webauthn_public_key,
             totp_key_id, totp_nonce, totp_ciphertext, credential_revision, security_epoch
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000014',
             '019a53a0-0000-7000-8000-000000000001', 'totp',
             decode(repeat('a2', 32), 'hex'), '\\x02'::bytea,
             'key', '\\x03'::bytea, '\\x04'::bytea, 1, 0
         )",
        "mixed credential material",
    )
    .await;
    sqlx::query(
        "INSERT INTO admin_credentials (
             id, admin_id, kind, totp_key_id, totp_nonce, totp_ciphertext,
             credential_revision, security_epoch
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000015',
             '019a53a0-0000-7000-8000-000000000002', 'totp',
             'key', '\\x03'::bytea, '\\x04'::bytea, 1, 0
         )",
    )
    .execute(&pool)
    .await
    .expect("insert encrypted TOTP credential");
    assert_rejected(
        &pool,
        "UPDATE admin_credentials SET credential_revision = 0 WHERE id = '019a53a0-0000-7000-8000-000000000011'",
        "invalid optimistic credential revision",
    )
    .await;

    sqlx::query(
        "INSERT INTO admin_sessions (
             id, token_digest, admin_id, security_epoch, authenticated_factor,
             issued_at, expires_at, last_activity_at
         ) VALUES ($1::uuid, decode(repeat('b1', 32), 'hex'), $2::uuid, 0, 'passkey', now(),
                   now() + interval '1 hour', now())",
    )
    .bind(SESSION)
    .bind(ADMIN)
    .execute(&pool)
    .await
    .expect("insert finite admin session");
    assert_rejected(
        &pool,
        "INSERT INTO admin_sessions (
             id, token_digest, admin_id, security_epoch, authenticated_factor,
             issued_at, expires_at, last_activity_at
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000022', decode(repeat('b1', 32), 'hex'),
             '019a53a0-0000-7000-8000-000000000001', 0, 'passkey', now(),
             now() + interval '1 hour', now()
         )",
        "duplicate session digest",
    )
    .await;
    assert_rejected(
        &pool,
        "INSERT INTO admin_sessions (
             id, token_digest, admin_id, security_epoch, authenticated_factor,
             issued_at, expires_at, last_activity_at
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000023', decode(repeat('b2', 32), 'hex'),
             '019a53a0-0000-7000-8000-000000000001', 0, 'passkey', now(), now(), now()
         )",
        "non-finite session expiry",
    )
    .await;
    assert_rejected(
        &pool,
        "DELETE FROM admins WHERE id = '019a53a0-0000-7000-8000-000000000002'",
        "restricted credential/session history deletion",
    )
    .await;

    sqlx::query(
        "INSERT INTO admin_session_states (
             id, reference_digest, admin_id, browser_digest, session_id, purpose, phase,
             security_epoch, primary_verified_at, created_at, expires_at,
             verification_key_id, verification_nonce, verification_ciphertext,
             provisioning_key_id, provisioning_nonce, provisioning_ciphertext
         ) VALUES ($1::uuid, decode(repeat('c1', 32), 'hex'), $2::uuid,
                   decode(repeat('c2', 32), 'hex'), $3::uuid, 'addition', 'totp', 0,
                   now(), now(), now() + interval '10 minutes',
                   'key', '\\x04'::bytea, '\\x05'::bytea,
                   'key', '\\x06'::bytea, '\\x07'::bytea)",
    )
    .bind(PENDING)
    .bind(ADMIN)
    .bind(SESSION)
    .execute(&pool)
    .await
    .expect("insert encrypted TOTP enrollment state");
    assert_rejected(
        &pool,
        "INSERT INTO admin_session_states (
             id, reference_digest, admin_id, browser_digest, session_id, purpose, phase,
             security_epoch, primary_verified_at, created_at, expires_at,
             provisioning_key_id, provisioning_nonce, provisioning_ciphertext
         ) VALUES (
             '019a53a0-0000-7000-8000-000000000032', decode(repeat('c3', 32), 'hex'),
             '019a53a0-0000-7000-8000-000000000001', decode(repeat('c4', 32), 'hex'),
             '019a53a0-0000-7000-8000-000000000021', 'addition', 'primary_verified', 0,
             now(), now(), now() + interval '10 minutes', 'key', '\\x06'::bytea, '\\x07'::bytea
         )",
        "provisioning material on a primary proof",
    )
    .await;
    assert_rejected(
        &pool,
        "UPDATE admin_session_states SET consumed_at = now(), revoked_at = now() WHERE id = '019a53a0-0000-7000-8000-000000000031'",
        "multiple terminal pending-state transitions",
    )
    .await;
    let presented: u64 = sqlx::query(
        "UPDATE admin_session_states SET provisioning_presented_at = now()
         WHERE id = $1::uuid AND provisioning_presented_at IS NULL
           AND consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL",
    )
    .bind(PENDING)
    .execute(&pool)
    .await
    .expect("conditionally mark provisioning presentation")
    .rows_affected();
    assert_eq!(presented, 1);
    let redisplayed: u64 = sqlx::query(
        "UPDATE admin_session_states SET provisioning_presented_at = now()
         WHERE id = $1::uuid AND provisioning_presented_at IS NULL
           AND consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL",
    )
    .bind(PENDING)
    .execute(&pool)
    .await
    .expect("reject provisioning redisplay")
    .rows_affected();
    assert_eq!(redisplayed, 0);

    sqlx::query(
        "INSERT INTO admin_otp_challenges (
             id, admin_id, browser_digest, purpose, security_epoch, code_digest,
             issued_at, expires_at, failed_attempts, max_attempts, submission_category
         ) VALUES ($1::uuid, $2::uuid, decode(repeat('d1', 32), 'hex'), 'sign_in', 0,
                   decode(repeat('d2', 32), 'hex'), now(), now() + interval '5 minutes',
                   0, 3, 'accepted')",
    )
    .bind(OTP)
    .bind(ADMIN)
    .execute(&pool)
    .await
    .expect("insert digest-only OTP challenge");
    assert_rejected(
        &pool,
        "UPDATE admin_otp_challenges SET expires_at = issued_at WHERE id = '019a53a0-0000-7000-8000-000000000041'",
        "non-finite OTP expiry",
    )
    .await;
    assert_rejected(
        &pool,
        "UPDATE admin_otp_challenges SET failed_attempts = 4 WHERE id = '019a53a0-0000-7000-8000-000000000041'",
        "OTP attempts beyond the configured budget",
    )
    .await;
    let consumed: u64 = sqlx::query(
        "UPDATE admin_otp_challenges SET consumed_at = now()
         WHERE id = $1::uuid AND code_digest = decode(repeat('d2', 32), 'hex')
           AND failed_attempts = 0 AND failed_attempts < max_attempts
           AND consumed_at IS NULL AND superseded_at IS NULL AND revoked_at IS NULL",
    )
    .bind(OTP)
    .execute(&pool)
    .await
    .expect("compare-and-set OTP consumption")
    .rows_affected();
    assert_eq!(consumed, 1);

    sqlx::query(
        "INSERT INTO admin_rate_limits (
             scope, purpose, bucket_digest, window_started_at, window_expires_at,
             count, max_attempts
         ) VALUES ('subject', 'send', decode(repeat('e1', 32), 'hex'),
                   now(), now() + interval '1 minute', 0, 3)",
    )
    .execute(&pool)
    .await
    .expect("insert finite rate-limit window");
    assert_rejected(
        &pool,
        "INSERT INTO admin_rate_limits (
             scope, purpose, bucket_digest, window_started_at, window_expires_at,
             count, max_attempts
         ) SELECT scope, purpose, bucket_digest, window_started_at, window_expires_at,
                  count, max_attempts FROM admin_rate_limits",
        "duplicate rate-limit bucket/window",
    )
    .await;
    for expected in [1_i64, 2] {
        let count: i64 = sqlx::query_scalar(
            "UPDATE admin_rate_limits SET count = count + 1
             WHERE scope = 'subject' AND purpose = 'send'
               AND bucket_digest = decode(repeat('e1', 32), 'hex')
             RETURNING count",
        )
        .fetch_one(&pool)
        .await
        .expect("atomically increment rate-limit count");
        assert_eq!(count, expected);
    }

    pool.close().await;
}

async fn assert_columns(pool: &PgPool, table: &str, expected: &[&str]) {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name = $1",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .expect("read table columns");
    let actual: HashSet<&str> = rows.iter().map(String::as_str).collect();
    for column in expected {
        assert!(actual.contains(column), "{table}.{column} is missing");
    }
}

async fn seed_admin(pool: &PgPool, id: &str, digest_byte: u8) {
    sqlx::query(
        "INSERT INTO admins (
             id, phone_key_id, phone_nonce, phone_ciphertext, phone_lookup_digest,
             eligibility, security_epoch, created_by, created_at
         ) VALUES ($1::uuid, 'key', '\\x01'::bytea, '\\x02'::bytea, $2, 'active', 0, $1::uuid, now())",
    )
    .bind(id)
    .bind(vec![digest_byte; 32])
    .execute(pool)
    .await
    .expect("seed encrypted admin identity");
}

async fn assert_rejected(pool: &PgPool, statement: &'static str, behavior: &str) {
    let mut tx = pool.begin().await.expect("begin constraint assertion");
    let error = sqlx::query(statement)
        .execute(&mut *tx)
        .await
        .expect_err(behavior);
    assert!(
        error.as_database_error().is_some(),
        "{behavior}: expected a database constraint error"
    );
    tx.rollback().await.expect("roll back rejected mutation");
}
