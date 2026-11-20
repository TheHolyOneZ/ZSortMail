use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::LazyLock;

use ammonia::Builder;
use regex::Regex;
use url::Url;

use crate::trackers::{is_tracker_url, strip_tracking_params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sanitized {
    pub html: String,
    pub remote_images: u32,
    pub trackers_removed: u32,
}

const ALLOWED_STYLE_PROPERTIES: &[&str] = &[
    "color",
    "background-color",
    "background",
    "font",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "line-height",
    "letter-spacing",
    "text-align",
    "text-decoration",
    "text-transform",
    "text-indent",
    "vertical-align",
    "white-space",
    "word-break",
    "overflow-wrap",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "border-color",
    "border-style",
    "border-width",
    "border-radius",
    "border-collapse",
    "border-spacing",
    "width",
    "height",
    "min-width",
    "max-width",
    "min-height",
    "max-height",
    "display",
    "list-style",
    "list-style-type",
    "direction",
    "table-layout",
];

const ALLOWED_DISPLAY: &[&str] = &["block", "inline", "inline-block", "table", "table-row", "table-cell", "none"];

const FORBIDDEN_IN_VALUES: &[&str] =
    &["url(", "expression", "javascript", "vbscript", "@import", "behavior", "binding", "\\", "<", ">"];

pub fn clean_style(css: &str) -> String {
    let mut kept = Vec::new();
    for declaration in css.split(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim().to_ascii_lowercase();
        let value = value.trim();
        if value.is_empty() || !ALLOWED_STYLE_PROPERTIES.contains(&property.as_str()) {
            continue;
        }
        let lowered = value.to_ascii_lowercase();
        if FORBIDDEN_IN_VALUES.iter().any(|bad| lowered.contains(bad)) {
            continue;
        }
        if property == "display" && !ALLOWED_DISPLAY.contains(&lowered.as_str()) {
            continue;
        }
        kept.push(format!("{property}: {value}"));
    }
    kept.join("; ")
}

fn builder() -> Builder<'static> {
    let mut b = Builder::default();
    b.add_tags(["font", "center", "section", "article", "header", "footer", "main", "nav"]);
    b.add_generic_attributes([
        "style",
        "dir",
        "align",
        "valign",
        "bgcolor",
        "color",
        "width",
        "height",
        "border",
        "cellpadding",
        "cellspacing",
        "colspan",
        "rowspan",
        "lang",
        "title",
    ]);
    b.add_tag_attributes("font", ["face", "size", "color"]);
    b.url_schemes(HashSet::from(["http", "https", "mailto", "tel", "cid"]));
    b.link_rel(Some("noopener noreferrer nofollow"));
    b.set_tag_attribute_value("a", "target", "_blank");
    b.strip_comments(true);
    b.clean_content_tags(HashSet::from(["script", "style", "head", "title", "noscript"]));
    b.attribute_filter(|_element, attribute, value| match attribute {
        "style" => {
            let cleaned = clean_style(value);
            if cleaned.is_empty() { None } else { Some(Cow::Owned(cleaned)) }
        }
        "href" => Some(Cow::Owned(strip_tracking_params(value))),
        _ => Some(Cow::Owned(value.to_string())),
    });
    b
}

static IMG_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<img\s[^>]*>").expect("valid regex"));
static ATTR_SRC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\ssrc="([^"]*)""#).expect("valid regex"));
static ATTR_WIDTH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\swidth="(\d+)(?:px)?""#).expect("valid regex"));
static ATTR_HEIGHT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\sheight="(\d+)(?:px)?""#).expect("valid regex"));

fn is_remote(src: &str) -> bool {
    Url::parse(src).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
}

fn tiny(tag: &str) -> bool {
    let small = |re: &Regex| {
        re.captures(tag).and_then(|c| c.get(1)).and_then(|m| m.as_str().parse::<u32>().ok()).is_some_and(|n| n <= 2)
    };
    small(&ATTR_WIDTH) || small(&ATTR_HEIGHT)
}

pub fn sanitize_html(input: &str) -> Sanitized {
    let cleaned = builder().clean(input).to_string();

    let mut remote_images = 0u32;
    let mut trackers_removed = 0u32;
    let html = IMG_TAG
        .replace_all(&cleaned, |caps: &regex::Captures<'_>| {
            let tag = &caps[0];
            let src = ATTR_SRC.captures(tag).map(|c| html_escape::decode_html_entities(&c[1]).into_owned());
            match src {
                Some(src) if is_remote(&src) => {
                    if tiny(tag) || is_tracker_url(&src) {
                        trackers_removed += 1;
                        String::new()
                    } else {
                        remote_images += 1;
                        tag.to_string()
                    }
                }
                _ => tag.to_string(),
            }
        })
        .into_owned();

    Sanitized { html, remote_images, trackers_removed }
}

