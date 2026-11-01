//! Reading order. pdf-inspector's Markdown keeps formatting (bold, links,
//! tables) but sometimes reads a two-column page out of order, for example
//! the top of the right column before the end of the left. Here each page's
//! paragraphs are located on the page and, on pages that really have two
//! columns, re-sorted: full-width items in place, then within each band the
//! left column top to bottom before the right column.

use crate::layout::Line;
use crate::text::{normalize_match_text, strip_inline_markdown};
use std::collections::HashMap;

/// Document Markdown with two-column pages put in reading order. The whole-document
/// Markdown stays the base (it joins paragraphs, lists and tables across page
/// breaks); each paragraph is assigned to the page where it starts, and only the
/// paragraphs of pages that really have two columns are re-sorted. Pull-quotes
/// are taken out of the text on any page. Returns the Markdown and the pages that changed.
pub fn document_markdown(lines: &[Line], page_widths: &HashMap<u32, f32>, whole: &str) -> (String, Vec<u32>) {
    let paragraphs: Vec<&str> = whole.split("\n\n").filter(|p| !p.trim().is_empty()).collect();
    let pages = assign_pages(&paragraphs, lines);
    let mut by_page: HashMap<u32, Vec<&Line>> = HashMap::new();
    for line in lines {
        by_page.entry(line.page).or_default().push(line);
    }
    let mut out: Vec<String> = Vec::with_capacity(paragraphs.len());
    let mut changed = Vec::new();
    let mut i = 0;
    while i < paragraphs.len() {
        let page = pages[i];
        let mut j = i;
        while j < paragraphs.len() && pages[j] == page {
            j += 1;
        }
        let group = paragraphs[i..j].join("\n\n");
        let page_lines = by_page.get(&page).map(Vec::as_slice).unwrap_or(&[]);
        let width = page_widths.get(&page).copied().unwrap_or(612.0);
        // Pull-quotes can sit on any page. Paragraphs run across page breaks and their
        // page is found from their opening words, so look on the neighbouring pages too.
        let quote_lines: Vec<&Line> = (page.saturating_sub(1)..=page + 1)
            .flat_map(|p| by_page.get(&p).map(Vec::as_slice).unwrap_or(&[]).iter().copied())
            .collect();
        let text = extract_pull_quotes(&group, &quote_lines);
        let text = two_column_split(page_lines, width).and_then(|mid| reorder(&text, page_lines, mid)).unwrap_or(text);
        if text != group {
            changed.push(page);
        }
        out.push(text);
        i = j;
    }
    (out.join("\n\n"), changed)
}

/// Put page texts that are missing from the document Markdown (hidden text layers)
/// in at their page: before the first paragraph that starts on a later page.
///
/// A hidden layer has one paragraph per printed line, so a heading that wraps
/// (a slide title over three lines) can't be matched by its text. The page is
/// known, though: each heading that starts on the page is put at the top of its
/// text as a Markdown heading, and the lines that spell it out are removed.
pub fn insert_pages(whole: &str, lines: &[Line], pages: &[(u32, String)], headings: &[crate::outline::Heading]) -> String {
    if pages.is_empty() {
        return whole.to_string();
    }
    let pages: Vec<(u32, String)> = pages
        .iter()
        .map(|(page, text)| {
            let mut paras: Vec<String> = text.split("\n\n").map(str::to_string).collect();
            let mut titled = Vec::new();
            for heading in headings.iter().filter(|h| h.page_start == *page) {
                remove_spelled_out(&mut paras, &heading.title);
                titled.push(format!("# {}", heading.title));
            }
            titled.extend(paras);
            (*page, titled.join("\n\n"))
        })
        .collect();
    let pages = &pages;
    let paragraphs: Vec<&str> = whole.split("\n\n").filter(|p| !p.trim().is_empty()).collect();
    let on_page = assign_pages(&paragraphs, lines);
    let mut out: Vec<String> = Vec::with_capacity(paragraphs.len() + pages.len());
    let mut pending = pages.iter().peekable();
    for (paragraph, page) in paragraphs.iter().zip(&on_page) {
        while pending.peek().is_some_and(|(p, _)| p < page) {
            out.push(pending.next().unwrap().1.clone());
        }
        out.push(paragraph.to_string());
    }
    out.extend(pending.map(|(_, text)| text.clone()));
    out.join("\n\n")
}

