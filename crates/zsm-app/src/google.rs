use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use zsm_core::dto::AccountInfo;
use zsm_gmail::client::{Credentials, GmailClient, build_http};
use zsm_gmail::oauth::{self, OAuthClient, Tokens};

use crate::core::Core;
use crate::error::{Error, Result};
use crate::events::{CoreEvent, LinkPhase};

#[derive(Debug, Clone)]
pub struct GoogleEndpoints {
    pub auth_url: String,
    pub token_url: String,
    pub gmail_base: String,
    pub people_base: String,
}

impl Default for GoogleEndpoints {
    fn default() -> Self {
        Self {
            auth_url: oauth::GOOGLE_AUTH_URL.into(),
            token_url: oauth::GOOGLE_TOKEN_URL.into(),
            gmail_base: zsm_gmail::client::GMAIL_BASE.into(),
            people_base: zsm_gmail::people::PEOPLE_BASE.into(),
        }
    }
}

pub fn platform_secrets(dir: std::path::PathBuf) -> Arc<dyn zsm_gmail::credentials::Secrets> {
    use zsm_gmail::credentials::{FileSecrets, Keychain, Layered};
    Arc::new(Layered::new(Box::new(Keychain), Box::new(FileSecrets::new(dir))))
}

pub(crate) struct LinkTask {
    handle: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct GoogleSetup {
    pub configured: bool,
    pub built_in: bool,
    pub client_id: Option<String>,
}

const KEY_ID: &str = "google.client_id";
const KEY_SECRET: &str = "google.client_secret";

const BUILT_IN_ID: Option<&str> = option_env!("ZSM_GOOGLE_CLIENT_ID");
const BUILT_IN_SECRET: Option<&str> = option_env!("ZSM_GOOGLE_CLIENT_SECRET");

impl Core {
    fn client_credentials(&self) -> Result<Option<(String, String)>> {
        if let (Some(id), Some(secret)) = (BUILT_IN_ID, BUILT_IN_SECRET) {
            return Ok(Some((id.to_string(), secret.to_string())));
        }
        self.with_store(|s| {
            Ok(match (s.setting(KEY_ID)?, s.setting(KEY_SECRET)?) {
                (Some(id), Some(secret)) if !id.is_empty() && !secret.is_empty() => Some((id, secret)),
                _ => None,
            })
        })
    }

    pub fn google_setup(&self) -> Result<GoogleSetup> {
        let creds = self.client_credentials()?;
        Ok(GoogleSetup {
            configured: creds.is_some(),
            built_in: BUILT_IN_ID.is_some() && BUILT_IN_SECRET.is_some(),
            client_id: creds.map(|(id, _)| id),
        })
    }

    pub fn save_google_client(&self, client_id: &str, client_secret: &str) -> Result<()> {
        let (id, secret) = (client_id.trim(), client_secret.trim());
        if id.is_empty() || secret.is_empty() {
            return Err(Error::Store(zsm_core::Error::Invalid(
                "Enter both the client ID and the client secret".into(),
            )));
        }
        if id.contains(char::is_whitespace) || secret.contains(char::is_whitespace) {
            return Err(Error::Store(zsm_core::Error::Invalid(
                "The client ID and secret cannot contain spaces".into(),
            )));
        }
        self.with_store(|s| {
            s.set_setting(KEY_ID, id)?;
            s.set_setting(KEY_SECRET, secret)?;
            Ok(())
        })
    }

    pub(crate) fn oauth_client(&self) -> Result<OAuthClient> {
        let (id, secret) = self.client_credentials()?.ok_or_else(|| {
            Error::Store(zsm_core::Error::Invalid(
                "ZSortMail needs a Google client before it can link Gmail. Enter yours in the setup step.".into(),
            ))
        })?;
        Ok(OAuthClient {
            client_id: id,
            client_secret: secret,
            auth_url: self.google.auth_url.clone(),
            token_url: self.google.token_url.clone(),
        })
    }

    pub async fn begin_gmail_link(&self) -> Result<String> {
        let runtime = self
            .runtime
            .clone()
            .ok_or_else(|| Error::Store(zsm_core::Error::Invalid("Background work is not available here".into())))?;
        let client = self.oauth_client()?;
        let pending = oauth::begin(&client).await?;
        let auth_url = pending.auth_url.clone();

        self.cancel_gmail_link();
        let weak = self.weak.clone();
        let handle = runtime.spawn(async move {
            let http = build_http();
            let clock = weak.clone();
            let result = pending
                .finish(&http, Duration::from_secs(300), move || clock.upgrade().map_or(0, |c| c.now_ms()))
                .await;
            let Some(core) = weak.upgrade() else { return };
            let outcome = match result {
                Ok(tokens) => core.complete_link(&http, tokens).await,
                Err(error) => Err(Error::Gmail(error)),
            };
            match outcome {
                Ok(account) => core.emit(CoreEvent::LinkState {
                    phase: LinkPhase::Linked,
                    message: format!("{} is linked.", account.email),
                }),
                Err(error) => {
                    let cancelled = error.to_string().contains("cancelled");
                    core.emit(CoreEvent::LinkState {
                        phase: if cancelled { LinkPhase::Cancelled } else { LinkPhase::Failed },
                        message: error.for_person(),
                    });
                }
            }
            *core.lock(&core.link) = None;
        });
        *self.lock(&self.link) = Some(LinkTask { handle });
        self.emit(CoreEvent::LinkState {
            phase: LinkPhase::Waiting,
            message: "Finish signing in in your browser.".into(),
        });
        Ok(auth_url)
    }

