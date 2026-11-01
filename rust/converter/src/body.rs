//! Section bodies: split pdf-inspector's whole-document Markdown at each
//! heading and clean every piece. Ported from `split_markdown_by_outline`,
//! `build_margin_noise_profile`, `find_furniture_lines`, `clean_hybrid_body`,
//! `format_dot_leader_blocks` and `deduplicate_body_paragraphs`.

use crate::layout::Line;
use crate::text::{normalize_margin_noise_text, normalize_match_text, starts_lowercase, strip_inline_markdown};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

pub const EMPTY_BODY_PLACEHOLDER: &str = "(No extractable text in this range.)";

static HEADING_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s+(.*?)\s*#*\s*$").unwrap());
static LIST_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(?:[-*+]|\d{1,3}[.)])\s+").unwrap());
static DOT_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.{5,}").unwrap());
static PAGE_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(page )?\d{1,4}$").unwrap());
static CATEGORIES_LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^categories( level \d)*$").unwrap());
static NOT_PROSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\||#|>|\*\*|[-*+]\s|\d{1,3}[.)]\s)").unwrap());
static SENTENCE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[.!?:;)"”|]$"#).unwrap());
static MULTI_BLANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
/// Photo captions and credits, which interrupt the text rather than continue it.
static CAPTION_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:BY[ -]|©|\(c\) |Photo|Credit|Source:|Image |Fig(?:ure|\.) ?\d|Table \d)").unwrap());
/// Where a footnote may begin: its number, then text starting with a capital, a quote
/// or a link ("10“All men…", "11Harari, Y.", "14UNESCO", "48https://…").
static FOOTNOTE_CANDIDATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?:^|\s)(\d{1,3})(\s?)(?:[A-Z]|[“"‘]|\[?https?://)"#).unwrap());
/// A footnote number set as superscript digits at a paragraph start or after a
/// sentence end ("… Simon and Schuster. ³⁵Understanding"). In-text markers follow a
/// word and are followed by lowercase ("tools¹⁸ and"), so they don't match.
static SUPERSCRIPT_FOOTNOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(^|[.)\]”"’>]\s+)([⁰¹²³⁴⁵⁶⁷⁸⁹]{1,3})([A-Z“"‘\[]|https?://)"#).unwrap());

fn superscript_to_ascii(digits: &str) -> String {
    digits
        .chars()
        .map(|c| match c {
            '⁰' => '0', '¹' => '1', '²' => '2', '³' => '3', '⁴' => '4',
            '⁵' => '5', '⁶' => '6', '⁷' => '7', '⁸' => '8', _ => '9',
        })
        .collect()
}

/// Split one paragraph into its body text and the footnotes in it.
///
/// Footnote numbers count up through a document, and in-text markers are
/// superscripts ("tools¹⁸"), so a plain number only starts a footnote when it is the
/// next one in sequence. At the start of a paragraph a jump of up to ten (footnotes
/// missed in extraction) or a restart at 1 (numbering per chapter) is also accepted.
/// `last` carries the last footnote number from one section to the next.
fn split_footnotes(para: &str, last: &mut u32) -> (String, Vec<String>) {
    let para = SUPERSCRIPT_FOOTNOTE
        .replace_all(para, |c: &regex::Captures| format!("{}{}{}", &c[1], superscript_to_ascii(&c[2]), &c[3]))
        .into_owned();
    let para = para.as_str();
    let mut starts: Vec<(usize, usize)> = Vec::new(); // (number start, number end)
    for caps in FOOTNOTE_CANDIDATE.captures_iter(para) {
        let number = caps.get(1).unwrap();
        let glued = caps[2].is_empty();
        let n: u32 = number.as_str().parse().unwrap_or(0);
        let before = para[..number.start()].trim_end();
        let at_start = before.is_empty();
        // Mid-paragraph, a footnote follows the end of something: a sentence, a link, a quote.
        let after_an_end = before.ends_with(['.', ')', ']', '”', '"', '’', '>']);
        // A number glued to text at a paragraph start is strong evidence on its own,
        // which lets the count recover after footnotes that weren't recognised.
        let accepted = (n == *last + 1 && (at_start || after_an_end))
            || (at_start && (n == 1 || (n > *last && (glued || n <= *last + 10))));
        if accepted {
            *last = n;
            starts.push((number.start(), number.end()));
        }
    }
    let Some(&(first, _)) = starts.first() else {
        return (para.to_string(), Vec::new());
    };
    let notes = starts
        .iter()
        .enumerate()
        .map(|(i, &(num_start, num_end))| {
            let end = starts.get(i + 1).map_or(para.len(), |&(next, _)| next);
            format!("{} {}", &para[num_start..num_end], para[num_end..end].trim())
        })
        .collect();
    (para[..first].trim().to_string(), notes)
}
static DOT_LEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(.+?)\s*(?:\.{3,}|(?:\.\s*){3,})\s*(?:<sup>)?(\d{1,3})(?:</sup>)?\s*$").unwrap());

/// Text of a Markdown line with any heading marks removed, normalized for matching.
fn line_norm(stripped: &str) -> (bool, String) {
    match HEADING_LINE.captures(stripped) {
        Some(caps) => (true, normalize_match_text(&strip_inline_markdown(&caps[1]))),
        None => (false, normalize_match_text(&strip_inline_markdown(stripped))),
    }
}

/// Split whole-document Markdown into one body per heading.
///
/// Headings are matched in order, and a match must fall within a few pages'
/// worth of lines of where the heading's page says it should be, so one false
/// match cannot swallow the rest of the document. Unmatched headings get an
/// empty body; their text stays with the preceding section. Returns the bodies
/// (aligned with `titles`) and the text before the first match.
pub fn split_by_outline(markdown: &str, titles: &[(String, u32, bool)], page_count: u32) -> (Vec<String>, String) {
    let lines: Vec<&str> = markdown.lines().collect();
    let parsed: Vec<(bool, String)> = lines.iter().map(|l| line_norm(l.trim())).collect();
    let lines_per_page = (lines.len() as f64 / page_count.max(1) as f64).max(1.0);

    let mut positions: Vec<(usize, usize)> = Vec::new();
    let (mut cursor, mut anchor_line, mut anchor_page) = (0usize, 0usize, 1u32);
    for (idx, (title, page, skip)) in titles.iter().enumerate() {
        let title_norm = normalize_match_text(title);
        if *skip || title_norm.is_empty() {
            continue;
        }
        let page = if *page == 0 { anchor_page } else { *page };
        let span = ((page.saturating_sub(anchor_page) + 3) as f64 * lines_per_page * 1.5) as usize;
        let limit = (anchor_line + span).min(lines.len());
        // Prefer a Markdown heading line; fall back to a short plain line with the
        // exact title, since the same words often appear earlier in a list.
        let mut found = None;
        let mut plain_fallback = None;
        for i in cursor..limit.max(cursor) {
            let (is_heading, norm) = &parsed[i];
            if norm.is_empty() {
                continue;
            }
            let loose = title_norm.len() >= 8 && norm.len() >= 8 && (norm.contains(&title_norm) || title_norm.contains(norm.as_str()));
            if *is_heading && (*norm == title_norm || loose) {
                found = Some(i);
                break;
            }
            if plain_fallback.is_none() && *norm == title_norm && lines[i].trim().chars().count() <= title.chars().count() + 12 {
                plain_fallback = Some(i);
            }
        }
        if let Some(line_no) = found.or(plain_fallback) {
            positions.push((idx, line_no));
            cursor = line_no + 1;
            anchor_line = line_no;
            anchor_page = page;
        }
    }

    let mut bodies = vec![String::new(); titles.len()];
    positions.sort_by_key(|&(_, line_no)| line_no);
    for (n, &(idx, line_no)) in positions.iter().enumerate() {
        let end = positions.get(n + 1).map_or(lines.len(), |&(_, next)| next);
        bodies[idx] = lines[line_no + 1..end].join("\n").trim().to_string();
    }
    let lead_end = positions.first().map_or(lines.len(), |&(_, line_no)| line_no);
    (bodies, lines[..lead_end].join("\n").trim().to_string())
}

/// Text repeated in the top or bottom tenth of many pages: running headers and footers.
pub fn margin_noise(lines: &[Line], page_count: u32) -> HashSet<String> {
    let min_repeat = 2usize.max((page_count as f64 * 0.15).ceil() as usize);
    let mut top: HashMap<String, HashSet<u32>> = HashMap::new();
    let mut bottom: HashMap<String, HashSet<u32>> = HashMap::new();
    for line in lines {
        let norm = normalize_margin_noise_text(&line.text);
        if norm.is_empty() {
            continue;
        }
        if line.top <= line.page_height * 0.10 {
            top.entry(norm.clone()).or_default().insert(line.page);
        }
        if line.baseline <= line.page_height * 0.10 {
            bottom.entry(norm).or_default().insert(line.page);
        }
    }
    top.into_iter()
        .chain(bottom)
        .filter(|(text, pages)| pages.len() >= min_repeat && text.len() >= 4)
        .map(|(text, _)| text)
        .collect()
}

/// Short standalone lines repeated five or more times (margin labels, running headers),
/// plus the margin noise found from page positions.
///
/// Repeated headings are usually real labels ("Specific expectations"), so a heading
/// only counts when it is link-styled (underlined, like "View in ASL").
pub fn furniture_lines(markdown: &str, margin: &HashSet<String>) -> HashSet<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for line in markdown.lines() {
        let stripped = line.trim();
        if stripped.is_empty() || stripped.starts_with('|') || LIST_ITEM.is_match(stripped) {
            continue;
        }
        let (is_heading, norm) = line_norm(stripped);
        if is_heading && !stripped.contains("<u>") {
            continue;
        }
        if !norm.is_empty() && norm.split_whitespace().count() <= 10 {
            *counts.entry(norm).or_default() += 1;
        }
    }
    // Labels: short lines (four words or fewer) repeated five or more times.
    let mut furniture: HashSet<String> =
        counts.iter().filter(|(t, n)| **n >= 5 && t.split_whitespace().count() <= 4).map(|(t, _)| t.clone()).collect();
    // Footers that carry the page number ("Report Title 02", "Report Title 03") repeat
    // only once the number is ignored; count those (three or more words) that way too.
    let mut by_footer: HashMap<String, usize> = HashMap::new();
    for (text, n) in &counts {
        let footer = normalize_margin_noise_text(text);
        if footer.split_whitespace().count() >= 3 && footer != *text {
            *by_footer.entry(footer).or_default() += n;
        }
    }
    furniture.extend(by_footer.into_iter().filter(|(_, n)| *n >= 5).map(|(t, _)| t));
    furniture.extend(margin.iter().cloned());
    furniture
}

/// Clean one section body: demote stray headings to bold, drop furniture and
/// page-number lines, separate lists from following text, and rejoin sentences
/// split by a removed line.
pub fn clean_section(text: &str, furniture: &HashSet<String>, last_footnote: &mut u32) -> String {
    let mut kept: Vec<String> = Vec::new();
    for raw in text.lines() {
        let mut line = raw.to_string();
        let mut stripped = raw.trim().to_string();
        if let Some(caps) = HEADING_LINE.captures(&stripped) {
            let plain = strip_inline_markdown(&caps[1]);
            line = if plain.is_empty() { String::new() } else { format!("**{plain}**") };
            stripped = line.clone();
        }
        if DOT_RUN.is_match(&stripped) {
            // Dot-leader ToC lines must be plain for format_dot_leaders.
            stripped = strip_inline_markdown(&stripped);
            line = stripped.clone();
        }
        if !stripped.is_empty() && !stripped.starts_with('|') && !LIST_ITEM.is_match(&stripped) {
            let plain = strip_inline_markdown(&stripped);
            let norm = normalize_match_text(&plain);
            let probe = if norm.is_empty() { "x" } else { norm.as_str() };
            // Running headers and footers may carry the page number ("Title Page 3").
            let is_furniture = furniture.contains(&norm) || furniture.contains(&normalize_margin_noise_text(&plain));
            if is_furniture || PAGE_NUMBER.is_match(probe) || CATEGORIES_LABEL.is_match(&norm) {
                continue;
            }
        }
        kept.push(line);
    }

    let mut with_breaks: Vec<String> = Vec::new();
    for line in kept {
        let prev_is_list = with_breaks.last().is_some_and(|p| LIST_ITEM.is_match(p));
        if !line.trim().is_empty() && prev_is_list && !LIST_ITEM.is_match(&line) && !line.starts_with(char::is_whitespace) {
            with_breaks.push(String::new());
        }
        with_breaks.push(line);
    }
    let body = MULTI_BLANK.replace_all(&with_breaks.join("\n"), "\n\n").trim().to_string();

    // Footnotes sit at the bottom of each page and would interrupt sentences that
    // run onto the next page; move them to the end of the section.
    let mut footnotes: Vec<String> = Vec::new();
    let mut paragraphs: Vec<String> = Vec::new();
    for para in body.split("\n\n").filter(|p| !p.trim().is_empty()) {
        // Tables and headings never hold footnotes.
        if para.starts_with('|') || para.starts_with("**") {
            paragraphs.push(para.to_string());
            continue;
        }
        let (text, notes) = split_footnotes(para, last_footnote);
        if !text.is_empty() {
            paragraphs.push(text);
        }
        footnotes.extend(notes);
    }

    let mut stitched: Vec<String> = Vec::new();
    for para in paragraphs {
        if let Some(prev) = stitched.last_mut() {
            let prev_last = prev.lines().last().unwrap_or("").trim().to_string();
            let first = para.trim_start();
            // Rejoin a sentence split across paragraphs: the next part starts in lowercase,
            // or this part ends with a comma ("(Estache & Rossi," / "2002) and ...").
            // A bracket that opens in lowercase after an unfinished sentence continues it too
            // ("... a monopoly of provision" / "(e.g. urban piped water supply) ...").
            let bracket_continues = first.starts_with('(')
                && starts_lowercase(&first[1..])
                && prev_last.chars().last().is_some_and(char::is_lowercase);
            let continues = (starts_lowercase(first) && !SENTENCE_END.is_match(&prev_last))
                || bracket_continues
                || (prev_last.ends_with([',', '&']) && !NOT_PROSE.is_match(first) && !CAPTION_START.is_match(first));
            if !NOT_PROSE.is_match(&prev_last) && continues {
                *prev = format!("{} {}", prev.trim_end(), first);
                continue;
            }
        }
        stitched.push(para);
    }
    stitched.extend(footnotes);
    stitched.join("\n\n")
}

/// Turn runs of three or more "Title ........ 12" lines into a Section/Page table.
pub fn format_dot_leaders(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let mut j = i;
        let mut rows: Vec<(String, String)> = Vec::new();
        while j < lines.len() {
            if lines[j].trim().is_empty() {
                if rows.is_empty() {
                    break;
                }
                j += 1;
                continue;
            }
            let Some(caps) = DOT_LEADER.captures(lines[j]) else { break };
            rows.push((caps[1].trim().to_string(), caps[2].to_string()));
            j += 1;
        }
        if rows.len() >= 3 {
            out.push("| Section | Page |".into());
            out.push("|---|---:|".into());
            for (title, page) in rows {
                out.push(format!("| {} | {page} |", title.replace('|', "\\|")));
            }
            out.push(String::new());
            i = j;
            continue;
        }
        out.push(lines[i].to_string());
        i += 1;
    }
    out.join("\n")
}

