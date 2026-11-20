use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use zsm_gmail::client::{Credentials, GmailClient, RetryPolicy, build_http};
use zsm_gmail::oauth::{OAuthClient, Session, begin};
use zsm_gmail::{Error, oauth};
use zsm_testkit::FakeGoogle;

fn oauth_client(fake: &FakeGoogle) -> OAuthClient {
    OAuthClient {
        client_id: "client-1.apps.example".into(),
        client_secret: "not-a-real-secret".into(),
        auth_url: fake.auth_url(),
        token_url: fake.token_url(),
    }
}

fn clock() -> (Arc<AtomicI64>, Arc<dyn Fn() -> i64 + Send + Sync>) {
    let now = Arc::new(AtomicI64::new(1_795_082_400_000));
    let reader = Arc::clone(&now);
    (now, Arc::new(move || reader.load(Ordering::SeqCst)))
}

async fn approve(auth_url: &str) {
    let response = build_http().get(auth_url).send().await.expect("the browser follows the redirect");
    assert!(response.status().is_success(), "the app's loopback page answered: {}", response.status());
    let page = response.text().await.unwrap();
    assert!(page.contains("ZSortMail is linked"), "{page}");
}

#[tokio::test]
async fn signing_in_trades_the_browser_redirect_for_tokens_using_pkce() {
    let fake = FakeGoogle::start("me@example.com").await;
    let (_, now) = clock();
    let pending = begin(&oauth_client(&fake)).await.unwrap();
    let auth_url = pending.auth_url.clone();

    let http = build_http();
    let (tokens, ()) = tokio::join!(
        async { pending.finish(&http, Duration::from_secs(10), || now()).await.unwrap() },
        approve(&auth_url),
    );
    assert_eq!(tokens.access_token, fake.access_token());
    assert_eq!(tokens.refresh_token.as_deref(), Some(fake.refresh_token().as_str()));
    assert!(tokens.scope.contains("gmail.modify"));
    assert_eq!(tokens.expires_at_ms, now() + 3_600_000);
    assert!(tokens.id_token.is_some());
}

#[tokio::test]
async fn a_stray_request_to_the_loopback_port_does_not_end_the_sign_in() {
    let fake = FakeGoogle::start("me@example.com").await;
    let (_, now) = clock();
    let pending = begin(&oauth_client(&fake)).await.unwrap();
    let auth_url = pending.auth_url.clone();
    let port = url_port(&auth_url);

    let http = build_http();
    let (tokens, ()) = tokio::join!(async { pending.finish(&http, Duration::from_secs(10), || now()).await }, async {
        let favicon = build_http().get(format!("http://127.0.0.1:{port}/favicon.ico")).send().await.unwrap();
        assert_eq!(favicon.status(), 404);
        approve(&auth_url).await;
    },);
    assert!(tokens.is_ok());
}

fn url_port(auth_url: &str) -> u16 {
    let url = url::Url::parse(auth_url).unwrap();
    let redirect = url.query_pairs().find(|(k, _)| k == "redirect_uri").unwrap().1.into_owned();
    url::Url::parse(&redirect).unwrap().port().unwrap()
}

#[tokio::test]
async fn an_answer_for_a_different_sign_in_is_refused() {
    let fake = FakeGoogle::start("me@example.com").await;
    let (_, now) = clock();
    let pending = begin(&oauth_client(&fake)).await.unwrap();
    let port = url_port(&pending.auth_url);
    let http = build_http();
    let (result, ()) = tokio::join!(async { pending.finish(&http, Duration::from_secs(10), || now()).await }, async {
        let response =
            build_http().get(format!("http://127.0.0.1:{port}/?code=stolen&state=not-ours")).send().await.unwrap();
        assert_eq!(response.status(), 400);
    },);
    assert!(matches!(result, Err(Error::Auth(_))), "{result:?}");
}

