use infra_system::clock::SystemClock;
use testkit::contracts::clock::clock_contract;

#[tokio::test]
async fn clock_contract_real_system() {
    clock_contract(&SystemClock::new()).await;
}
