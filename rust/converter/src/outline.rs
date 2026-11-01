//! Building the heading outline: bookmarks, accessibility tags, and the
//! coded headings ("A.", "A1.") that some tagged PDFs mark as paragraphs.
//! Ported from `build_outline_from_toc`, `build_outline_from_tags`,
//! `add_untagged_code_headings` and `normalize_outline`.

use crate::layout::Line;
use crate::text::{clean_line, normalize_match_text, repair_mojibake, starts_lowercase};
use pdf_inspector::types::ItemType;
use pdf_inspector::{StructureElement, TextItem};
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

/// One heading before numbering and file names are assigned.
#[derive(Clone, Debug, PartialEq)]
pub struct Heading {
    pub level: u32,
    pub title: String,
    pub page_start: u32,
    pub source: String,
}

static HEADING_ROLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^H[1-6]?$").unwrap());
static SUPERSCRIPT_DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[¹²³\u{2070}-\u{2079}]+").unwrap());
static UNFINISHED_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[.?!:]$").unwrap());
/// Lettered/numbered headings: "A. Programming", "B3. Cybersecurity and Data".
static CODE_HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([A-Z])(\d{1,2})?\.\s+[A-Z]").unwrap());

/// Footnote numbers copied into bookmarks are glued to the last word
/// ("Effect107"); real numbering has a space ("Part 1", "Appendix 2").
fn strip_bookmark_footnote(title: &str) -> String {
    let digits = title.chars().rev().take_while(char::is_ascii_digit).count();
    if !(1..=3).contains(&digits) {
        return title.to_string();
    }
    let head: Vec<char> = title.chars().collect();
    let cut = head.len() - digits;
    let lowercase_run = head[..cut].iter().rev().take(4).filter(|c| c.is_ascii_lowercase()).count();
    if cut >= 4 && lowercase_run == 4 {
        head[..cut].iter().collect()
    } else {
        title.to_string()
    }
}

/// Headings from the PDF's bookmarks (document outline).
pub fn from_bookmarks(doc: &lopdf::Document) -> Vec<Heading> {
    let Ok(toc) = doc.get_toc() else {
        return Vec::new();
    };
    toc.toc
        .into_iter()
        .filter_map(|entry| {
            let title = strip_bookmark_footnote(&clean_line(&entry.title));
            (!title.is_empty()).then(|| Heading {
                level: entry.level.max(1) as u32,
                title,
                page_start: entry.page.max(1) as u32,
                source: "pdf-outline".into(),
            })
        })
        .collect()
}

type RunKey = (u32, i64);

fn text_runs(items: &[TextItem], keep: impl Fn(&RunKey) -> bool) -> BTreeMap<RunKey, Vec<&TextItem>> {
    let mut runs: BTreeMap<RunKey, Vec<&TextItem>> = BTreeMap::new();
    for item in items {
        let Some(mcid) = item.mcid else { continue };
        let key = (item.page, mcid);
        if matches!(item.item_type, ItemType::Text) && keep(&key) {
            runs.entry(key).or_default().push(item);
        }
    }
    runs
}

/// Join a run's pieces, adding a space at line breaks and visible gaps.
fn run_text(run: &[&TextItem]) -> String {
    let mut out = String::new();
    let mut last: Option<&TextItem> = None;
    for item in run {
        if let Some(prev) = last {
            let same_line = (item.y - prev.y).abs() <= 2.0;
            let gap = item.x - (prev.x + prev.width);
            if !same_line || gap > 0.15 * item.font_size.max(1.0) {
                out.push(' ');
            }
        }
        out.push_str(&item.text);
        last = Some(item);
    }
    out
}