/// Remove the run of consecutive paragraphs (within the first few) that together
/// spell out `title`.
fn remove_spelled_out(paras: &mut Vec<String>, title: &str) {
    let target = normalize_match_text(title);
    for start in 0..paras.len().min(8) {
        let mut joined = String::new();
        for end in start..paras.len().min(start + 6) {
            if !joined.is_empty() {
                joined.push(' ');
            }
            joined.push_str(&normalize_match_text(&paras[end]));
            if joined == target {
                paras.drain(start..=end);
                return;
            }
            if !target.starts_with(&joined) {
                break;
            }
        }
    }
}

/// The page each paragraph starts on: its opening words are looked up in the page
/// lines, moving forward through the document. A paragraph that can't be found
/// stays on the page of the one before it.
fn assign_pages(paragraphs: &[&str], lines: &[Line]) -> Vec<u32> {
    let norms: Vec<String> = lines.iter().map(|l| normalize_match_text(&l.text)).collect();
    let mut cursor = 0;
    let mut page = lines.first().map_or(1, |l| l.page);
    paragraphs
        .iter()
        .map(|p| {
            let key = paragraph_key(p);
            let opening: String = key.split_whitespace().take(5).collect::<Vec<_>>().join(" ");
            if !key.is_empty() {
                // Look ahead a few pages at most, so a repeated phrase can't jump far.
                // Short paragraphs (a one-word heading) must match a whole line.
                let limit_page = page + 3;
                let long = opening.split_whitespace().count() >= 3;
                let hit = (cursor..lines.len()).take_while(|&k| lines[k].page <= limit_page).find(|&k| {
                    let n = &norms[k];
                    !n.is_empty()
                        && (*n == key
                            || (long && (n.starts_with(&opening) || (n.split_whitespace().count() >= 3 && key.starts_with(n.as_str())))))
                });
                if let Some(k) = hit {
                    cursor = k;
                    page = lines[k].page;
                }
            }
            page
        })
        .collect()
}

/// Pull-quotes: large text in a box beside a column that pdf-inspector weaves into
/// the body text line by line ("(Burgess & Wilson, ***efficiency as they*** 1996;").
/// A block of lines clearly larger than the page's body text counts as a pull-quote
/// when at least one of its lines sits inside a body paragraph (so headings and
/// labels are left alone). Its lines are cut out of the text and the whole quote is
/// put back as a blockquote after the last paragraph it interrupted.
fn extract_pull_quotes(markdown: &str, lines: &[&Line]) -> String {
    let mut sizes: Vec<f32> = lines.iter().map(|l| l.size).filter(|s| *s > 0.0).collect();
    if sizes.len() < 10 {
        return markdown.to_string();
    }
    sizes.sort_by(f32::total_cmp);
    let body = sizes[sizes.len() / 2];
    let mut big: Vec<&Line> = lines.iter().copied().filter(|l| l.size >= body * 1.15 && !l.text.trim().is_empty()).collect();
    big.sort_by(|a, b| a.page.cmp(&b.page).then(a.top.total_cmp(&b.top)));
    // Blocks: big lines stacked closely with the same left edge.
    let mut blocks: Vec<Vec<&Line>> = Vec::new();
    for line in big {
        match blocks.last_mut() {
            Some(block) if {
                let last = block.last().unwrap();
                line.page == last.page && (line.left - last.left).abs() <= 6.0 && line.top - last.top <= 1.8 * line.size
            } => block.push(line),
            _ => blocks.push(vec![line]),
        }
    }
    let mut paragraphs: Vec<String> = markdown.split("\n\n").filter(|p| !p.trim().is_empty()).map(str::to_string).collect();
    for block in blocks.into_iter().filter(|b| b.len() >= 2) {
        let patterns: Vec<regex::Regex> = block
            .iter()
            // Only emphasised text: pull-quotes are set in bold or bold italic.
            .filter_map(|l| regex::Regex::new(&format!(r"\*{{1,3}}\s*{}\s*\*{{1,3}}", regex::escape(l.text.trim()))).ok())
            .collect();
        // Embedded: a paragraph holding some of the quote's lines still has plenty of
        // other text once they are taken out. (A paragraph simply set in large type,
        // like a strand introduction, has nothing left.)
        let embedded = paragraphs.iter().any(|p| {
            if p.starts_with(['#', '>']) || !patterns.iter().any(|re| re.is_match(p)) {
                return false;
            }
            let rest = patterns.iter().fold(p.clone(), |acc, re| re.replace_all(&acc, " ").into_owned());
            normalize_match_text(&strip_inline_markdown(&rest)).len() > 60
        });
        let matching = paragraphs.iter().filter(|p| patterns.iter().any(|re| re.is_match(p))).count();
        debug(&format!(
            "p{}: quote block '{}' ({} lines) embedded={embedded}; {} of {} paragraphs match; first paragraph '{}'",
            block[0].page, block[0].text, block.len(), matching, paragraphs.len(), paragraphs.first().map_or("", |p| &p[..p.len().min(50)])
        ));
        if !embedded {
            continue;
        }
        let line_texts: Vec<String> = block.iter().map(|l| normalize_match_text(&l.text)).collect();
        let mut last_touched = None;
        for (i, p) in paragraphs.iter_mut().enumerate() {
            // pdf-inspector sometimes makes a quote line a heading ("# fail to measure the").
            if p.starts_with('#') && line_texts.contains(&normalize_match_text(&strip_inline_markdown(p.trim_start_matches('#')))) {
                p.clear();
                last_touched = Some(i);
                continue;
            }
            if p.starts_with(['#', '|', '>']) {
                continue;
            }
            let mut changed = p.clone();
            for re in &patterns {
                changed = re.replace_all(&changed, " ").into_owned();
            }
            if changed != *p {
                *p = crate::text::clean_line(&changed);
                last_touched = Some(i);
            }
        }
        if let Some(mut i) = last_touched {
            // After the end of the interrupted sentence: skip removed pieces and
            // continuations that start in lowercase.
            while i + 1 < paragraphs.len() {
                let next = paragraphs[i + 1].trim();
                if next.is_empty() || crate::text::starts_lowercase(next) {
                    i += 1;
                } else {
                    break;
                }
            }
            let quote = block.iter().map(|l| l.text.trim()).collect::<Vec<_>>().join(" ");
            paragraphs.insert(i + 1, format!("> {quote}"));
        }
        paragraphs.retain(|p| !p.trim().is_empty());
    }
    paragraphs.join("\n\n")
}

