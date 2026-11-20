use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\p{L}\p{N}]+").expect("valid regex"));

pub fn fold(text: &str) -> String {
    text.nfd().filter(|c| !unicode_normalization::char::is_combining_mark(*c)).collect::<String>().to_lowercase()
}

pub fn words(text: &str) -> Vec<String> {
    let folded = fold(text);
    WORD.find_iter(&folded).map(|m| m.as_str().to_string()).collect()
}

pub fn fts_prefix_query(query: &str) -> Option<String> {
    let tokens = words(query);
    if tokens.is_empty() {
        return None;
    }
    Some(tokens.iter().map(|t| format!("\"{t}\"*")).collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_accents_and_case() {
        assert_eq!(fold("Amélie Dubois-Castellanos"), "amelie dubois-castellanos");
        assert_eq!(fold("ŁUKASZ"), "łukasz");
    }

    #[test]
    fn splits_on_punctuation_like_the_search_index() {
        assert_eq!(words("rasmus@lindqvist-moe.example"), ["rasmus", "lindqvist", "moe", "example"]);
        assert_eq!(words("Xbox Series X #394"), ["xbox", "series", "x", "394"]);
        assert_eq!(words("田中 弘"), ["田中", "弘"]);
    }

    #[test]
    fn builds_prefix_queries() {
        assert_eq!(fts_prefix_query("Rasmus lind").as_deref(), Some("\"rasmus\"* \"lind\"*"));
        assert_eq!(fts_prefix_query("  ---  "), None);
        assert_eq!(fts_prefix_query("a\"b* OR c").as_deref(), Some("\"a\"* \"b\"* \"or\"* \"c\"*"));
    }
}