#[tokio::test]
async fn declining_in_the_browser_says_so_and_a_forgotten_sign_in_times_out() {
    let fake = FakeGoogle::start("me@example.com").await;
    let (_, now) = clock();
    let http = build_http();

    let pending = begin(&oauth_client(&fake)).await.unwrap();
    let url = url::Url::parse(&pending.auth_url).unwrap();
    let state = url.query_pairs().find(|(k, _)| k == "state").unwrap().1.into_owned();
    let port = url_port(&pending.auth_url);
    let (result, ()) = tokio::join!(async { pending.finish(&http, Duration::from_secs(10), || now()).await }, async {
        build_http().get(format!("http://127.0.0.1:{port}/?error=access_denied&state={state}")).send().await.unwrap();
    },);
    match result {
        Err(Error::Auth(message)) => assert_eq!(message, "Sign-in was cancelled."),
        other => panic!("{other:?}"),
    }

    let pending = begin(&oauth_client(&fake)).await.unwrap();
    match pending.finish(&http, Duration::from_millis(150), || now()).await {
        Err(Error::Auth(message)) => assert!(message.contains("too long"), "{message}"),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn the_server_checks_the_pkce_verifier() {
    let fake = FakeGoogle::start("me@example.com").await;
    let challenge = oauth::challenge_for("the-real-verifier-the-real-verifier-the-real-v");
    let no_redirects = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
    let redirect = no_redirects
        .get(format!(
            "{}?redirect_uri=http://127.0.0.1:9&code_challenge={challenge}&code_challenge_method=S256&access_type=offline",
            fake.auth_url()
        ))
        .send()
        .await
        .unwrap();
    let location = redirect.headers().get("location").and_then(|v| v.to_str().ok()).unwrap_or_default().to_string();
    assert!(location.contains("code=code-1"), "{location}");
    let response = build_http()
        .post(fake.token_url())
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", "code-1"),
            ("client_id", "x"),
            ("client_secret", "y"),
            ("code_verifier", "an-attacker-guess"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert!(response.text().await.unwrap().contains("invalid_grant"));
}

#[tokio::test]
async fn a_session_refreshes_only_when_the_token_is_about_to_expire() {
    let fake = FakeGoogle::start("me@example.com").await;
    fake.set_token_lifetime(3600);
    let (now_cell, now) = clock();
    let session = Session::from_refresh_token(oauth_client(&fake), build_http(), fake.refresh_token(), now, None);

    assert_eq!(session.access_token().await.unwrap(), fake.access_token());
    assert_eq!(fake.token_requests(), 1);
    session.access_token().await.unwrap();
    session.access_token().await.unwrap();
    assert_eq!(fake.token_requests(), 1, "a valid token is reused");

    now_cell.fetch_add(3600 * 1000 - 30_000, Ordering::SeqCst);
    session.access_token().await.unwrap();
    assert_eq!(fake.token_requests(), 2, "within a minute of expiry it is renewed");
}

fn gmail(fake: &FakeGoogle, session: Arc<Session>) -> GmailClient {
    GmailClient::new(
        build_http(),
        fake.gmail_base(),
        Credentials::Session(session),
        RetryPolicy { max_attempts: 3, base: Duration::from_millis(2), max: Duration::from_millis(10) },
    )
}

#[tokio::test]
async fn an_access_token_rejected_mid_session_is_replaced_and_the_call_succeeds() {
    let fake = FakeGoogle::start("me@example.com").await;
    let (_, now) = clock();
    let session = Session::from_refresh_token(oauth_client(&fake), build_http(), fake.refresh_token(), now, None);
    let client = gmail(&fake, session);

    assert_eq!(client.profile().await.unwrap().email_address, "me@example.com");
    let before = fake.token_requests();
    fake.expire_access_token();
    assert_eq!(client.profile().await.unwrap().email_address, "me@example.com");
    assert_eq!(fake.token_requests(), before + 1, "one refresh, then it worked");
}

#[tokio::test]
async fn a_revoked_sign_in_asks_the_person_to_link_again() {
    let fake = FakeGoogle::start("me@example.com").await;
    let (_, now) = clock();
    let session = Session::from_refresh_token(oauth_client(&fake), build_http(), fake.refresh_token(), now, None);
    let client = gmail(&fake, session);
    client.profile().await.unwrap();

    fake.revoke_refresh_token();
    fake.expire_access_token();
    let error = client.profile().await.unwrap_err();
    assert!(matches!(error, Error::NeedsRelink(_)), "{error:?}");
    assert!(error.for_person().contains("Link the account again"));
    assert!(!error.is_transient());
}
