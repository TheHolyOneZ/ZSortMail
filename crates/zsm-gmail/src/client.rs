use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::model::{
    AttachmentBody, GmailMessage, HistoryPage, LabelList, MessageList, ModifyRequest, Profile, SendRequest, SentMessage,
};
use crate::oauth::Session;

pub const GMAIL_BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

#[derive(Clone)]
pub enum Credentials {
    Fixed(String),
    Session(Arc<Session>),
}

impl Credentials {
    async fn token(&self) -> Result<String> {
        match self {
            Self::Fixed(token) => Ok(token.clone()),
            Self::Session(session) => session.access_token().await,
        }
    }

    async fn renew(&self) -> Result<String> {
        match self {
            Self::Fixed(_) => Err(Error::NeedsRelink("the access token was rejected".into())),
            Self::Session(session) => session.force_refresh().await,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base: Duration,
    pub max: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 5, base: Duration::from_millis(500), max: Duration::from_secs(30) }
    }
}

impl RetryPolicy {
    pub fn delay(&self, attempt: u32) -> Duration {
        let doubled = self.base.saturating_mul(1u32 << attempt.min(10)).min(self.max);
        let jitter = doubled.as_millis() as f64 * 0.25 * fastrand::f64();
        doubled + Duration::from_millis(jitter as u64)
    }
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    #[serde(default)]
    message: String,
    #[serde(default)]
    errors: Vec<ApiErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    #[serde(default)]
    reason: String,
}

#[derive(Clone)]
pub struct GmailClient {
    http: reqwest::Client,
    base: String,
    creds: Credentials,
    retry: RetryPolicy,
    throttle: std::sync::Arc<Throttle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Full,
    Metadata,
}

pub fn build_http() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("ZSortMail/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .expect("an HTTP client with these options builds")
}

fn encode_path(segment: &str) -> String {
    segment
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32 & 0xff)
            }
        })
        .collect()
}

impl GmailClient {
    pub fn new(http: reqwest::Client, base: impl Into<String>, creds: Credentials, retry: RetryPolicy) -> Self {
        Self {
            http,
            base: base.into().trim_end_matches('/').to_string(),
            creds,
            retry,
            throttle: std::sync::Arc::new(Throttle::unlimited()),
        }
    }

    #[must_use]
    pub fn with_throttle(mut self, throttle: std::sync::Arc<Throttle>) -> Self {
        self.throttle = throttle;
        self
    }

    pub fn gmail(creds: Credentials) -> Self {
        Self::new(build_http(), GMAIL_BASE, creds, RetryPolicy::default())
    }

