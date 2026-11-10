use infra_system::id_generator::SystemIdGenerator;
use testkit::contracts::id_generator::id_generator_contract;

#[tokio::test]
async fn id_generator_contract_real_system() {
    id_generator_contract(&SystemIdGenerator::new()).await;
}
