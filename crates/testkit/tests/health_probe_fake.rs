use testkit::contracts::health_probe::{HealthProbeContractHarness, health_probe_contract};
use testkit::fakes::health_probe::FakeHealthProbe;
use testkit::fakes::store::FakeStore;

#[tokio::test]
async fn health_probe_fake_passes_contract() {
    let default_store = FakeStore::new();
    let unseeded_store = FakeStore::new();
    let seeded_store = FakeStore::new();
    seeded_store.seed(|state| {
        state.health_ready = true;
    });

    let harness = HealthProbeContractHarness {
        unseeded_probe: FakeHealthProbe::new(unseeded_store),
        seeded_probe: FakeHealthProbe::new(seeded_store),
        unavailable_probe: FakeHealthProbe::unavailable(),
    };

    health_probe_contract(&FakeHealthProbe::new(default_store), &harness).await;
}
