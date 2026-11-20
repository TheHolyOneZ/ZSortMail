pub mod client;
pub mod convert;
pub mod credentials;
mod error;
pub mod model;
pub mod oauth;
pub mod outgoing;
pub mod people;
pub mod sync;

pub use client::{Credentials, GmailClient, RetryPolicy};
pub use error::{Error, MISSING_PERMISSION, Result};
pub use reqwest::Client as HttpClient;
