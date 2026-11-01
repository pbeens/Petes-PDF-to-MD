//! Small text helpers shared by the outline, body and output code.
//! Ported from the retired Python converter
//! (`archive/electron-python/scripts/extract_outline.py`).

use regex::Regex;
use std::sync::LazyLock;

static CONTROL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x00-\x1f\x7f]").unwrap());
static WHITESPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
static LEADING_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{1,3}\s+").unwrap());
static TRAILING_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+\d{1,3}$").unwrap());
static PAGE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^page\s+\d{1,4}\s*").unwrap());
static PAGE_SUFFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s*\bpage\s+\d{1,4}$").unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]\([^)]*\)").unwrap());
static EMPHASIS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\*\*|__|\*|`)").unwrap());

/// Collapse whitespace, drop control characters and object-replacement marks.
pub fn clean_line(line: &str) -> String {
    let line = line.replace(['\u{0}', '\u{fffc}'], "");
    let line = CONTROL.replace_all(&line, " ");
    WHITESPACE.replace_all(&line, " ").trim().to_string()
}

/// File-name friendly form of a heading: lowercase words joined by dashes, at most 64 characters.
pub fn slugify(value: &str) -> String {
    let lower = value.to_lowercase();
    let slug = NON_ALNUM.replace_all(&lower, "-");
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "section" } else { slug };
    slug.chars().take(64).collect()
}

/// Lowercase letters and digits only, for comparing headings with page text.
pub fn normalize_match_text(value: &str) -> String {
    let lower = clean_line(value).to_lowercase();
    let no_tags = TAG.replace_all(&lower, " ");
    clean_line(&NON_ALNUM.replace_all(&no_tags, " "))
}

/// Like [`normalize_match_text`], ignoring a page number at either end
/// ("12 Title", "Title 12", "Page 3 Title", "Title Page 3").
pub fn normalize_margin_noise_text(value: &str) -> String {
    let text = clean_line(value);
    let text = PAGE_SUFFIX.replace(&text, "");
    let text = LEADING_NUMBER.replace(&text, "");
    let text = TRAILING_NUMBER.replace(&text, "");
    let text = PAGE_PREFIX.replace(&text, "");
    normalize_match_text(&text)
}

/// Undo UTF-8 text that was decoded as Windows-1252 ("â€™" -> "’").
pub fn repair_mojibake(text: &str) -> String {
    const REPLACEMENTS: [(&str, &str); 12] = [
        ("\u{00c3}\u{00a2}\u{00e2}\u{201a}\u{00ac}\u{00c2}\u{00a2}", "\u{2022}"),
        ("\u{00e2}\u{20ac}\u{00a2}", "\u{2022}"),
        ("\u{00e2}\u{20ac}\u{201c}", "\u{2013}"),
        ("\u{00e2}\u{20ac}\u{201d}", "\u{2014}"),
        ("\u{00e2}\u{20ac}\u{02dc}", "\u{2018}"),
        ("\u{00e2}\u{20ac}\u{2122}", "\u{2019}"),
        ("\u{00e2}\u{20ac}\u{0153}", "\u{201c}"),
        ("\u{00e2}\u{20ac}\u{009d}", "\u{201d}"),
        ("\u{00e2}\u{20ac}\u{00a6}", "\u{2026}"),
        ("\u{00c2}\u{00a0}", " "),
        ("\u{00c2} ", " "),
        ("\u{00c2}", ""),
    ];
    let mut out = text.to_string();
    for (bad, good) in REPLACEMENTS {
        out = out.replace(bad, good);
    }
    out
}

/// Plain text of a Markdown line: no HTML tags, links reduced to their text, no emphasis marks.
pub fn strip_inline_markdown(text: &str) -> String {
    let out = TAG.replace_all(text, "");
    let out = LINK.replace_all(&out, "$1");
    let out = EMPHASIS.replace_all(&out, "");
    clean_line(&out)
}

/// First character is a lowercase letter (Python's `str[:1].islower()`).
pub fn starts_lowercase(text: &str) -> bool {
    text.chars().next().is_some_and(char::is_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_matches_python() {
        assert_eq!(slugify("A1. Computational Thinking, Planning"), "a1-computational-thinking-planning");
        assert_eq!(slugify("***"), "section");
    }

    #[test]
    fn normalize_ignores_case_and_punctuation() {
        assert_eq!(normalize_match_text("The <b>What</b>: Part 1!"), "the what part 1");
        assert_eq!(normalize_margin_noise_text("12 Artificial Intelligence in Education"), "artificial intelligence in education");
        assert_eq!(normalize_margin_noise_text("Public vs. Private Sector Efficiency Page 3"), "public vs private sector efficiency");
        assert_eq!(normalize_margin_noise_text("Page 16 Public vs. Private Sector Efficiency"), "public vs private sector efficiency");
    }

    #[test]
    fn repairs_mojibake_quotes() {
        assert_eq!(repair_mojibake("don\u{00e2}\u{20ac}\u{2122}t"), "don\u{2019}t");
    }

    #[test]
    fn strips_inline_markdown() {
        assert_eq!(strip_inline_markdown("**Bold** and [link](http://x)"), "Bold and link");
    }
}