/// Drop paragraphs repeated within a section (exact, or 94% similar with the same opening words).
pub fn deduplicate_paragraphs(text: &str) -> String {
    let paragraphs: Vec<&str> = text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()).collect();
    if paragraphs.is_empty() {
        return text.to_string();
    }
    let is_candidate = |p: &str| {
        !(p.starts_with('|') || p.starts_with('#') || p.starts_with("- Level:") || p.starts_with("- Pages:") || p.starts_with("- Source:"))
            && p.chars().count() >= 80
            && p.split_whitespace().count() >= 12
    };
    let mut kept: Vec<&str> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut by_prefix: HashMap<String, Vec<String>> = HashMap::new();
    for para in paragraphs {
        let norm = normalize_match_text(para);
        if norm.is_empty() {
            continue;
        }
        if !is_candidate(para) {
            kept.push(para);
            continue;
        }
        if seen.contains(&norm) {
            continue;
        }
        let prefix = norm.split_whitespace().take(10).collect::<Vec<_>>().join(" ");
        let near_dup = by_prefix
            .get(&prefix)
            .is_some_and(|prev| prev.iter().any(|p| similar::TextDiff::from_chars(p.as_str(), norm.as_str()).ratio() >= 0.94));
        if near_dup {
            continue;
        }
        seen.insert(norm.clone());
        by_prefix.entry(prefix).or_default().push(norm);
        kept.push(para);
    }
    if kept.is_empty() {
        EMPTY_BODY_PLACEHOLDER.to_string()
    } else {
        kept.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_matching_headings() {
        let md = "Cover\n\n# Intro\nHello\n\n## Details\nMore text";
        let titles = vec![("Intro".to_string(), 1, false), ("Details".to_string(), 1, false)];
        let (bodies, lead) = split_by_outline(md, &titles, 1);
        assert_eq!(lead, "Cover");
        assert_eq!(bodies, vec!["Hello".to_string(), "More text".to_string()]);
    }

    #[test]
    fn cleans_page_numbers_and_rejoins_sentences() {
        let furniture = HashSet::from(["running header".to_string()]);
        let body = clean_section("This sentence is cut\n\n12\n\nRunning Header\n\nacross a page.", &furniture, &mut 0);
        assert_eq!(body, "This sentence is cut across a page.");
        let comma = clean_section("compared utilities (Estache & Rossi,\n\n2002) and a review.", &HashSet::new(), &mut 0);
        assert_eq!(comma, "compared utilities (Estache & Rossi, 2002) and a review.");
        let bracket = clean_section("a monopoly of provision\n\n(e.g. urban piped water supply), or", &HashSet::new(), &mut 0);
        assert_eq!(bracket, "a monopoly of provision (e.g. urban piped water supply), or");
        let caption = clean_section("less efficiency (e.g. France,\n\nBY Denis Bocquet / *Graffiti in Turin.*", &HashSet::new(), &mut 0);
        assert_eq!(caption, "less efficiency (e.g. France,\n\nBY Denis Bocquet / *Graffiti in Turin.*");
    }

    #[test]
    fn moves_footnotes_out_of_split_sentences() {
        let mut last = 9;
        let body = clean_section("Not meant to be an in-depth\n\n10“All men are mortal.” 11Harari, Y. (2018).\n\nacademic piece.", &HashSet::new(), &mut last);
        assert_eq!(body, "Not meant to be an in-depth academic piece.\n\n10 “All men are mortal.”\n\n11 Harari, Y. (2018).");
        assert_eq!(last, 11);
        // Superscript footnote numbers after a sentence end are recognised too.
        let body = clean_section("Search for meaning. ³⁵Understanding is key.", &HashSet::new(), &mut 34);
        assert_eq!(body, "Search for meaning.\n\n35 Understanding is key.");
        // In-text superscript markers and numbers that are not the next footnote stay in the text.
        assert_eq!(clean_section("various tools¹⁸ and processes", &HashSet::new(), &mut 17), "various tools¹⁸ and processes");
        let kept = clean_section("21st century skills matter. In 12 Months we grew.", &HashSet::new(), &mut last);
        assert_eq!(kept, "21st century skills matter. In 12 Months we grew.");
    }

    #[test]
    fn builds_toc_table() {
        let out = format_dot_leaders("Intro ........ 1\nMethods ...... 4\nResults ...... 9");
        assert!(out.starts_with("| Section | Page |"));
        assert!(out.contains("| Methods | 4 |"));
    }

    #[test]
    fn removes_repeated_paragraphs() {
        let p = "This paragraph is long enough to count as a candidate for duplicate removal, it has many words.";
        assert_eq!(deduplicate_paragraphs(&format!("{p}\n\n{p}")), p);
    }
}
