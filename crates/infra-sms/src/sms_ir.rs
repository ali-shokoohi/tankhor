//! SMS.ir approved-template OTP transport.

use std::{fmt, time::Duration};

use admin::ports::{
    otp_challenge_repository::SubmissionCategory,
    sms_sender::{OtpSms, SmsSender},
};
use kernel::error::{DomainError, ErrorCode};
use serde::Serialize;

use crate::{http_category, response_body, transport_category};

pub struct SmsIrSender {
    client: reqwest::Client,
    base_url: String,
    api_key: reqwest::header::HeaderValue,
    template_id: u64,
    parameter: String,
}

#[derive(Serialize)]
struct Body<'a> {
    mobile: &'a str,
    #[serde(rename = "templateId")]
    template_id: u64,
    parameters: [Parameter<'a>; 1],
}

#[derive(Serialize)]
struct Parameter<'a> {
    name: &'a str,
    value: &'a str,
}

impl SmsIrSender {
    /// # Errors
    /// Rejects invalid endpoint/credentials/template configuration or HTTP client setup.
    pub fn new(
        base_url: &str,
        api_key: &str,
        template_id: u64,
        parameter: &str,
        timeout: Duration,
    ) -> Result<Self, DomainError> {
        let mut key = reqwest::header::HeaderValue::from_str(api_key)
            .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?;
        key.set_sensitive(true);
        if api_key.is_empty()
            || parameter.is_empty()
            || template_id == 0
            || reqwest::Url::parse(base_url).is_err()
        {
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
            api_key: key,
            template_id,
            parameter: parameter.to_owned(),
        })
    }

    /// Base URL without the sensitive API key, for local test fixture lookup.
    #[must_use]
    pub fn server_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait::async_trait]
impl SmsSender for SmsIrSender {
    async fn send_otp(&self, request: OtpSms) -> SubmissionCategory {
        let (Ok(recipient), Ok(code)) = (
            std::str::from_utf8(request.recipient.expose()),
            std::str::from_utf8(request.code.expose()),
        ) else {
            return SubmissionCategory::Rejected;
        };
        let response = self
            .client
            .post(format!("{}/v1/send/verify", self.base_url))
            .header("X-API-KEY", self.api_key.clone())
            .json(&Body {
                mobile: recipient,
                template_id: self.template_id,
                parameters: [Parameter {
                    name: &self.parameter,
                    value: code,
                }],
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
        match value.get("status").and_then(serde_json::Value::as_i64) {
            Some(1) => SubmissionCategory::Accepted,
            Some(_) => SubmissionCategory::Rejected,
            None => SubmissionCategory::Unknown,
        }
    }
}

impl fmt::Debug for SmsIrSender {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SmsIrSender([REDACTED])")
    }
}
