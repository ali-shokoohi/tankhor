//! Kavenegar approved-template OTP transport.

use std::{fmt, time::Duration};

use admin::{
    domain::secrets::SecretBytes,
    ports::{
        otp_challenge_repository::SubmissionCategory,
        sms_sender::{OtpSms, SmsSender},
    },
};
use kernel::error::{DomainError, ErrorCode};
use serde::Serialize;

use crate::{http_category, response_body, transport_category};

pub struct KavenegarSender {
    client: reqwest::Client,
    base_url: String,
    api_key: SecretBytes,
    template: String,
}

#[derive(Serialize)]
struct Form<'a> {
    receptor: &'a str,
    token: &'a str,
    template: &'a str,
    #[serde(rename = "type")]
    kind: &'static str,
}

impl KavenegarSender {
    /// # Errors
    /// Rejects an invalid endpoint, empty credentials/template or HTTP client configuration.
    pub fn new(
        base_url: &str,
        api_key: &str,
        template: &str,
        timeout: Duration,
    ) -> Result<Self, DomainError> {
        if api_key.is_empty() || template.is_empty() || reqwest::Url::parse(base_url).is_err() {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .build()
            .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key: SecretBytes::new(api_key.as_bytes().to_vec()),
            template: template.to_owned(),
        })
    }

    /// Base URL without the sensitive key path, for local test fixture lookup.
    #[must_use]
    pub fn server_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait::async_trait]
impl SmsSender for KavenegarSender {
    async fn send_otp(&self, request: OtpSms) -> SubmissionCategory {
        let (Ok(recipient), Ok(code), Ok(api_key)) = (
            std::str::from_utf8(request.recipient.expose()),
            std::str::from_utf8(request.code.expose()),
            std::str::from_utf8(self.api_key.expose()),
        ) else {
            return SubmissionCategory::Rejected;
        };
        let url = format!("{}/v1/{api_key}/verify/lookup.json", self.base_url);
        let response = self
            .client
            .post(url)
            .form(&Form {
                receptor: recipient,
                token: code,
                template: &self.template,
                kind: "sms",
            })
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => return transport_category(&error),
        };
        if let Some(category) = http_category(response.status()) {
            return category;
        }
        let Some(body) = response_body(response).await else {
            return SubmissionCategory::Unknown;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) else {
            return SubmissionCategory::Unknown;
        };
        match value
            .pointer("/return/status")
            .and_then(serde_json::Value::as_i64)
        {
            Some(200) => SubmissionCategory::Accepted,
            Some(_) => SubmissionCategory::Rejected,
            None => SubmissionCategory::Unknown,
        }
    }
}

impl fmt::Debug for KavenegarSender {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KavenegarSender([REDACTED])")
    }
}