/// The x position dividing two columns, when the page clearly has two: at least
/// five body lines entirely on each side of the centre and few lines crossing it.
fn two_column_split(lines: &[&Line], width: f32) -> Option<f32> {
    let mut sizes: Vec<f32> = lines.iter().map(|l| l.size).filter(|s| *s > 0.0).collect();
    if sizes.len() < 10 {
        return None;
    }
    sizes.sort_by(f32::total_cmp);
    let body = sizes[sizes.len() / 2];
    let mid = width / 2.0;
    let body_lines: Vec<&&Line> = lines
        .iter()
        // Skip running headers and footers in the top and bottom margins.
        .filter(|l| (l.size - body).abs() <= 1.5 && l.text.chars().count() >= 20 && l.top > l.page_height * 0.06 && l.baseline > l.page_height * 0.06)
        .collect();
    // Text in a page column fills most of the column; table cells are much narrower.
    let wide = |l: &&&Line| l.right - l.left >= 0.6 * mid;
    let left = body_lines.iter().filter(|l| l.right <= mid + 10.0 && wide(l)).count();
    let right = body_lines.iter().filter(|l| l.left >= mid - 10.0 && wide(l)).count();
    let crossing = body_lines.iter().filter(|l| l.left < mid - 20.0 && l.right > mid + 20.0).count();
    (left >= 5 && right >= 5 && crossing * 10 < (left + right) * 3).then_some(mid)
}

#[derive(Clone, Copy, PartialEq)]
enum Place {
    Full,
    Left,
    Right,
}

/// Normalized text that a paragraph's first line on the page should start with.
fn paragraph_key(paragraph: &str) -> String {
    let first = paragraph.trim_start();
    let text = if first.starts_with('|') {
        // A table: its first cell.
        first.trim_start_matches('|').split('|').next().unwrap_or("").to_string()
    } else {
        first.trim_start_matches('#').to_string()
    };
    normalize_match_text(&strip_inline_markdown(&text))
}

fn place_of(line: &Line, mid: f32) -> Place {
    if line.left < mid - 20.0 && line.right > mid + 20.0 {
        Place::Full
    } else if line.left < mid {
        Place::Left
    } else {
        Place::Right
    }
}

