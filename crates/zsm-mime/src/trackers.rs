use url::Url;

const TRACKER_FRAGMENTS: &[&str] = &[
    "mailtrack.io",
    "list-manage.com/track",
    "sendgrid.net/wf/open",
    "mandrillapp.com/track",
    "doubleclick.net",
    "google-analytics.com",
    "mixpanel.com",
    "hubspotemail.net",
    "hs-analytics",
    "/e/o/",
    "/open.gif",
    "/open.php",
    "/track/open",
    "/tracking/open",
    "/wf/open",
    "pixel.gif",
    "email.mg.",
    "ci3.googleusercontent.com/meips",
    "sg-open.",
    "bl.open.",
    "u001.",
];

pub fn is_tracker_url(src: &str) -> bool {
    let lowered = src.to_ascii_lowercase();
    TRACKER_FRAGMENTS.iter().any(|fragment| lowered.contains(fragment))
}

const TRACKING_PARAMS: &[&str] = &[
    "fbclid",
    "gclid",
    "dclid",
    "msclkid",
    "mc_eid",
    "mc_cid",
    "igshid",
    "yclid",
    "_hsenc",
    "_hsmi",
    "mkt_tok",
    "vero_id",
    "oly_enc_id",
    "oly_anon_id",
    "ref_src",
];

fn is_tracking_param(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("utm_") || TRACKING_PARAMS.contains(&name.as_str())
}

pub fn strip_tracking_params(href: &str) -> String {
    let Ok(mut url) = Url::parse(href) else {
        return href.to_string();
    };
    if !matches!(url.scheme(), "http" | "https") || !url.query_pairs().any(|(k, _)| is_tracking_param(&k)) {
        return href.to_string();
    }
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| !is_tracking_param(k))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if kept.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(kept);
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_known_trackers() {
        assert!(is_tracker_url("https://pixel.mailtrack.io/open?id=1"));
        assert!(is_tracker_url("https://x.list-manage.com/track/open.php?u=1"));
        assert!(!is_tracker_url("https://cdn.example.com/hero.jpg"));
    }

    #[test]
    fn strips_only_tracking_parameters() {
        assert_eq!(
            strip_tracking_params("https://a.example/p?id=7&utm_source=x&utm_medium=y#top"),
            "https://a.example/p?id=7#top"
        );
        assert_eq!(strip_tracking_params("https://a.example/p?utm_source=x"), "https://a.example/p");
    }

    #[test]
    fn leaves_other_links_alone() {
        assert_eq!(strip_tracking_params("mailto:a@b.example?subject=hi"), "mailto:a@b.example?subject=hi");
        assert_eq!(strip_tracking_params("https://a.example/p?id=7"), "https://a.example/p?id=7");
        assert_eq!(strip_tracking_params("not a url"), "not a url");
    }
}
