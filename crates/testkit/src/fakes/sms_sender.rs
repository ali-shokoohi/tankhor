//! Captures synthetic submissions without exposing payloads through diagnostics.

use admin::ports::{
    otp_challenge_repository::SubmissionCategory,
    sms_sender::{OtpSms, SmsSender},
};
use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct State {
    outcomes: VecDeque<SubmissionCategory>,
    captured: Vec<OtpSms>,
}

#[derive(Clone)]
pub struct FakeSmsSender {
    state: Arc<Mutex<State>>,
}

impl FakeSmsSender {
    #[must_use]
    pub fn new(outcomes: Vec<SubmissionCategory>) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                outcomes: outcomes.into(),
                captured: Vec::new(),
            })),
        }
    }

    /// Explicit inspection for synthetic tests only; never logs payloads.
    /// # Panics
    /// Panics if the fixture mutex is poisoned.
    #[must_use]
    pub fn captured(&self) -> Vec<OtpSms> {
        self.state
            .lock()
            .expect("SMS fixture mutex poisoned")
            .captured
            .clone()
    }
}

impl fmt::Debug for FakeSmsSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FakeSmsSender([REDACTED])")
    }
}

#[async_trait::async_trait]
impl SmsSender for FakeSmsSender {
    async fn send_otp(&self, request: OtpSms) -> SubmissionCategory {
        let mut state = self.state.lock().expect("SMS fixture mutex poisoned");
        state.captured.push(request);
        state
            .outcomes
            .pop_front()
            .unwrap_or(SubmissionCategory::Unavailable)
    }
}
