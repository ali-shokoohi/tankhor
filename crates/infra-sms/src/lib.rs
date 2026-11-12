//! Approved-template SMS transports with closed delivery submission categories.

pub mod kavenegar;
pub mod sms_ir;

use admin::ports::otp_challenge_repository::SubmissionCategory;

const MAX_RESPONSE_BYTES: usize = 16 * 1024;

async fn response_body(response: reqwest::Response) -> Option<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return None;
    }
    let mut response = response;
    let mut body = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                    return None;
                }
                body.extend_from_slice(&chunk);
            }
            Ok(None) => return Some(body),
            Err(_) => return None,
        }
    }
}

fn transport_category(error: &reqwest::Error) -> SubmissionCategory {
    if error.is_timeout() {
        SubmissionCategory::Unknown
    } else {
        SubmissionCategory::Unavailable
    }
}

fn http_category(status: reqwest::StatusCode) -> Option<SubmissionCategory> {
    if status.is_success() {
        None
    } else if status == reqwest::StatusCode::REQUEST_TIMEOUT {
        Some(SubmissionCategory::Unknown)
    } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        Some(SubmissionCategory::Unavailable)
    } else {
        Some(SubmissionCategory::Rejected)
    }
}
