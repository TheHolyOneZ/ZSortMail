use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

use crate::error::{Error, Result};

pub const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const SCOPES: &str = "openid email https://www.googleapis.com/auth/gmail.modify";
pub const CONTACTS_SCOPE: &str = "https://www.googleapis.com/auth/contacts.readonly";

#[derive(Debug, Clone)]
pub struct OAuthClient {
    pub client_id: String,
    pub client_secret: String,
    pub auth_url: String,
    pub token_url: String,
}

impl OAuthClient {
    pub fn google(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            auth_url: GOOGLE_AUTH_URL.into(),
            token_url: GOOGLE_TOKEN_URL.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    pub access_token: String,
    pub expires_at_ms: i64,
    pub refresh_token: Option<String>,
    pub scope: String,
    pub id_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: i64,
    refresh_token: Option<String>,
    #[serde(default)]
    scope: String,
    id_token: Option<String>,
}

#[derive(Deserialize)]
struct TokenError {
    error: String,
    #[serde(default)]
    error_description: String,
}

fn random_url_safe(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    for chunk in buf.chunks_mut(8) {
        let word = rand::random::<u64>().to_le_bytes();
        chunk.copy_from_slice(&word[..chunk.len()]);
    }
    URL_SAFE_NO_PAD.encode(buf)
}

pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub struct PendingLink {
    pub auth_url: String,
    listener: TcpListener,
    verifier: String,
    state: String,
    redirect_uri: String,
    client: OAuthClient,
}

pub async fn begin(client: &OAuthClient) -> Result<PendingLink> {
    begin_with(client, SCOPES, true).await
}

pub async fn begin_with(client: &OAuthClient, scope: &str, offline: bool) -> Result<PendingLink> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    let verifier = random_url_safe(32);
    let state = random_url_safe(16);

    let mut url = Url::parse(&client.auth_url).map_err(|e| Error::Auth(format!("bad sign-in address: {e}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", &client.client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", scope)
        .append_pair("code_challenge", &challenge_for(&verifier))
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state);
    if offline {
        url.query_pairs_mut().append_pair("access_type", "offline").append_pair("prompt", "consent");
    }
    Ok(PendingLink { auth_url: url.into(), listener, verifier, state, redirect_uri, client: client.clone() })
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>ZSortMail</title>\
<body style=\"font:16px system-ui,sans-serif;margin:15vh auto;max-width:30em;padding:0 1em;color:#172135\">\
<h1 style=\"font-size:1.4em\">{TITLE}</h1><p>{BODY}</p></body>";

async fn answer(stream: &mut TcpStream, status: &str, title: &str, body: &str) {
    let page = DONE_PAGE.replace("{TITLE}", title).replace("{BODY}", body);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

enum Callback {
    Code(String),
    Denied(String),
}

async fn read_callback(stream: &mut TcpStream, expected_state: &str) -> Option<Callback> {
    let mut buffer = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while !buffer.windows(4).any(|w| w == b"\r\n\r\n") && buffer.len() < 16 * 1024 {
        match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut chunk)).await {
            Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
            Ok(Ok(n)) => buffer.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&buffer);
    let target = head.lines().next()?.strip_prefix("GET ")?.split(' ').next()?.to_string();
    let url = Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    let mut code = None;
    let mut error = None;
    let mut state = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            _ => {}
        }
    }
    if code.is_none() && error.is_none() {
        return None;
    }
    if state.as_deref() != Some(expected_state) {
        return Some(Callback::Denied("The sign-in answer did not match the request.".into()));
    }
    match (code, error) {
        (_, Some(error)) => Some(Callback::Denied(match error.as_str() {
            "access_denied" => "Sign-in was cancelled.".into(),
            other => format!("Google reported: {other}"),
        })),
        (Some(code), None) => Some(Callback::Code(code)),
        (None, None) => None,
    }
}

impl PendingLink {
    pub async fn finish(self, http: &reqwest::Client, timeout: Duration, now_ms: impl Fn() -> i64) -> Result<Tokens> {
        let wait = async {
            loop {
                let (mut stream, _) = self.listener.accept().await?;
                match read_callback(&mut stream, &self.state).await {
                    Some(Callback::Code(code)) => {
                        answer(
                            &mut stream,
                            "200 OK",
                            "ZSortMail is linked to your Gmail",
                            "You can close this tab and go back to the app.",
                        )
                        .await;
                        return Ok::<_, Error>(code);
                    }
                    Some(Callback::Denied(message)) => {
                        answer(&mut stream, "400 Bad Request", "Sign-in did not finish", &message).await;
                        return Err(Error::Auth(message));
                    }
                    None => answer(&mut stream, "404 Not Found", "Nothing here", "").await,
                }
            }
        };
        let code = tokio::time::timeout(timeout, wait)
            .await
            .map_err(|_| Error::Auth("Sign-in took too long. Start again when you are ready.".into()))??;

        let form = [
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("client_id", self.client.client_id.as_str()),
            ("client_secret", self.client.client_secret.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("code_verifier", self.verifier.as_str()),
        ];
        token_request(http, &self.client.token_url, &form, now_ms()).await
    }
}

async fn token_request(http: &reqwest::Client, url: &str, form: &[(&str, &str)], now_ms: i64) -> Result<Tokens> {
    let response = http.post(url).form(form).send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let parsed: Option<TokenError> = serde_json::from_str(&body).ok();
        return Err(match parsed {
            Some(e) if e.error == "invalid_grant" => Error::NeedsRelink(e.error_description),
            Some(e) => Error::Auth(format!("Google refused the sign-in ({}): {}", e.error, e.error_description)),
            None => Error::Api { status: status.as_u16(), reason: "token".into(), message: body },
        });
    }
    let parsed: TokenResponse = response.json().await?;
    Ok(Tokens {
        access_token: parsed.access_token,
        expires_at_ms: now_ms + parsed.expires_in.max(0) * 1000,
        refresh_token: parsed.refresh_token,
        scope: parsed.scope,
        id_token: parsed.id_token,
    })
}

pub async fn refresh(client: &OAuthClient, http: &reqwest::Client, refresh_token: &str, now_ms: i64) -> Result<Tokens> {
    let form = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client.client_id.as_str()),
        ("client_secret", client.client_secret.as_str()),
    ];
    let mut tokens = token_request(http, &client.token_url, &form, now_ms).await?;
    tokens.refresh_token.get_or_insert_with(|| refresh_token.to_string());
    Ok(tokens)
}

