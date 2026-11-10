use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::fixtures::database::{
    FixtureDatabase, database_exists_on_server, verify_database_absent,
};

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn sqlx_cases_get_distinct_migrated_databases(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let db = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect fixture database helper");

    let db_name = db.database_name().to_owned();
    assert!(!db_name.is_empty());

    db.verify_migrated_and_role_separation()
        .await
        .expect("database must have canonical migration metadata and restricted runtime role");

    let runtime_pool = db
        .connect_runtime_pool(2)
        .await
        .expect("connect restricted runtime pool");
    let current_user: String = sqlx::query_scalar("SELECT current_user")
        .fetch_one(&runtime_pool)
        .await
        .expect("query runtime user");
    assert_eq!(current_user, "tankhor_fixture_runtime");

    db.verify_pool_cleanup_and_failed_retention_semantics()
        .await
        .expect("pool cleanup and failed-database retention semantics must hold");

    runtime_pool.close().await;
    let server_options = db.server_admin_options();
    db.close().await;

    // Spawn a separate inner module #[sqlx::test] harness via a dedicated helper function
    // invoked only inside our child subprocesses so nextest never races with it.
    let current_exe = std::env::current_exe().expect("current test binary path");

    // 1. Distinct migrated database + clean drop after passing case closes its pools.
    let distinct_run = std::process::Command::new(&current_exe)
        .args([
            "--exact",
            "sqlx_subcase_driver_only_when_requested",
            "--nocapture",
        ])
        .env("TANKHOR_SQLX_SUBCASE_INNER", "distinct")
        .output()
        .expect("spawn distinct child probe");
    assert!(
        distinct_run.status.success(),
        "distinct child probe must succeed: {}",
        String::from_utf8_lossy(&distinct_run.stderr)
    );
    let peer_db = extract_probe_db(&distinct_run, "TANKHOR_PEER_DB=");
    assert_ne!(
        db_name, peer_db,
        "different SQLx test paths must derive distinct fresh databases"
    );
    verify_database_absent(&server_options, &peer_db)
        .await
        .expect("passing SQLx case must drop its database after closing extra pools");

    // 2. Failing case retains its database on the server until the same test path is rerun.
    let fail_run = std::process::Command::new(&current_exe)
        .args([
            "--exact",
            "sqlx_subcase_driver_only_when_requested",
            "--nocapture",
        ])
        .env("TANKHOR_SQLX_SUBCASE_INNER", "fail")
        .output()
        .expect("spawn failing child probe");
    assert!(
        !fail_run.status.success(),
        "failing child probe must exit non-zero"
    );
    let failed_db = extract_probe_db(&fail_run, "TANKHOR_PEER_DB=");
    assert_eq!(
        peer_db, failed_db,
        "same test path must derive deterministic SQLx database name"
    );
    assert!(
        database_exists_on_server(&server_options, &failed_db)
            .await
            .expect("check retained failed database"),
        "failed SQLx test database must remain inspectable until same-case rerun"
    );

    // 3. Rerunning the exact same test path replaces the retained database with a fresh migrated DB
    // and drops it upon success.
    let pass_run = std::process::Command::new(&current_exe)
        .args([
            "--exact",
            "sqlx_subcase_driver_only_when_requested",
            "--nocapture",
        ])
        .env("TANKHOR_SQLX_SUBCASE_INNER", "pass")
        .output()
        .expect("spawn passing rerun child probe");
    assert!(
        pass_run.status.success(),
        "passing rerun must succeed: {}",
        String::from_utf8_lossy(&pass_run.stderr)
    );
    verify_database_absent(&server_options, &failed_db)
        .await
        .expect("same-case passing rerun must replace and drop the previously retained database");
}

#[test]
fn sqlx_subcase_driver_only_when_requested() {
    use sqlx::testing::TestTermination as _;
    if std::env::var_os("TANKHOR_SQLX_SUBCASE_INNER").is_none() {
        return;
    }
    let mut args = sqlx::testing::TestArgs::new("infra_postgres::subcase::peer_isolated_case");
    args.migrator(&infra_postgres::MIGRATOR);
    let f: fn(PgPoolOptions, PgConnectOptions) -> _ = inner_sqlx_case;
    let result = sqlx::testing::TestFn::run_test(f, args);
    assert!(result.is_success(), "inner SQLx test must succeed");
}

async fn inner_sqlx_case(
    pool_opts: PgPoolOptions,
    conn_opts: PgConnectOptions,
) -> Result<(), sqlx::Error> {
    let mode = std::env::var("TANKHOR_SQLX_SUBCASE_INNER").unwrap_or_default();
    let db = FixtureDatabase::connect(pool_opts, conn_opts)
        .await
        .expect("connect inner SQLx test database");
    let name = db.database_name().to_owned();
    println!("TANKHOR_PEER_DB={name}");
    if mode == "distinct" {
        db.verify_migrated_and_role_separation()
            .await
            .expect("inner case must be migrated with role separation");
        db.close().await;
        return Ok(());
    }
    let admin_pool = db.admin_pool().clone();
    if mode == "fail" {
        sqlx::query("CREATE TABLE retained_failure_marker (marker TEXT NOT NULL)")
            .execute(&admin_pool)
            .await?;
        sqlx::query("INSERT INTO retained_failure_marker (marker) VALUES ('retained')")
            .execute(&admin_pool)
            .await?;
        db.close().await;
        panic!("deliberate failure to verify SQLx failed-database retention");
    }
    let leftover: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM information_schema.tables
            WHERE table_schema = 'public' AND table_name = 'retained_failure_marker'
        )",
    )
    .fetch_one(&admin_pool)
    .await?;
    assert!(
        !leftover,
        "same-case rerun must replace the previously failed database with a fresh one"
    );
    db.close().await;
    Ok(())
}

fn extract_probe_db(output: &std::process::Output, prefix: &str) -> String {
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    combined
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .expect("probe must emit its generated SQLx database name")
        .trim()
        .to_owned()
}
