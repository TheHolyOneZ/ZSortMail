use std::sync::LazyLock;

use regex::Regex;

static DROP_BLOCKS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<script\b.*?</script\s*>|<style\b.*?</style\s*>|<head\b.*?</head\s*>|<title\b.*?</title\s*>")
        .expect("valid regex")
});
static BREAKS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<\s*(br|/p|/div|/li|/tr|/h[1-6]|/table|/blockquote|hr)\b[^>]*>").expect("valid regex")
});
static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").expect("valid regex"));
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t\u{a0}]+").expect("valid regex"));
static BLANKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").expect("valid regex"));

pub fn html_to_text(html: &str) -> String {
    let without_blocks = DROP_BLOCKS.replace_all(html, "");
    let broken = BREAKS.replace_all(&without_blocks, "\n");
    let stripped = TAGS.replace_all(&broken, "");
    let decoded = html_escape::decode_html_entities(&stripped);
    let tidy: Vec<String> = decoded.lines().map(|line| SPACES.replace_all(line, " ").trim().to_string()).collect();
    BLANKS.replace_all(tidy.join("\n").trim(), "\n\n").into_owned()
}

pub fn snippet(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let cut: String = flat.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_blocks_to_lines_and_decodes_entities() {
        let text = html_to_text("<p>Hello&nbsp;there &amp; welcome</p><p>Second<br>line</p>");
        assert_eq!(text, "Hello there & welcome\nSecond\nline");
    }

    #[test]
    fn drops_script_and_style_content() {
        assert_eq!(html_to_text("<style>p{color:red}</style><script>x()</script><p>text</p>"), "text");
    }

    #[test]
    fn collapses_runs_of_blank_lines() {
        assert_eq!(html_to_text("<div>a</div><div></div><div></div><div></div><div>b</div>"), "a\n\nb");
    }

    #[test]
    fn snippet_is_one_line_and_bounded() {
        assert_eq!(snippet("a  b\n\nc", 20), "a b c");
        assert_eq!(snippet("abcdefghij", 6), "abcde…");
        assert_eq!(snippet("日本語のテキストです", 5), "日本語の…");
    }
}
