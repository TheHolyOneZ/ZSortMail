use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unsubscribe {
    pub https: Option<String>,
    pub mailto: Option<String>,
    pub one_click: bool,
}

static ANGLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<([^>\s]+)>").expect("valid regex"));

pub fn parse_unsubscribe(list_unsubscribe: &str, post: Option<&str>) -> Unsubscribe {
    let mut out = Unsubscribe::default();
    for caps in ANGLE.captures_iter(list_unsubscribe) {
        let target = caps[1].trim();
        let lower = target.to_ascii_lowercase();
        if lower.starts_with("https://") && out.https.is_none() {
            out.https = Some(target.to_string());
        } else if lower.starts_with("mailto:") && out.mailto.is_none() {
            out.mailto = Some(target.to_string());
        }
    }
    out.one_click =
        out.https.is_some() && post.is_some_and(|p| p.to_ascii_lowercase().contains("list-unsubscribe=one-click"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_targets() {
        let u = parse_unsubscribe("<mailto:unsub@x.example?subject=unsubscribe>, <https://x.example/u/123>", None);
        assert_eq!(u.https.as_deref(), Some("https://x.example/u/123"));
        assert_eq!(u.mailto.as_deref(), Some("mailto:unsub@x.example?subject=unsubscribe"));
        assert!(!u.one_click);
    }

    #[test]
    fn one_click_needs_both_https_and_the_post_header() {
        let post = Some("List-Unsubscribe=One-Click");
        assert!(parse_unsubscribe("<https://x.example/u>", post).one_click);
        assert!(!parse_unsubscribe("<mailto:u@x.example>", post).one_click);
        assert!(!parse_unsubscribe("<https://x.example/u>", None).one_click);
    }

    #[test]
    fn ignores_plain_http_and_junk() {
        let u = parse_unsubscribe("<http://x.example/u>, garbage", None);
        assert_eq!(u, Unsubscribe::default());
    }
}
