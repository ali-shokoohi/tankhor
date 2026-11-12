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
        sms_sender::{OtpSms, SmsTemplate},
    },
};
use infra_sms::kavenegar::KavenegarSender;
use kernel::ids::ChallengeId;
use testkit::contracts::sms_sender::{SmsSenderContractHarness, sms_sender_contract};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::sync::Mutex;
use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{method, path},
};

const API_KEY: &str = "private-kavenegar-key";
const TEMPLATE: &str = "approved-admin-otp";

struct Harness {
    servers: Mutex<HashMap<String, Arc<MockServer>>>,
}

impl Harness {
    fn new() -> Self {
        Self {
            servers: Mutex::new(HashMap::new()),
        }
    }
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
    type Sender = KavenegarSender;

    async fn sender(&self, outcomes: Vec<SubmissionCategory>) -> Self::Sender {
        let server = Arc::new(MockServer::start().await);
        let responses = outcomes
            .into_iter()
            .map(|outcome| match outcome {
                SubmissionCategory::Accepted => ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"return":{"status":200,"message":"accepted"}}),
                ),
                SubmissionCategory::Rejected => ResponseTemplate::new(400).set_body_json(
                    serde_json::json!({"return":{"status":400,"message":"rejected"}}),
                ),
                SubmissionCategory::Unavailable => ResponseTemplate::new(503),
                SubmissionCategory::Unknown => {
                    ResponseTemplate::new(200).set_body_string("malformed-private-response")
                }
            })
            .collect();
        Mock::given(method("POST"))
            .and(path(format!("/v1/{API_KEY}/verify/lookup.json")))
            .respond_with(Outcomes(StdMutex::new(responses)))
            .mount(&server)
            .await;
        let url = server.uri();
        self.servers.lock().await.insert(url.clone(), server);
        KavenegarSender::new(&url, API_KEY, TEMPLATE, Duration::from_millis(100)).unwrap()
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
                let body = String::from_utf8(request.body.clone()).unwrap();
                assert!(
                    body.split('&')
                        .any(|field| field == "receptor=%2B989120000001")
                );
                assert!(body.split('&').any(|field| field == "token=654321"));
                assert!(
                    body.split('&')
                        .any(|field| field == "template=approved-admin-otp")
                );
                assert!(body.split('&').any(|field| field == "type=sms"));
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
async fn sms_sender_contract_kavenegar() {
    let harness = Harness::new();
    sms_sender_contract(&harness).await;
    let sender = harness.sender(vec![SubmissionCategory::Accepted]).await;
    let debug = harness.diagnostics(&sender);
    assert!(!debug.contains(API_KEY));
}

#[tokio::test]
async fn kavenegar_refuses_redirects_and_classifies_timeouts_unknown() {
    let destination = MockServer::start().await;
    let redirect = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", destination.uri()))
        .mount(&redirect)
        .await;
    let sender = KavenegarSender::new(
        &redirect.uri(),
        API_KEY,
        TEMPLATE,
        Duration::from_millis(20),
    )
    .unwrap();
    assert_ne!(
        admin::ports::sms_sender::SmsSender::send_otp(&sender, request_message()).await,
        SubmissionCategory::Accepted
    );
    assert!(destination.received_requests().await.unwrap().is_empty());

    let slow = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_json(serde_json::json!({"return":{"status":200}})),
        )
        .mount(&slow)
        .await;
    let sender =
        KavenegarSender::new(&slow.uri(), API_KEY, TEMPLATE, Duration::from_millis(10)).unwrap();
    assert_eq!(
        admin::ports::sms_sender::SmsSender::send_otp(&sender, request_message()).await,
        SubmissionCategory::Unknown
    );
}
