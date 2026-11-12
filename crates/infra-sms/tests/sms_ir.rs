use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};

use admin::{
    domain::secrets::SecretBytes,
    ports::{
        otp_challenge_repository::SubmissionCategory,
        sms_sender::{OtpSms, SmsSender, SmsTemplate},
    },
};
use infra_sms::sms_ir::SmsIrSender;
use kernel::ids::ChallengeId;
use testkit::contracts::sms_sender::{SmsSenderContractHarness, sms_sender_contract};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::sync::Mutex;
use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{header, method, path},
};

const API_KEY: &str = "private-sms-ir-key";
const PARAMETER: &str = "OTP_CODE";
const TEMPLATE_ID: u64 = 123_456;

struct Harness {
    servers: Mutex<HashMap<String, Arc<MockServer>>>,
}

struct Outcomes(StdMutex<Vec<ResponseTemplate>>);

impl Respond for Outcomes {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let mut outcomes = self.0.lock().unwrap();
        if outcomes.len() > 1 {
            outcomes.remove(0)
        } else {
            outcomes[0].clone()
        }
    }
}

#[async_trait::async_trait]
impl SmsSenderContractHarness for Harness {
    type Sender = SmsIrSender;

    async fn sender(&self, outcomes: Vec<SubmissionCategory>) -> Self::Sender {
        let server = Arc::new(MockServer::start().await);
        let responses = outcomes
            .into_iter()
            .map(|outcome| match outcome {
                SubmissionCategory::Accepted => ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"status":1,"message":"accepted"})),
                SubmissionCategory::Rejected => ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"status":0,"message":"rejected"})),
                SubmissionCategory::Unavailable => ResponseTemplate::new(503),
                SubmissionCategory::Unknown => {
                    ResponseTemplate::new(200).set_body_string("malformed-private-response")
                }
            })
            .collect();
        Mock::given(method("POST"))
            .and(path("/v1/send/verify"))
            .and(header("x-api-key", API_KEY))
            .respond_with(Outcomes(StdMutex::new(responses)))
            .mount(&server)
            .await;
        let url = server.uri();
        self.servers.lock().await.insert(url.clone(), server);
        SmsIrSender::new(
            &url,
            API_KEY,
            TEMPLATE_ID,
            PARAMETER,
            Duration::from_millis(100),
        )
        .unwrap()
    }

    async fn captured(&self, sender: &Self::Sender) -> Vec<OtpSms> {
        let server = self
            .servers
            .lock()
            .await
            .get(sender.server_url())
            .unwrap()
            .clone();
        let requests = server.received_requests().await.unwrap();
        requests
            .iter()
            .map(|request| {
                let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
                assert_eq!(body["mobile"], "+989120000001");
                assert_eq!(body["templateId"], TEMPLATE_ID);
                assert_eq!(
                    body["parameters"],
                    serde_json::json!([{"name":PARAMETER,"value":"654321"}])
                );
                request_message()
            })
            .collect()
    }

    fn diagnostics(&self, sender: &Self::Sender) -> String {
        format!("{sender:?}")
    }
}

fn request_message() -> OtpSms {
    OtpSms {
        challenge: ChallengeId::from_str("019a53a0-0000-7000-8000-000000000001").unwrap(),
        recipient: SecretBytes::new(b"+989120000001".to_vec()),
        code: SecretBytes::new(b"654321".to_vec()),
        template: SmsTemplate::AdminOtp,
        expires_at: OffsetDateTime::UNIX_EPOCH + TimeDuration::MINUTE,
    }
}

#[tokio::test]
async fn sms_sender_contract_sms_ir() {
    let harness = Harness {
        servers: Mutex::new(HashMap::new()),
    };
    sms_sender_contract(&harness).await;
    let sender = harness.sender(vec![SubmissionCategory::Accepted]).await;
    assert!(!harness.diagnostics(&sender).contains(API_KEY));
}

#[tokio::test]
async fn sms_ir_refuses_redirects_and_classifies_timeouts_unknown() {
    let destination = MockServer::start().await;
    let redirect = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", destination.uri()))
        .mount(&redirect)
        .await;
    let sender = SmsIrSender::new(
        &redirect.uri(),
        API_KEY,
        TEMPLATE_ID,
        PARAMETER,
        Duration::from_millis(20),
    )
    .unwrap();
    assert_ne!(
        sender.send_otp(request_message()).await,
        SubmissionCategory::Accepted
    );
    assert!(destination.received_requests().await.unwrap().is_empty());

    let slow = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_json(serde_json::json!({"status":1})),
        )
        .mount(&slow)
        .await;
    let sender = SmsIrSender::new(
        &slow.uri(),
        API_KEY,
        TEMPLATE_ID,
        PARAMETER,
        Duration::from_millis(10),
    )
    .unwrap();
    assert_eq!(
        sender.send_otp(request_message()).await,
        SubmissionCategory::Unknown
    );
}