/// A paragraph or table read across both columns side by side (pdf-inspector
/// sometimes merges a column's lines with the lines beside them). Returns the page
/// lines it was made of, as paragraphs with their place on the page, or `None` when
/// it is genuine: a real table, or a sentence running from the bottom of one column
/// to the top of the next.
fn split_across_columns(paragraph: &str, lines: &[&Line], norms: &[String], mid: f32) -> Option<Vec<(String, Place, f32)>> {
    if paragraph.trim_start().starts_with('|') {
        return split_fake_table(paragraph, lines, norms, mid);
    }
    let para_norm = normalize_match_text(&strip_inline_markdown(paragraph));
    let anchors: Vec<usize> = (0..lines.len())
        .filter(|&i| norms[i].split_whitespace().count() >= 3 && para_norm.contains(norms[i].as_str()))
        .collect();
    let tops = |place: Place| anchors.iter().filter(|&&i| place_of(lines[i], mid) == place).map(|&i| lines[i].top).collect::<Vec<_>>();
    let (left_tops, right_tops) = (tops(Place::Left), tops(Place::Right));
    debug(&format!("p{}: '{}' anchors left {:?} right {:?}", lines.first().map_or(0, |l| l.page), paragraph.chars().take(40).collect::<String>(), left_tops, right_tops));
    if left_tops.is_empty() || right_tops.is_empty() {
        return None;
    }
    // Side by side: some right-column line sits level with the left-column lines.
    let size = anchors.iter().map(|&i| lines[i].size).fold(1.0, f32::max);
    let (low, high) = (left_tops.iter().copied().fold(f32::MAX, f32::min), left_tops.iter().copied().fold(f32::MIN, f32::max));
    if !right_tops.iter().any(|t| *t >= low - 2.0 * size && *t <= high + 2.0 * size) {
        return None;
    }
    let members: Vec<usize> = (0..lines.len())
        .filter(|&i| !norms[i].is_empty() && place_of(lines[i], mid) != Place::Full && para_norm.contains(norms[i].as_str()))
        .collect();
    Some(column_paragraphs(&members, lines, mid))
}

/// Diagnostics, shown when PDF_TO_MD_DEBUG_READING is set.
fn debug(message: &str) {
    if std::env::var_os("PDF_TO_MD_DEBUG_READING").is_some() {
        eprintln!("reading: {message}");
    }
}

/// True when the rebuilt paragraphs hold every word of the original.
fn keeps_all_words(original: &str, parts: &[(String, Place, f32)]) -> bool {
    let rebuilt: std::collections::HashSet<String> =
        parts.iter().flat_map(|(t, _, _)| normalize_match_text(t).split_whitespace().map(str::to_string).collect::<Vec<_>>()).collect();
    // pdf-inspector sometimes fuses two words ("paperFigure"); accept a word that
    // splits into two rebuilt words.
    let covered = |w: &str| {
        rebuilt.contains(w) || (1..w.len()).any(|k| w.is_char_boundary(k) && rebuilt.contains(&w[..k]) && rebuilt.contains(&w[k..]))
    };
    normalize_match_text(&strip_inline_markdown(&original.replace('|', " "))).split_whitespace().all(covered)
}

/// A Markdown table whose cells come from both columns is two columns read
/// across, not a table.
fn split_fake_table(table: &str, lines: &[&Line], norms: &[String], mid: f32) -> Option<Vec<(String, Place, f32)>> {
    let table_norm = normalize_match_text(&strip_inline_markdown(&table.replace('|', " ")));
    // Lines of two or more words that clearly belong to the table...
    let anchors: Vec<usize> = (0..lines.len())
        .filter(|&i| norms[i].split_whitespace().count() >= 2 && table_norm.contains(norms[i].as_str()))
        .collect();
    let in_column = |place: Place| anchors.iter().filter(|&&i| place_of(lines[i], mid) == place).count();
    // Two page columns read across: the left one holds full-width column text,
    // not the narrow cells of a real table.
    let wide_left = anchors.iter().filter(|&&i| place_of(lines[i], mid) == Place::Left && lines[i].right - lines[i].left >= 0.6 * mid).count();
    if in_column(Place::Left) < 2 || in_column(Place::Right) < 2 || wide_left < 2 {
        return None;
    }
    // ...and every line in their vertical span, so single words and page numbers
    // ("Singapore", "4") come along too.
    let first_top = anchors.iter().map(|&i| lines[i].top).fold(f32::MAX, f32::min);
    let last_top = anchors.iter().map(|&i| lines[i].top).fold(f32::MIN, f32::max);
    let reach = 1.5 * anchors.iter().map(|&i| lines[i].size).fold(0.0, f32::max);
    let members: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].top >= first_top - 1.0 && lines[i].top <= last_top + reach && place_of(lines[i], mid) != Place::Full)
        .collect();
    Some(column_paragraphs(&members, lines, mid))
}

