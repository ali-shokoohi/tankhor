use admin::application::health::{HealthReport, get_health};
use admin::ports::health_probe::{MockHealthProbe, ProbeReadiness};
use kernel::error::{DomainError, ErrorCategory, ErrorCode};
use testkit::fakes::health_probe::FakeHealthProbe;
use testkit::fakes::store::FakeStore;

#[tokio::test]
async fn get_health_calls_probe_once_and_fails_closed() {
    // 1. Positive mock interaction: calls HealthProbe::check exactly once when ready.
    let mut probe = MockHealthProbe::new();
    probe
        .expect_check()
        .times(1)
        .returning(|| Ok(ProbeReadiness::Ready));
    let report = get_health(&probe).await.expect("seeded probe is ready");
    assert_eq!(report, HealthReport::Ready);

    // 2. Unseeded probe fails closed as Unavailable (never converted to Ready), calling check once.
    let mut unseeded_mock = MockHealthProbe::new();
    unseeded_mock
        .expect_check()
        .times(1)
        .returning(|| Ok(ProbeReadiness::Unseeded));
    let unseeded_err = get_health(&unseeded_mock)
        .await
        .expect_err("unseeded probe must fail closed");
    assert_eq!(unseeded_err.category(), ErrorCategory::Unavailable);
    assert_eq!(unseeded_err.code(), ErrorCode::PrerequisiteUnavailable);

    // 3. Probe failure propagates sanitized Unavailable without retries.
    let mut unavailable_mock = MockHealthProbe::new();
    unavailable_mock
        .expect_check()
        .times(1)
        .returning(|| Err(DomainError::unavailable(ErrorCode::PrerequisiteUnavailable)));
    let probe_err = get_health(&unavailable_mock)
        .await
        .expect_err("unavailable probe must fail closed");
    assert_eq!(probe_err.category(), ErrorCategory::Unavailable);
    assert_eq!(probe_err.code(), ErrorCode::PrerequisiteUnavailable);

    // 4. Fake end-to-end verification: unseeded store fails closed; seeded store reports Ready;
    // unavailable fake fails closed.
    let store = FakeStore::new();
    let fake_probe = FakeHealthProbe::new(store.clone());
    let unseeded_fake_err = get_health(&fake_probe)
        .await
        .expect_err("unseeded fake store must not report ready");
    assert_eq!(unseeded_fake_err.category(), ErrorCategory::Unavailable);

    store.seed(|state| {
        state.health_ready = true;
    });
    assert_eq!(
        get_health(&fake_probe).await.expect("seeded fake store"),
        HealthReport::Ready
    );

    let offline_fake = FakeHealthProbe::unavailable();
    assert_eq!(
        get_health(&offline_fake)
            .await
            .expect_err("unavailable fake fails closed")
            .category(),
        ErrorCategory::Unavailable
    );
}