/// Headings from a tagged PDF's structure tree (H1-H6 roles), plus coded
/// headings tagged as paragraphs. Empty when the PDF is untagged or
/// `PDF_TO_MD_NO_TAGS` is set.
pub fn from_tags(items: &[TextItem], elements: &[StructureElement]) -> Vec<Heading> {
    if std::env::var_os("PDF_TO_MD_NO_TAGS").is_some() {
        return Vec::new();
    }
    let all_roles: HashMap<RunKey, &str> = elements.iter().map(|e| ((e.page, e.mcid), e.role.as_str())).collect();
    let roles: HashMap<RunKey, &str> = all_roles.iter().filter(|(_, r)| HEADING_ROLE.is_match(r)).map(|(k, r)| (*k, *r)).collect();
    if roles.is_empty() {
        return Vec::new();
    }

    let runs = text_runs(items, |key| roles.contains_key(key));
    let mut entries: Vec<(Heading, RunKey)> = Vec::new();
    let mut prev_key: Option<RunKey> = None;
    for (key, run) in &runs {
        // Drop superscript footnote markers ("Effect¹⁰⁷").
        let title = clean_line(&SUPERSCRIPT_DIGITS.replace_all(&run_text(run), ""));
        if title.is_empty() || title.chars().count() > 200 {
            continue;
        }
        let role = roles[key];
        let level = role[1..].parse::<u32>().unwrap_or(1);

        // A heading that wraps onto a second marked-content run continues the previous
        // one, but separate headings stacked on a page share the role too. Join only on
        // the same font size and an unfinished first line (ends on a lowercase word, or
        // the next run starts in lowercase).
        let continues = match (entries.last(), prev_key) {
            (Some((prev, _)), Some(pk)) => {
                let last_word = prev.title.split_whitespace().last().unwrap_or("");
                let unfinished = !UNFINISHED_END.is_match(&prev.title) && (starts_lowercase(last_word) || starts_lowercase(&title));
                key.0 == pk.0
                    && key.1 == pk.1 + 1
                    && roles[&pk] == role
                    && (run[0].font_size - runs[&pk][0].font_size).abs() <= 0.5
                    && unfinished
            }
            _ => false,
        };
        if continues {
            let (prev, _) = entries.last_mut().unwrap();
            prev.title = clean_line(&format!("{} {}", prev.title, title));
        } else {
            entries.push((Heading { level, title, page_start: key.0, source: "pdf-tags".into() }, *key));
        }
        prev_key = Some(*key);
    }
    add_untagged_code_headings(entries, items, &all_roles)
}

/// Add "A. …" / "A1. …" lines that are bold and clearly larger than body text but
/// tagged as paragraphs. Single letters go one level below the tagged heading they
/// sit under; letter-and-number codes one level below that.
fn add_untagged_code_headings(entries: Vec<(Heading, RunKey)>, items: &[TextItem], all_roles: &HashMap<RunKey, &str>) -> Vec<Heading> {
    let mut sizes: Vec<f32> = items
        .iter()
        .filter(|i| matches!(i.item_type, ItemType::Text) && i.font_size > 0.0)
        .map(|i| i.font_size)
        .collect();
    if sizes.is_empty() {
        return entries.into_iter().map(|(h, _)| h).collect();
    }
    sizes.sort_by(f32::total_cmp);
    let body_size = sizes[sizes.len() / 2];

    // (heading, key, code depth): depth 0 = tagged heading, 1 = "A.", 2 = "A1."
    let mut merged: Vec<(Heading, RunKey, u8)> = entries.into_iter().map(|(h, k)| (h, k, 0)).collect();
    for (key, run) in text_runs(items, |key| all_roles.get(key) == Some(&"P")) {
        let first = run[0];
        let title = clean_line(&run.iter().map(|i| i.text.as_str()).collect::<String>());
        let Some(caps) = CODE_HEADING.captures(&title) else { continue };
        if !first.is_bold || first.font_size < 1.3 * body_size || title.chars().count() > 120 {
            continue;
        }
        let depth = if caps.get(2).is_some() { 2 } else { 1 };
        merged.push((Heading { level: 0, title, page_start: key.0, source: "pdf-tags".into() }, key, depth));
    }
    merged.sort_by_key(|(_, key, _)| *key);

    let mut base_level = 1;
    let mut strand_level: Option<u32> = None;
    merged
        .into_iter()
        .map(|(mut heading, _, depth)| {
            match depth {
                0 => {
                    base_level = heading.level;
                    strand_level = None;
                }
                1 => {
                    let level = (base_level + 1).min(6);
                    strand_level = Some(level);
                    heading.level = level;
                }
                _ => heading.level = (strand_level.unwrap_or(base_level) + 1).min(6),
            }
            heading
        })
        .collect()
}

