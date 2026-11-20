use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Extracted {
    pub kind: String,
    pub value: String,
}

static UPS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(1Z[0-9A-Z]{16})\b").expect("valid regex"));
static DHL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(JD\d{18}|JJD\d{15,20}|\d{10})\b").expect("valid regex"));
static USPS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(9[2-5]\d{20,24})\b").expect("valid regex"));
static LABELLED_TRACKING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\btracking(?:\s+(?:number|no\.?|id|code))?\s*(?:is|:|#)?\s*([A-Z0-9][A-Z0-9-]{7,29})\b")
        .expect("valid regex")
});
static ORDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\border(?:\s+(?:number|no\.?|id))?\s*(?:is|:|#)?\s*#?\s*([A-Z0-9][A-Z0-9-]{3,24})\b")
        .expect("valid regex")
});
static REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:booking|confirmation|reservation|reference|ref\.?)\s*(?:number|no\.?|code|id)?\s*(?:is|:|#)?\s*([A-Z0-9][A-Z0-9-]{4,19})\b")
        .expect("valid regex")
});

const NOT_VALUES: &[&str] =
    &["has", "was", "will", "is", "number", "confirmation", "details", "status", "from", "for", "and"];

fn plausible(value: &str, needs_digit: bool) -> bool {
    let lower = value.to_ascii_lowercase();
    if NOT_VALUES.contains(&lower.as_str()) {
        return false;
    }
    !needs_digit || value.chars().any(|c| c.is_ascii_digit())
}

fn push(out: &mut Vec<Extracted>, kind: &str, value: &str) {
    if !out.iter().any(|e| e.kind == kind && e.value.eq_ignore_ascii_case(value)) {
        out.push(Extracted { kind: kind.into(), value: value.to_string() });
    }
}

pub fn extract_details(subject: &str, text: &str) -> Vec<Extracted> {
    let haystack = format!("{subject}\n{text}");
    let mut out: Vec<Extracted> = Vec::new();
    for re in [&*UPS, &*USPS] {
        for c in re.captures_iter(&haystack) {
            push(&mut out, "tracking", &c[1]);
        }
    }
    for c in DHL.captures_iter(&haystack) {
        let v = &c[1];
        if v.starts_with("JD") || v.starts_with("JJD") || haystack.to_ascii_lowercase().contains("tracking") {
            push(&mut out, "tracking", v);
        }
    }
    for c in LABELLED_TRACKING.captures_iter(&haystack) {
        if plausible(&c[1], true) {
            push(&mut out, "tracking", &c[1]);
        }
    }
    for c in ORDER.captures_iter(&haystack) {
        if plausible(&c[1], true) {
            push(&mut out, "order", &c[1]);
        }
    }
    for c in REFERENCE.captures_iter(&haystack) {
        if plausible(&c[1], true) {
            push(&mut out, "reference", &c[1]);
        }
    }
    out.sort_by_key(|e| match e.kind.as_str() {
        "tracking" => 0,
        "order" => 1,
        _ => 2,
    });
    out.truncate(6);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(subject: &str, text: &str) -> Vec<(String, String)> {
        extract_details(subject, text).into_iter().map(|e| (e.kind, e.value)).collect()
    }

    #[test]
    fn finds_the_order_and_tracking_numbers_in_a_parcel_email() {
        let got = found(
            "Your parcel is on its way",
            "Your order 394-2211 has been handed to DHL.\n\nXbox Series X, Galaxy Black (1 item)\nExpected delivery: Tuesday\nTracking number: JD014600003394512",
        );
        assert_eq!(got, [("tracking".into(), "JD014600003394512".into()), ("order".into(), "394-2211".into())]);
    }

    #[test]
    fn recognises_common_carrier_formats_without_a_label() {
        assert_eq!(
            found("", "Package 1Z999AA10123456784 left the depot"),
            [("tracking".to_string(), "1Z999AA10123456784".to_string())]
        );
        assert_eq!(
            found("", "USPS 9400111899223197428490 delivered"),
            [("tracking".to_string(), "9400111899223197428490".to_string())]
        );
    }

    #[test]
    fn ignores_words_that_only_look_like_values() {
        assert!(found("Order has shipped", "Your order has been received. Tracking number will follow.").is_empty());
        assert!(found("", "Our reference to the above, and the reference material").is_empty());
    }

    #[test]
    fn a_bare_ten_digit_number_is_only_tracking_when_the_message_says_so() {
        assert!(found("Call us on 0123456789", "Thanks").is_empty());
        assert_eq!(found("", "Tracking info: 5012345678"), [("tracking".to_string(), "5012345678".to_string())]);
    }

    #[test]
    fn finds_booking_references_and_removes_duplicates() {
        let got = found(
            "Booking confirmation",
            "Booking reference: ZX81QP. Please keep reference ZX81QP safe. Order #A-77231.",
        );
        assert_eq!(got, [("order".into(), "A-77231".into()), ("reference".into(), "ZX81QP".into())]);
    }

    #[test]
    fn caps_the_list() {
        let text = (0..20).map(|i| format!("order #A{i:04}")).collect::<Vec<_>>().join(" ");
        assert_eq!(extract_details("", &text).len(), 6);
    }
}
