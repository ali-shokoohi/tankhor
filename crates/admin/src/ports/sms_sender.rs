//! FR-002, FR-CON-02, Q09: one approved-template submission, not verification.

use super::otp_challenge_repository::SubmissionCategory;
use crate::domain::secrets::SecretBytes;
use kernel::ids::ChallengeId;
use time::OffsetDateTime;

/// Closed logical template; provider-specific approved IDs stay in adapter config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsTemplate {
    AdminOtp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpSms {
    pub challenge: ChallengeId,
    pub recipient: SecretBytes,
    pub code: SecretBytes,
    pub template: SmsTemplate,
    pub expires_at: OffsetDateTime,
}

/// An adapter performs a single bounded submission, never generates a new code,
/// resets expiry, retries via another provider, or treats acceptance as delivery.
/// The application owns eligible failover after the persistence transaction ends.
/// Ambiguous timeout/malformed responses are Unknown, never Accepted.
#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
#[async_trait::async_trait]
pub trait SmsSender: Send + Sync {
    async fn send_otp(&self, request: OtpSms) -> SubmissionCategory;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::secrets::SecretBytes,
        ports::otp_challenge_repository::SubmissionCategory as SubmissionOutcome,
    };
    use kernel::ids::ChallengeId;
    use std::str::FromStr;
    use time::{Duration, OffsetDateTime};

    #[tokio::test]
    async fn sms_sender_port_is_mockable() {
        let request = OtpSms {
            challenge: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap(),
            recipient: SecretBytes::new(b"+989120000001".to_vec()),
            code: SecretBytes::new(b"654321".to_vec()),
            template: SmsTemplate::AdminOtp,
            expires_at: OffsetDateTime::UNIX_EPOCH + Duration::MINUTE,
        };
        let expected = request.clone();
        let mut mock = MockSmsSender::new();
        mock.expect_send_otp()
            .times(4)
            .withf(move |actual| actual == &expected)
            .returning({
                let mut outcomes = [
                    SubmissionOutcome::Accepted,
                    SubmissionOutcome::Rejected,
                    SubmissionOutcome::Unavailable,
                    SubmissionOutcome::Unknown,
                ]
                .into_iter();
                move |_| outcomes.next().expect("four controlled submissions")
            });
        let port: &dyn SmsSender = &mock;
        for expected in [
            SubmissionOutcome::Accepted,
            SubmissionOutcome::Rejected,
            SubmissionOutcome::Unavailable,
            SubmissionOutcome::Unknown,
        ] {
            assert_eq!(port.send_otp(request.clone()).await, expected);
        }
        let debug = format!("{request:?}");
        assert!(!debug.contains("+989120000001"));
        assert!(!debug.contains("654321"));
    }
}