/// Group page lines into paragraphs per column (lines close together join),
/// each with its place and top for sorting.
fn column_paragraphs(members: &[usize], lines: &[&Line], mid: f32) -> Vec<(String, Place, f32)> {
    let out = column_paragraphs_raw(members, lines, mid);
    // Plain page text: a leading '#' ("#08-01, Block A") is not a Markdown heading.
    out.into_iter().map(|(t, p, top)| (escape_markdown_start(&t), p, top)).collect()
}

fn escape_markdown_start(text: &str) -> String {
    if text.starts_with(['#', '>', '|']) { format!("\\{text}") } else { text.to_string() }
}

fn column_paragraphs_raw(members: &[usize], lines: &[&Line], mid: f32) -> Vec<(String, Place, f32)> {
    let mut members = members.to_vec();
    members.sort_by(|&a, &b| lines[a].top.total_cmp(&lines[b].top).then(lines[a].left.total_cmp(&lines[b].left)));
    let left: Vec<usize> = members.iter().copied().filter(|&i| place_of(lines[i], mid) == Place::Left).collect();
    let right: Vec<usize> = members.iter().copied().filter(|&i| place_of(lines[i], mid) == Place::Right).collect();
    let mut out = Vec::new();
    for (column, place) in [(left, Place::Left), (right, Place::Right)] {
        // Lines close together form one paragraph.
        let mut current: Vec<usize> = Vec::new();
        for i in column {
            if let Some(&prev) = current.last() {
                let gap = lines[i].top - (lines[prev].page_height - lines[prev].baseline);
                if gap > 0.8 * lines[i].size.max(1.0) {
                    out.push((current.iter().map(|&k| lines[k].text.as_str()).collect::<Vec<_>>().join(" "), place, lines[current[0]].top));
                    current.clear();
                }
            }
            current.push(i);
        }
        if let Some(&first) = current.first() {
            out.push((current.iter().map(|&k| lines[k].text.as_str()).collect::<Vec<_>>().join(" "), place, lines[first].top));
        }
    }
    out
}