pub type OnRotate = Arc<dyn Fn(&str) + Send + Sync>;

pub struct Session {
    client: OAuthClient,
    http: reqwest::Client,
    tokens: tokio::sync::Mutex<Tokens>,
    now: Arc<dyn Fn() -> i64 + Send + Sync>,
    on_rotate: Option<OnRotate>,
}

impl Session {
    pub fn new(
        client: OAuthClient,
        http: reqwest::Client,
        tokens: Tokens,
        now: Arc<dyn Fn() -> i64 + Send + Sync>,
        on_rotate: Option<OnRotate>,
    ) -> Arc<Self> {
        Arc::new(Self { client, http, tokens: tokio::sync::Mutex::new(tokens), now, on_rotate })
    }

    pub fn from_refresh_token(
        client: OAuthClient,
        http: reqwest::Client,
        refresh_token: String,
        now: Arc<dyn Fn() -> i64 + Send + Sync>,
        on_rotate: Option<OnRotate>,
    ) -> Arc<Self> {
        let tokens = Tokens {
            access_token: String::new(),
            expires_at_ms: 0,
            refresh_token: Some(refresh_token),
            scope: String::new(),
            id_token: None,
        };
        Self::new(client, http, tokens, now, on_rotate)
    }

    pub async fn access_token(&self) -> Result<String> {
        let mut tokens = self.tokens.lock().await;
        if tokens.access_token.is_empty() || tokens.expires_at_ms - 60_000 <= (self.now)() {
            self.renew(&mut tokens).await?;
        }
        Ok(tokens.access_token.clone())
    }

    pub async fn force_refresh(&self) -> Result<String> {
        let mut tokens = self.tokens.lock().await;
        self.renew(&mut tokens).await?;
        Ok(tokens.access_token.clone())
    }

    async fn renew(&self, tokens: &mut Tokens) -> Result<()> {
        let refresh_token =
            tokens.refresh_token.clone().ok_or_else(|| Error::NeedsRelink("there is no saved sign-in".into()))?;
        let fresh = refresh(&self.client, &self.http, &refresh_token, (self.now)()).await?;
        if fresh.refresh_token.as_deref() != Some(refresh_token.as_str())
            && let (Some(callback), Some(new)) = (&self.on_rotate, fresh.refresh_token.as_deref())
        {
            callback(new);
        }
        *tokens = fresh;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pkce_challenge_matches_the_rfc_example() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn random_values_have_the_length_and_alphabet_pkce_needs() {
        let verifier = random_url_safe(32);
        assert_eq!(verifier.len(), 43, "RFC 7636 wants 43 to 128 characters");
        assert!(verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_ne!(random_url_safe(32), random_url_safe(32));
    }

    #[tokio::test]
    async fn the_sign_in_address_carries_everything_google_requires() {
        let client = OAuthClient::google("id-123", "secret-456");
        let pending = begin(&client).await.unwrap();
        let url = Url::parse(&pending.auth_url).unwrap();
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("accounts.google.com"));
        assert_eq!(q["client_id"], "id-123");
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], challenge_for(&pending.verifier));
        assert_eq!(q["access_type"], "offline");
        assert_eq!(q["prompt"], "consent");
        assert!(q["scope"].contains("gmail.modify") && q["scope"].contains("openid"));
        assert!(q["redirect_uri"].starts_with("http://127.0.0.1:"));
        assert!(!pending.auth_url.contains("secret-456"), "the client secret never goes in the browser address");
        assert!(!pending.auth_url.contains(&pending.verifier), "nor does the verifier");
    }
}
