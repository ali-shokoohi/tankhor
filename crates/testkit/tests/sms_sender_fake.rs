use admin::ports::{
    otp_challenge_repository::SubmissionCategory,
    sms_sender::{OtpSms, SmsSender},
};
use testkit::{
    contracts::sms_sender::{SmsSenderContractHarness, sms_sender_contract},
    fakes::sms_sender::FakeSmsSender,
};

struct Harness;
#[async_trait::async_trait]
impl SmsSenderContractHarness for Harness {
    type Sender = FakeSmsSender;
    async fn sender(&self, outcomes: Vec<SubmissionCategory>) -> Self::Sender {
        FakeSmsSender::new(outcomes)
    }
    async fn captured(&self, sender: &Self::Sender) -> Vec<OtpSms> {
        sender.captured()
    }
    fn diagnostics(&self, sender: &Self::Sender) -> String {
        format!("{sender:?}")
    }
}

#[tokio::test]
async fn sms_sender_fake_passes_contract() {
    sms_sender_contract(&Harness).await;
}

#[tokio::test]
async fn sms_sender_fake_exhaustion_is_never_acceptance() {
    use admin::{domain::secrets::SecretBytes, ports::sms_sender::SmsTemplate};
    let sender = FakeSmsSender::new(vec![]);
    let request = OtpSms {
        challenge: "019a53a0-0000-7000-8000-000000000001".parse().unwrap(),
        recipient: SecretBytes::new(b"+989120000001".to_vec()),
        code: SecretBytes::new(b"654321".to_vec()),
        template: SmsTemplate::AdminOtp,
        expires_at: time::OffsetDateTime::UNIX_EPOCH,
    };
    assert_eq!(
        sender.send_otp(request.clone()).await,
        SubmissionCategory::Unavailable
    );
    assert_eq!(sender.captured(), vec![request]);
}