/// Re-sort one page's paragraphs into column reading order. `None` when too few
/// paragraphs can be located on the page to do so safely.
fn reorder(markdown: &str, lines: &[&Line], mid: f32) -> Option<String> {
    let norms: Vec<String> = lines.iter().map(|l| normalize_match_text(&l.text)).collect();
    // Paragraphs, with fake two-column tables replaced by their lines (already placed).
    let mut paragraphs: Vec<String> = Vec::new();
    let mut preset: Vec<Option<(Place, f32)>> = Vec::new();
    for para in markdown.split("\n\n").filter(|p| !p.trim().is_empty()) {
        let split = split_across_columns(para, lines, &norms, mid).filter(|parts| {
            let ok = keeps_all_words(para, parts);
            if !ok {
                debug(&format!("split of '{para}' would lose words; rebuilt: {:?}", parts.iter().map(|p| p.0.as_str()).collect::<Vec<_>>()));
            }
            ok
        });
        match split {
            Some(parts) => {
                for (text, place, top) in parts {
                    paragraphs.push(text);
                    preset.push(Some((place, top)));
                }
            }
            None => {
                paragraphs.push(para.to_string());
                preset.push(None);
            }
        }
    }
    let mut used = vec![false; lines.len()];
    let mut located: Vec<Option<(Place, f32)>> = Vec::with_capacity(paragraphs.len());
    for (paragraph, preset) in paragraphs.iter().zip(&preset) {
        if preset.is_some() {
            located.push(*preset);
            continue;
        }
        // Pull-quotes were placed after the sentence they interrupted; keep them there.
        if paragraph.starts_with("> ") {
            located.push(None);
            continue;
        }
        let key = paragraph_key(paragraph);
        let words: Vec<&str> = key.split_whitespace().collect();
        let opening = words.iter().take(4).copied().collect::<Vec<_>>().join(" ");
        let hit = (!key.is_empty())
            .then(|| {
                (0..lines.len()).find(|&i| {
                    let n = &norms[i];
                    !used[i]
                        && !n.is_empty()
                        && (*n == key
                            || (n.split_whitespace().count() >= 2 && key.starts_with(n.as_str()))
                            || (words.len() >= 3 && n.starts_with(&opening)))
                })
            })
            .flatten();
        located.push(hit.map(|i| {
            used[i] = true;
            (place_of(lines[i], mid), lines[i].top)
        }));
    }
    let found = located.iter().filter(|l| l.is_some()).count();
    if found * 10 < paragraphs.len() * 7 {
        return None;
    }

    // Unlocated paragraphs stay right after the paragraph before them.
    let mut keyed: Vec<(Place, f32, usize)> = Vec::with_capacity(paragraphs.len());
    let mut last = (Place::Full, 0.0f32);
    for (i, spot) in located.iter().enumerate() {
        let (place, top) = spot.unwrap_or((last.0, last.1 + 0.001));
        last = (place, top);
        keyed.push((place, top, i));
    }
    // Full-width items split the page into bands; within a band the left column
    // comes before the right one.
    let mut full_tops: Vec<f32> = keyed.iter().filter(|k| k.0 == Place::Full).map(|k| k.1).collect();
    full_tops.sort_by(f32::total_cmp);
    let band = |top: f32| full_tops.iter().filter(|t| **t <= top + 0.5).count();
    keyed.sort_by(|a, b| {
        let key = |k: &(Place, f32, usize)| {
            let column = match k.0 {
                Place::Full => 0,
                Place::Left => 1,
                Place::Right => 2,
            };
            (band(k.1), column, k.1, k.2)
        };
        let (ka, kb) = (key(a), key(b));
        ka.0.cmp(&kb.0).then(ka.1.cmp(&kb.1)).then(ka.2.total_cmp(&kb.2)).then(ka.3.cmp(&kb.3))
    });
    Some(keyed.iter().map(|k| paragraphs[k.2].as_str()).collect::<Vec<_>>().join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, left: f32, top: f32) -> Line {
        Line { page: 1, text: text.into(), size: 10.0, top, baseline: 792.0 - top - 10.0, left, right: left + 230.0, page_height: 792.0 }
    }

    #[test]
    fn two_column_page_is_read_left_column_first() {
        let mut lines = Vec::new();
        for k in 0..6 {
            lines.push(line(&format!("left column sentence number {k} continues here"), 50.0, 100.0 + 14.0 * k as f32));
            lines.push(line(&format!("right column sentence number {k} continues here"), 330.0, 100.0 + 14.0 * k as f32));
        }
        let refs: Vec<&Line> = lines.iter().collect();
        let mid = two_column_split(&refs, 612.0).expect("two columns");
        // pdf-inspector order: the right column's first paragraph came first.
        let markdown = "right column sentence number 0 continues here\n\nleft column sentence number 0 continues here";
        let fixed = reorder(markdown, &refs, mid).unwrap();
        assert!(fixed.starts_with("left column"));
    }

    #[test]
    fn single_column_page_is_left_alone() {
        let lines: Vec<Line> = (0..12).map(|k| Line { right: 560.0, ..line(&format!("a full width line of body text {k}"), 50.0, 100.0 + 14.0 * k as f32) }).collect();
        let refs: Vec<&Line> = lines.iter().collect();
        assert!(two_column_split(&refs, 612.0).is_none());
    }
}

#[cfg(test)]
mod layer_tests {
    use super::*;
    use crate::outline::Heading;

    #[test]
    fn slide_title_spelled_over_lines_becomes_one_heading() {
        let heading = Heading { level: 1, title: "The AI becomes a working assistant.".into(), page_start: 3, source: "pdf-outline".into() };
        let page = "The AI becomes a working\n\nassistant.\n\nIt can read and create files.".to_string();
        let out = insert_pages("", &[], &[(3, page)], &[heading]);
        assert_eq!(out, "# The AI becomes a working assistant.\n\nIt can read and create files.");
    }
}