    async fn send(&self, build: impl Fn(&reqwest::Client) -> RequestBuilder) -> Result<Response> {
        let mut renewed = false;
        let mut attempt = 0;
        loop {
            let token = self.creds.token().await?;
            let result = match build(&self.http).bearer_auth(&token).build() {
                Ok(request) => {
                    self.throttle.acquire(quota_cost(request.method(), request.url().path())).await;
                    self.http.execute(request).await
                }
                Err(error) => Err(error),
            };
            let response = match result {
                Ok(response) => response,
                Err(error)
                    if (error.is_connect() || error.is_timeout() || error.is_request())
                        && attempt + 1 < self.retry.max_attempts =>
                {
                    tokio::time::sleep(self.retry.delay(attempt)).await;
                    attempt += 1;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let status = response.status();
            if status.is_success() {
                return Ok(response);
            }
            if status == StatusCode::UNAUTHORIZED {
                if renewed {
                    return Err(Error::NeedsRelink("Google rejected the saved sign-in".into()));
                }
                renewed = true;
                self.creds.renew().await?;
                continue;
            }
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            let body = response.text().await.unwrap_or_default();
            let parsed: Option<ApiErrorBody> = serde_json::from_str(&body).ok();
            let reason =
                parsed.as_ref().and_then(|b| b.error.errors.first()).map(|d| d.reason.clone()).unwrap_or_default();
            let message = parsed.map(|b| b.error.message).filter(|m| !m.is_empty()).unwrap_or(body);

            if status == StatusCode::FORBIDDEN
                && (reason == "insufficientPermissions"
                    || message.to_lowercase().contains("insufficient authentication scopes")
                    || message.to_lowercase().contains("insufficient permission"))
            {
                return Err(Error::MissingPermission(message));
            }

            let rate_limited = status == StatusCode::TOO_MANY_REQUESTS
                || (status == StatusCode::FORBIDDEN
                    && matches!(reason.as_str(), "rateLimitExceeded" | "userRateLimitExceeded"));
            if rate_limited {
                self.throttle.back_off(retry_after.unwrap_or(Duration::from_secs(45)));
                tracing::warn!(status = status.as_u16(), %reason, %message, ?retry_after, "Gmail is rate limiting this account");
            }
            let transient = rate_limited || status.is_server_error();
            if transient && attempt + 1 < self.retry.max_attempts {
                let wait = retry_after.unwrap_or_else(|| self.retry.delay(attempt)).min(self.retry.max);
                tokio::time::sleep(wait).await;
                attempt += 1;
                continue;
            }
            return Err(if rate_limited {
                Error::RateLimited { retry_after }
            } else {
                Error::Api { status: status.as_u16(), reason, message }
            });
        }
    }

    async fn json<T: DeserializeOwned>(&self, build: impl Fn(&reqwest::Client) -> RequestBuilder) -> Result<T> {
        let response = self.send(build).await?;
        Ok(response.json::<T>().await?)
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str, query: &[(&str, String)]) -> Result<T> {
        self.json(|http| http.get(url).query(query)).await
    }

    pub async fn profile(&self) -> Result<Profile> {
        let url = format!("{}/profile", self.base);
        self.json(|http| http.get(&url)).await
    }

    pub async fn list_messages(&self, query: Option<&str>, page_token: Option<&str>, max: u32) -> Result<MessageList> {
        let url = format!("{}/messages", self.base);
        let max = max.to_string();
        self.json(|http| {
            let mut request = http.get(&url).query(&[("maxResults", max.as_str()), ("includeSpamTrash", "false")]);
            if let Some(q) = query {
                request = request.query(&[("q", q)]);
            }
            if let Some(token) = page_token {
                request = request.query(&[("pageToken", token)]);
            }
            request
        })
        .await
    }

    pub async fn get_message(&self, id: &str, format: Format) -> Result<GmailMessage> {
        let url = format!("{}/messages/{}", self.base, encode_path(id));
        self.json(|http| {
            let request = http.get(&url);
            match format {
                Format::Full => request.query(&[("format", "full")]),
                Format::Metadata => {
                    let mut request = request.query(&[("format", "metadata")]);
                    for name in crate::convert::METADATA_HEADERS {
                        request = request.query(&[("metadataHeaders", *name)]);
                    }
                    request
                }
            }
        })
        .await
    }

    pub async fn history(&self, start_history_id: &str, page_token: Option<&str>) -> Result<HistoryPage> {
        let url = format!("{}/history", self.base);
        let result = self
            .json::<HistoryPage>(|http| {
                let mut request = http.get(&url).query(&[("startHistoryId", start_history_id), ("maxResults", "500")]);
                for kind in ["messageAdded", "messageDeleted", "labelAdded", "labelRemoved"] {
                    request = request.query(&[("historyTypes", kind)]);
                }
                if let Some(token) = page_token {
                    request = request.query(&[("pageToken", token)]);
                }
                request
            })
            .await;
        match result {
            Err(Error::Api { status: 404, .. }) => Err(Error::HistoryGone),
            other => other,
        }
    }

    pub async fn labels(&self) -> Result<LabelList> {
        let url = format!("{}/labels", self.base);
        self.json(|http| http.get(&url)).await
    }

    pub async fn create_label(&self, name: &str) -> Result<crate::model::Label> {
        let url = format!("{}/labels", self.base);
        let body = serde_json::json!({
            "name": name,
            "labelListVisibility": "labelShow",
            "messageListVisibility": "show",
        });
        self.json(|http| http.request(Method::POST, &url).json(&body)).await
    }

    pub async fn modify_thread(&self, thread_id: &str, add: &[String], remove: &[String]) -> Result<()> {
        let url = format!("{}/threads/{}/modify", self.base, encode_path(thread_id));
        let body =
            serde_json::to_value(ModifyRequest { add_label_ids: add, remove_label_ids: remove }).unwrap_or_default();
        self.send(|http| http.request(Method::POST, &url).json(&body)).await.map(drop)
    }

    pub async fn modify_message(&self, message_id: &str, add: &[String], remove: &[String]) -> Result<()> {
        let url = format!("{}/messages/{}/modify", self.base, encode_path(message_id));
        let body =
            serde_json::to_value(ModifyRequest { add_label_ids: add, remove_label_ids: remove }).unwrap_or_default();
        self.send(|http| http.request(Method::POST, &url).json(&body)).await.map(drop)
    }

    pub async fn trash_thread(&self, thread_id: &str) -> Result<()> {
        let url = format!("{}/threads/{}/trash", self.base, encode_path(thread_id));
        self.send(|http| http.request(Method::POST, &url)).await.map(drop)
    }

    pub async fn untrash_thread(&self, thread_id: &str) -> Result<()> {
        let url = format!("{}/threads/{}/untrash", self.base, encode_path(thread_id));
        self.send(|http| http.request(Method::POST, &url)).await.map(drop)
    }

    pub async fn send_raw(&self, raw: &[u8], thread_id: Option<&str>) -> Result<SentMessage> {
        let url = format!("{}/messages/send", self.base);
        let body =
            serde_json::to_value(SendRequest { raw: URL_SAFE_NO_PAD.encode(raw), thread_id }).unwrap_or_default();
        self.json(|http| http.request(Method::POST, &url).json(&body)).await
    }

    pub async fn attachment(&self, message_id: &str, attachment_id: &str) -> Result<Vec<u8>> {
        let url =
            format!("{}/messages/{}/attachments/{}", self.base, encode_path(message_id), encode_path(attachment_id));
        let body: AttachmentBody = self.json(|http| http.get(&url)).await?;
        URL_SAFE_NO_PAD
            .decode(body.data.trim_end_matches('='))
            .map_err(|e| Error::Unexpected(format!("attachment data is not base64: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_is_capped_and_never_waits_less_than_planned() {
        let policy = RetryPolicy { max_attempts: 5, base: Duration::from_millis(100), max: Duration::from_secs(2) };
        for attempt in 0..8 {
            let planned = Duration::from_millis(100).saturating_mul(1 << attempt).min(Duration::from_secs(2));
            let actual = policy.delay(attempt);
            assert!(
                actual >= planned && actual <= planned + planned / 4 + Duration::from_millis(1),
                "attempt {attempt}: {actual:?}"
            );
        }
    }

    #[test]
    fn path_segments_cannot_break_out_of_the_url() {
        assert_eq!(encode_path("18c0ffee"), "18c0ffee");
        assert_eq!(encode_path("../x/y?z"), "..%2Fx%2Fy%3Fz");
    }
}

fn quota_cost(method: &Method, path: &str) -> f64 {
    let _ = method;
    if path.contains("/messages/send") {
        100.0
    } else if path.contains("/history") {
        2.0
    } else if path.ends_with("/profile") || path.ends_with("/labels") {
        1.0
    } else if path.contains("/threads/") {
        10.0
    } else {
        5.0
    }
}

#[derive(Debug)]
pub struct Throttle {
    rate: f64,
    capacity: f64,
    state: std::sync::Mutex<(f64, std::time::Instant)>,
}

impl Throttle {
    pub fn unlimited() -> Self {
        Self { rate: 0.0, capacity: 0.0, state: std::sync::Mutex::new((0.0, std::time::Instant::now())) }
    }

    pub fn per_minute(units_per_minute: f64, burst: f64) -> Self {
        Self {
            rate: units_per_minute / 60.0,
            capacity: burst,
            state: std::sync::Mutex::new((burst, std::time::Instant::now())),
        }
    }

    pub async fn acquire(&self, cost: f64) {
        if self.rate <= 0.0 {
            return;
        }
        let cost = cost.min(self.capacity);
        loop {
            let wait = {
                let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                let now = std::time::Instant::now();
                let gained = now.duration_since(state.1).as_secs_f64() * self.rate;
                state.0 = (state.0 + gained).min(self.capacity);
                state.1 = now;
                if state.0 >= cost {
                    state.0 -= cost;
                    return;
                }
                Duration::from_secs_f64((cost - state.0) / self.rate)
            };
            tokio::time::sleep(wait.max(Duration::from_millis(5))).await;
        }
    }

    pub fn back_off(&self, for_how_long: Duration) {
        if self.rate <= 0.0 {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 = -(for_how_long.as_secs_f64() * self.rate);
        state.1 = std::time::Instant::now();
    }
}

#[cfg(test)]
mod throttle_tests {
    use super::*;

    #[tokio::test]
    async fn calls_beyond_the_budget_wait_instead_of_being_sent() {
        let throttle = Throttle::per_minute(6000.0, 50.0);
        let start = std::time::Instant::now();
        for _ in 0..10 {
            throttle.acquire(10.0).await;
        }
        let took = start.elapsed();
        assert!(took >= Duration::from_millis(400), "waited {took:?}");
        assert!(took < Duration::from_millis(1500), "waited {took:?}");
    }

    #[tokio::test]
    async fn unlimited_never_waits_and_a_back_off_makes_the_next_call_wait() {
        let free = Throttle::unlimited();
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            free.acquire(100.0).await;
        }
        assert!(start.elapsed() < Duration::from_millis(100));

        let throttle = Throttle::per_minute(6000.0, 500.0);
        throttle.back_off(Duration::from_millis(300));
        let start = std::time::Instant::now();
        throttle.acquire(5.0).await;
        assert!(start.elapsed() >= Duration::from_millis(250), "{:?}", start.elapsed());
    }

    #[test]
    fn calls_are_charged_what_gmail_charges() {
        assert_eq!(quota_cost(&Method::GET, "/gmail/v1/users/me/messages/abc"), 5.0);
        assert_eq!(quota_cost(&Method::GET, "/gmail/v1/users/me/history"), 2.0);
        assert_eq!(quota_cost(&Method::POST, "/gmail/v1/users/me/messages/send"), 100.0);
        assert_eq!(quota_cost(&Method::POST, "/gmail/v1/users/me/threads/t/modify"), 10.0);
        assert_eq!(quota_cost(&Method::GET, "/gmail/v1/users/me/profile"), 1.0);
    }
}
