//! FR-002, Q09: controlled transport outcomes and unchanged failover payloads.

use admin::{
    domain::secrets::SecretBytes,
    ports::{
        otp_challenge_repository::SubmissionCategory,
        sms_sender::{OtpSms, SmsSender, SmsTemplate},
    },
};
use kernel::ids::ChallengeId;
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

/// Adapters configure local wiremock responses; fakes configure a finite queue.
#[async_trait::async_trait]
pub trait SmsSenderContractHarness: Sync {
    type Sender: SmsSender;
    async fn sender(&self, outcomes: Vec<SubmissionCategory>) -> Self::Sender;
    async fn captured(&self, sender: &Self::Sender) -> Vec<OtpSms>;
    fn diagnostics(&self, sender: &Self::Sender) -> String;
}

/// # Panics
/// Panics on changed payloads, misclassified outcomes or leaked diagnostics.
pub async fn sms_sender_contract<H: SmsSenderContractHarness>(h: &H) {
    use SubmissionCategory::{Accepted, Rejected, Unavailable, Unknown};
    let primary = h
        .sender(vec![Unavailable, Rejected, Unknown, Accepted])
        .await;
    let fallback = h.sender(vec![Accepted]).await;
    let request = OtpSms {
        challenge: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap(),
        recipient: SecretBytes::new(b"+989120000001".to_vec()),
        code: SecretBytes::new(b"654321".to_vec()),
        template: SmsTemplate::AdminOtp,
        expires_at: OffsetDateTime::UNIX_EPOCH + Duration::MINUTE,
    };
    assert_eq!(primary.send_otp(request.clone()).await, Unavailable);
    // Sender contract checks payload fidelity; application owns fallback eligibility.
    assert_eq!(fallback.send_otp(request.clone()).await, Accepted);
    for outcome in [Rejected, Unknown, Accepted] {
        assert_eq!(primary.send_otp(request.clone()).await, outcome);
    }
    assert_eq!(h.captured(&primary).await, vec![request.clone(); 4]);
    assert_eq!(h.captured(&fallback).await, vec![request.clone()]);
    let debug = format!(
        "{:?} {} {}",
        h.captured(&primary).await,
        h.diagnostics(&primary),
        h.diagnostics(&fallback)
    );
    assert!(!debug.contains("+989120000001"));
    assert!(!debug.contains("654321"));
}