    pub async fn begin_contacts_import(&self) -> Result<String> {
        let runtime = self
            .runtime
            .clone()
            .ok_or_else(|| Error::Store(zsm_core::Error::Invalid("Background work is not available here".into())))?;
        let client = self.oauth_client()?;
        let pending = oauth::begin_with(&client, zsm_gmail::oauth::CONTACTS_SCOPE, false).await?;
        let auth_url = pending.auth_url.clone();
        let weak = self.weak.clone();
        runtime.spawn(async move {
            let http = build_http();
            let clock = weak.clone();
            let result = pending
                .finish(&http, Duration::from_secs(300), move || clock.upgrade().map_or(0, |c| c.now_ms()))
                .await;
            let Some(core) = weak.upgrade() else { return };
            let text = match result {
                Ok(tokens) => core.import_google_contacts(&http, tokens.access_token).await,
                Err(error) => Err(Error::Gmail(error)),
            };
            let (level, text) = match text {
                Ok(text) => (crate::events::NoticeLevel::Info, text),
                Err(error) => (crate::events::NoticeLevel::Error, error.for_person()),
            };
            core.emit(CoreEvent::Notice { level, text });
        });
        Ok(auth_url)
    }

    async fn import_google_contacts(&self, http: &zsm_gmail::HttpClient, token: String) -> Result<String> {
        let client = GmailClient::new(
            http.clone(),
            self.google.people_base.clone(),
            Credentials::Fixed(token),
            self.retry.clone(),
        );
        let found = zsm_gmail::people::fetch_contacts(&client, &self.google.people_base).await?;
        let contacts: Vec<zsm_core::vcard::Contact> = found
            .into_iter()
            .map(|c| zsm_core::vcard::Contact {
                name: c.name,
                emails: c.emails,
                organization: c.organization,
                title: c.title,
                note: c.note,
            })
            .collect();
        if contacts.is_empty() {
            return Ok("Google Contacts had no one with an email address.".into());
        }
        let report = self.with_store(|s| Ok(s.import_contacts(&contacts)?))?;
        self.changed("people", Vec::new());
        Ok(format!(
            "Read {} from Google Contacts. {} matched, {} updated.",
            report.contacts, report.matched, report.updated
        ))
    }

    pub fn cancel_gmail_link(&self) {
        if let Some(task) = self.lock(&self.link).take() {
            task.handle.abort();
            self.emit(CoreEvent::LinkState { phase: LinkPhase::Cancelled, message: "Sign-in cancelled.".into() });
        }
    }

    async fn complete_link(self: &Arc<Self>, http: &zsm_gmail::HttpClient, tokens: Tokens) -> Result<AccountInfo> {
        let client = GmailClient::new(
            http.clone(),
            self.google.gmail_base.clone(),
            Credentials::Fixed(tokens.access_token.clone()),
            self.retry.clone(),
        );
        if !tokens.scope.is_empty() && !tokens.scope.split_whitespace().any(|s| s.ends_with("gmail.modify")) {
            return Err(Error::Gmail(zsm_gmail::Error::MissingPermission("the Gmail box was not ticked".into())));
        }
        let profile = client.profile().await?;
        let refresh = tokens.refresh_token.ok_or_else(|| {
            Error::Store(zsm_core::Error::Invalid(
                "Google did not give ZSortMail lasting access. Remove ZSortMail under your Google account's security settings, then link again."
                    .into(),
            ))
        })?;
        let email = profile.email_address.to_lowercase();
        self.secrets.set(&email, &refresh)?;
        let now = self.now_ms();
        let account = self.with_store(|s| {
            let id = s.upsert_account("gmail", &email, None, now)?;
            s.set_setting(&format!("screener.{id}"), "1")?;
            Ok(s.accounts()?.into_iter().find(|a| a.id == id).expect("the account just saved"))
        })?;
        self.changed("*", Vec::new());
        self.spawn_syncer(&account);
        Ok(account)
    }

    pub fn unlink_account(&self, account_id: i64) -> Result<()> {
        self.stop_syncer(account_id);
        let account = self.with_store(|s| Ok(s.accounts()?.into_iter().find(|a| a.id == account_id)))?;
        let Some(account) = account else {
            return Err(Error::Store(zsm_core::Error::NotFound("account")));
        };
        let _ = self.secrets.delete(&account.email);
        self.with_store(|s| Ok(s.delete_account(account_id)?))?;
        self.lock(&self.status).remove(&account_id);
        self.changed("*", Vec::new());
        Ok(())
    }
}