pub fn block_remote_images(html: &str) -> String {
    IMG_TAG
        .replace_all(html, |caps: &regex::Captures<'_>| {
            let tag = &caps[0];
            let remote = ATTR_SRC.captures(tag).is_some_and(|c| is_remote(&html_escape::decode_html_entities(&c[1])));
            if remote { ATTR_SRC.replace(tag, r#" data-zsm-src="$1""#).into_owned() } else { tag.to_string() }
        })
        .into_owned()
}

pub fn rewrite_cid_images(html: &str, mut resolve: impl FnMut(&str) -> Option<String>) -> String {
    static CID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"src="cid:([^"]+)""#).expect("valid regex"));
    CID.replace_all(html, |caps: &regex::Captures<'_>| match resolve(&caps[1]) {
        Some(url) => format!(r#"src="{url}""#),
        None => r#"src="""#.to_string(),
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(html: &str) -> String {
        sanitize_html(html).html
    }

    #[test]
    fn removes_script_and_event_handlers() {
        let out = clean(r#"<p onclick="evil()">hi</p><script>alert(1)</script><img src="cid:a" onerror="evil()">"#);
        assert!(!out.contains("script") && !out.contains("alert"));
        assert!(!out.contains("onclick") && !out.contains("onerror"));
        assert!(out.contains("hi"));
    }

    #[test]
    fn removes_the_elements_a_csp_cannot_stop() {
        for attack in [
            r#"<meta http-equiv="refresh" content="0;url=http://x.example/">"#,
            r#"<base href="http://x.example/">"#,
            r#"<form action="http://x.example/" method="post"><input name="a"></form>"#,
            r#"<iframe src="http://x.example/"></iframe>"#,
            r#"<object data="http://x.example/"></object><embed src="http://x.example/">"#,
            r#"<link rel="stylesheet" href="http://x.example/a.css">"#,
        ] {
            let out = clean(attack);
            for needle in ["meta", "<base", "<form", "<iframe", "<object", "<embed", "<link", "x.example"] {
                assert!(!out.contains(needle), "{needle} survived in {out:?} (from {attack})");
            }
        }
    }

    #[test]
    fn style_blocks_are_dropped_with_their_content() {
        let out = clean("<style>body{background:url(http://x.example/a)}</style><p>text</p>");
        assert!(!out.contains("x.example") && !out.contains("style"));
        assert!(out.contains("text"));
    }

    #[test]
    fn inline_style_keeps_presentation_and_loses_urls() {
        let out = clean(
            r#"<p style="color: #123; background: url(http://x.example/a); font-size: 14px; position: fixed; display: flex">x</p>"#,
        );
        assert!(out.contains("color: #123"));
        assert!(out.contains("font-size: 14px"));
        assert!(!out.contains("url(") && !out.contains("x.example"));
        assert!(!out.contains("position"));
        assert!(!out.contains("flex"));
    }

    #[test]
    fn style_with_hidden_escapes_is_rejected() {
        assert_eq!(clean_style(r"color: \75rl(javascript:alert(1))"), "");
        assert_eq!(clean_style("width: expression(alert(1))"), "");
        assert_eq!(clean_style("color: red; margin: 0 auto"), "color: red; margin: 0 auto");
    }

    #[test]
    fn javascript_links_are_dropped_and_real_links_are_hardened() {
        let out = clean(r#"<a href="javascript:alert(1)">a</a><a href="https://example.org/x">b</a>"#);
        assert!(!out.contains("javascript"));
        assert!(out.contains(r#"href="https://example.org/x""#));
        assert!(out.contains(r#"target="_blank""#));
        assert!(out.contains("noopener"));
    }

    #[test]
    fn tracking_parameters_are_removed_from_links() {
        let out = clean(r#"<a href="https://example.org/page?id=7&utm_source=news&fbclid=abc&gclid=z">go</a>"#);
        assert!(out.contains("id=7"));
        assert!(!out.contains("utm_source") && !out.contains("fbclid") && !out.contains("gclid"));
    }

    #[test]
    fn tracking_pixels_are_removed_and_counted() {
        let s = sanitize_html(
            r#"<img src="https://t.example/p.gif" width="1" height="1"><img src="https://pixel.mailtrack.io/open?id=1"><img src="https://cdn.example/logo.png" width="120"><img src="cid:logo">"#,
        );
        assert_eq!(s.trackers_removed, 2);
        assert_eq!(s.remote_images, 1);
        assert!(s.html.contains("cdn.example/logo.png"));
        assert!(s.html.contains("cid:logo"));
        assert!(!s.html.contains("mailtrack") && !s.html.contains("t.example"));
    }

    #[test]
    fn remote_images_can_be_hidden_and_inline_ones_survive() {
        let html = r#"<img src="https://cdn.example/a.png" alt="a"><img src="cid:b" alt="b">"#;
        let blocked = block_remote_images(&sanitize_html(html).html);
        assert!(blocked.contains(r#"data-zsm-src="https://cdn.example/a.png""#));
        assert!(!blocked.contains(r#" src="https://cdn.example/a.png""#));
        assert!(blocked.contains(r#"src="cid:b""#));
    }

    #[test]
    fn cid_references_are_rewritten_or_emptied() {
        let out = rewrite_cid_images(r#"<img src="cid:a"><img src="cid:zzz">"#, |cid| {
            (cid == "a").then(|| "zsm://localhost/cid/1/a".to_string())
        });
        assert!(out.contains(r#"src="zsm://localhost/cid/1/a""#));
        assert!(out.contains(r#"src="""#));
    }

    #[test]
    fn ordinary_marketing_markup_survives() {
        let html = r##"<table width="600" cellpadding="0" bgcolor="#ffffff"><tr><td align="center" style="padding:12px;color:#333"><h1>Hello</h1><p>Body <b>bold</b> <a href="https://example.org">link</a></p></td></tr></table>"##;
        let out = clean(html);
        for needle in ["<table", "cellpadding", "<h1>", "<b>bold</b>", "padding: 12px", "example.org"] {
            assert!(out.contains(needle), "lost {needle}: {out}");
        }
    }

    #[test]
    fn nested_tricks_do_not_reassemble() {
        let out =
            clean("<scr<script>ipt>alert(1)</scr</script>ipt><svg onload=alert(1)><math><mtext><script>x</script>");
        assert!(!out.contains("<script") && !out.contains("onload"));
    }
}