static NUMBERED_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{1,2}[.)]\s").unwrap());
static ENDS_LIKE_SENTENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\\.:;!?]$").unwrap());

fn percentile(sorted: &[f32], pct: f64) -> f32 {
    let idx = ((sorted.len() - 1) as f64 * pct).floor() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Headings worked out from the text when a PDF has neither bookmarks nor tags:
/// lines clearly larger than body text, with wrapped titles joined and levels
/// assigned by size (largest = 1, at most 3 levels).
pub fn from_text(lines: &[Line]) -> Vec<Heading> {
    let mut sizes: Vec<f32> = lines.iter().map(|l| l.size).filter(|s| *s > 0.0).collect();
    if sizes.is_empty() {
        return Vec::new();
    }
    sizes.sort_by(f32::total_cmp);
    let cutoff = percentile(&sizes, 0.75).max(percentile(&sizes, 0.5) + 1.0);

    let mut seen = std::collections::HashSet::new();
    let mut candidates: Vec<Line> = Vec::new();
    for line in lines {
        let text = &line.text;
        let words = text.split_whitespace().count();
        let chars = text.chars().count();
        if !(4..=120).contains(&chars) || text.starts_with(['•', '◦', '▪', '-', '*']) || line.size < cutoff {
            continue;
        }
        // Numbered list items ("3. Students discuss their choices ...") are body text
        // when long or sentence-like; short titles such as "1. Introduction" stay.
        if NUMBERED_ITEM.is_match(text) && (words > 8 || text.ends_with('.')) {
            continue;
        }
        if text.ends_with('.') && words > 8 {
            continue;
        }
        if seen.insert(text.to_lowercase()) {
            candidates.push(line.clone());
        }
    }
    candidates.sort_by(|a, b| a.page.cmp(&b.page).then(a.top.total_cmp(&b.top)));

    // Join headings that wrap onto the next line.
    const TAIL_WORDS: [&str; 11] = ["for", "and", "or", "to", "in", "on", "of", "with", "program", "course", "grade"];
    const LEAD_WORDS: [&str; 7] = ["and", "or", "for", "to", "in", "of", "the"];
    let mut merged: Vec<Line> = Vec::new();
    let mut iter = candidates.into_iter().peekable();
    while let Some(mut current) = iter.next() {
        while let Some(next) = iter.peek() {
            let cur_words: Vec<&str> = current.text.split_whitespace().collect();
            let next_words: Vec<&str> = next.text.split_whitespace().collect();
            let tail = cur_words.last().map(|w| w.to_lowercase()).unwrap_or_default();
            let first = next_words.first().copied().unwrap_or("");
            let lead_word = LEAD_WORDS.contains(&first.to_lowercase().as_str());
            let unfinished = !ENDS_LIKE_SENTENCE.is_match(&current.text);
            let long_wrapped_title = cur_words.len() >= 6
                && next_words.len() <= 8
                && unfinished
                && (next.text.contains(',') || current.text.chars().count() >= 35 || lead_word);
            let short_wrapped = cur_words.len() <= 6
                && next_words.len() <= 6
                && (starts_lowercase(first) || TAIL_WORDS.contains(&tail.as_str()) || current.text.ends_with('-') || lead_word);
            let wraps = current.page == next.page
                && (current.size - next.size).abs() <= 0.2
                && (next.top - current.top).abs() <= 90.0
                && unfinished
                && (short_wrapped || long_wrapped_title);
            if !wraps {
                break;
            }
            current.text = format!("{} {}", current.text, next.text);
            iter.next();
        }
        merged.push(current);
    }

    let mut levels: Vec<i64> = merged.iter().map(|c| (c.size * 10.0).round() as i64).collect();
    levels.sort_unstable_by(|a, b| b.cmp(a));
    levels.dedup();
    merged
        .into_iter()
        .map(|c| {
            let rank = levels.iter().position(|s| *s == (c.size * 10.0).round() as i64).unwrap_or(1);
            Heading { level: (rank as u32 + 1).min(3), title: c.text, page_start: c.page, source: "text-heuristic".into() }
        })
        .collect()
}

/// The document's title for single-file output when the headings had to be guessed:
/// the first few lines of at least 12 characters in the top 28% of page 1.
pub fn document_title(lines: &[Line], fallback: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for line in lines.iter().filter(|l| l.page == 1 && l.top <= l.page_height * 0.28) {
        let text = repair_mojibake(&line.text);
        if text.chars().count() < 12 {
            continue;
        }
        if parts.len() == 3 || (!parts.is_empty() && parts.join(" ").chars().count() + 1 + text.chars().count() > 140) {
            break;
        }
        parts.push(text);
    }
    let title = clean_line(&parts.join(" "));
    if title.is_empty() { fallback.to_string() } else { title }
}

/// Pick bookmarks or tags: tags win when there are no bookmarks, or when the
/// bookmarks are flat and the tags have more levels.
pub fn choose(bookmarks: Vec<Heading>, tags: Vec<Heading>) -> (Vec<Heading>, &'static str) {
    if bookmarks.is_empty() {
        let note = if tags.is_empty() { "" } else { "No embedded TOC found; using tagged-PDF headings" };
        return (tags, note);
    }
    if !tags.is_empty() {
        let levels = |v: &[Heading]| v.iter().map(|h| h.level).collect::<std::collections::HashSet<_>>().len();
        if levels(&tags) > levels(&bookmarks) && tags.len() >= bookmarks.len() / 2 {
            return (tags, "Bookmarks are flat; using tagged-PDF headings");
        }
    }
    (bookmarks, "")
}

/// Drop near-duplicates, keep pages non-decreasing, and never jump more than one level deeper.
pub fn normalize(outline: Vec<Heading>) -> Vec<Heading> {
    let mut cleaned: Vec<Heading> = Vec::new();
    let mut recent: Vec<(String, u32)> = Vec::new();
    let mut prev_level = 1;
    let mut last_page = 1;
    for item in outline {
        let title = repair_mojibake(&clean_line(&item.title));
        if title.is_empty() {
            continue;
        }
        let norm = normalize_match_text(&title);
        let page_start = item.page_start.max(last_page);
        let mut level = item.level.max(1);
        if !cleaned.is_empty() {
            level = level.min(prev_level + 1);
        }
        let near_repeat = recent.iter().rev().take(8).any(|(t, p)| *t == norm && page_start.abs_diff(*p) <= 2);
        let same_page_repeat = cleaned
            .last()
            .is_some_and(|c| c.page_start == page_start && normalize_match_text(&c.title) == norm);
        if near_repeat || same_page_repeat {
            continue;
        }
        recent.push((norm, page_start));
        prev_level = level;
        last_page = page_start;
        cleaned.push(Heading { level, title, page_start, ..item });
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(level: u32, title: &str, page: u32) -> Heading {
        Heading { level, title: title.into(), page_start: page, source: "pdf-tags".into() }
    }

    #[test]
    fn bookmark_footnotes_are_stripped() {
        assert_eq!(strip_bookmark_footnote("Avoiding the Dunning–Kruger Effect107"), "Avoiding the Dunning–Kruger Effect");
        assert_eq!(strip_bookmark_footnote("Part 1"), "Part 1");
        assert_eq!(strip_bookmark_footnote("Web3"), "Web3");
    }

    #[test]
    fn normalize_limits_level_jumps_and_repeats() {
        let out = normalize(vec![h(1, "Intro", 1), h(4, "Deep", 2), h(2, "Deep", 2)]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1].level, 2);
    }

    #[test]
    fn flat_bookmarks_lose_to_deeper_tags() {
        let marks = vec![h(1, "A", 1), h(1, "B", 2)];
        let tags = vec![h(1, "A", 1), h(2, "A.1", 1), h(1, "B", 2)];
        assert_eq!(choose(marks, tags).0.len(), 3);
    }
}
