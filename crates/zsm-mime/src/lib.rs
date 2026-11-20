pub mod quotes;
pub mod sanitize;
pub mod text;
pub mod trackers;
pub mod unsubscribe;

pub use quotes::{Split, split_quoted_html, strip_quotes};
pub use sanitize::{Sanitized, block_remote_images, rewrite_cid_images, sanitize_html};
pub use text::{html_to_text, snippet};
pub use unsubscribe::{Unsubscribe, parse_unsubscribe};
