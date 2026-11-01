//! Rebuild tables that pdf-inspector's Markdown read row by row as run-on text
//! ("Knowledge of content The student The student The student The student …").
//! The run-on paragraph is located on its page, and that area is re-extracted
//! with pdf-inspector's region table detection.

use crate::layout::Line;
use crate::text::{clean_line, normalize_match_text, strip_inline_markdown};
use pdf_inspector::types::ItemType;
use pdf_inspector::TextItem;
use std::collections::HashMap;

/// Everything needed to look a paragraph up on its page and re-extract it.
pub struct TableContext<'a> {
    pub buffer: &'a [u8],
    pub items: &'a [TextItem],
    pub lines: &'a [Line],
    pub page_widths: &'a HashMap<u32, f32>,
    pub page_heights: &'a HashMap<u32, f32>,
}

/// True when a run of 1-4 words repeats three or more times back to back, the
/// signature of table cells read across a row ("The student The student The student").
/// Single repeated words must be at least 4 letters and repeat four times.
pub fn looks_flattened(paragraph: &str) -> bool {
    if paragraph.starts_with('|') {
        return false;
    }
    let plain = strip_inline_markdown(paragraph).to_lowercase();
    let words: Vec<&str> = plain.split_whitespace().collect();
    for n in 1..=4 {
        let needed = if n == 1 { 4 } else { 3 };
        for start in 0..words.len() {
            if n == 1 && words[start].chars().filter(|c| c.is_alphabetic()).count() < 4 {
                continue;
            }
            let mut repeats = 1;
            while start + (repeats + 1) * n <= words.len()
                && words[start + repeats * n..start + (repeats + 1) * n] == words[start..start + n]
            {
                repeats += 1;
            }
            if repeats >= needed {
                return true;
            }
        }
    }
    false
}

/// Normalized opening words of a paragraph, used to find it on the page.
fn opening(paragraph: &str, words: usize) -> String {
    normalize_match_text(&strip_inline_markdown(paragraph)).split_whitespace().take(words).collect::<Vec<_>>().join(" ")
}

/// Index of the first line at or after `from`, on one of `pages`, that contains `needle`.
fn find_line(lines: &[Line], from: usize, pages: std::ops::RangeInclusive<u32>, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    // Table cells are separate lines, so the opening words may continue in the next
    // column: also accept a line of two or more words that the needle starts with.
    (from..lines.len()).find(|&i| {
        let text = normalize_match_text(&lines[i].text);
        pages.contains(&lines[i].page)
            && !text.is_empty()
            && (text.contains(needle) || (text.split_whitespace().count() >= 2 && needle.starts_with(&format!("{text} "))))
    })
}

/// Replace run-on table paragraphs in one section with real tables where the
/// page area can be re-extracted. Lookups stay within the section's `pages`
/// (chart wording repeats across charts) and `cursor` keeps them in document order.
pub fn repair(section: &str, pages: std::ops::RangeInclusive<u32>, ctx: &TableContext, cursor: &mut usize) -> String {
    let paragraphs: Vec<&str> = section.split("\n\n").collect();
    if !paragraphs.iter().any(|p| looks_flattened(p)) {
        return section.to_string();
    }
    let mut out: Vec<String> = Vec::with_capacity(paragraphs.len());
    let mut skip_until = 0;
    for (i, para) in paragraphs.iter().enumerate() {
        if i < skip_until {
            continue;
        }
        if !looks_flattened(para) {
            out.push(para.to_string());
            continue;
        }
        // Neighbouring run-on paragraphs usually belong to the same table.
        let mut last = i;
        while last + 1 < paragraphs.len() && looks_flattened(paragraphs[last + 1]) {
            last += 1;
        }
        match rebuild(&paragraphs[i..=last], &paragraphs[last + 1..], pages.clone(), ctx, cursor) {
            Some((table, absorbed)) => {
                out.push(table);
                skip_until = last + 1 + absorbed;
            }
            None => out.push(para.to_string()),
        }
    }
    out.join("\n\n")
}

/// Rebuild one run-on table. Returns the table and how many of the `following`
/// paragraphs it absorbed (pieces of the same table that didn't look run-on).
fn rebuild(run_on: &[&str], following_all: &[&str], pages: std::ops::RangeInclusive<u32>, ctx: &TableContext, cursor: &mut usize) -> Option<(String, usize)> {
    let following = following_all.first().copied();
    let Some(start) = find_line(ctx.lines, *cursor, pages.clone(), &opening(run_on[0], 4)) else {
        debug(&format!("could not find '{}' on a page", opening(run_on[0], 4)));
        return None;
    };
    let first = &ctx.lines[start];
    let page = first.page;
    // The table ends where the next paragraph starts, if that is on the same page;
    // otherwise at the footer band. Table columns are separate lines on the page, so
    // look for a following table by its first cell, and anything else by its opening
    // words, then just its first two.
    let below = |needle: String| {
        find_line(ctx.lines, start + 1, page..=page, &needle).filter(|&i| ctx.lines[i].top > first.top + 1.0)
    };
    let bottom = following
        .and_then(|next| {
            let lead = if next.starts_with('|') {
                normalize_match_text(next.trim_start_matches('|').split('|').next().unwrap_or(""))
            } else {
                opening(next, 4)
            };
            below(lead).or_else(|| below(opening(next, 2)))
        })
        .map_or(first.page_height * 0.92, |i| ctx.lines[i].top - 1.0);
    let width = ctx.page_widths.get(&page).copied().unwrap_or(612.0);
    let footer = first.page_height * 0.92;
    let top = (first.top - 3.0).max(0.0);
    // Try the table's own outline first, then the tightest area; the first table
    // that keeps every word wins.
    let mut regions = vec![[0.0, top, width, bottom], [0.0, top, width, footer], [0.0, first.page_height * 0.08, width, footer]];
    regions.dedup();
    let wanted: std::collections::HashSet<String> = run_on.iter().flat_map(|p| word_set(p)).collect();
    // First rebuild the grid from the repeated phrase that gave the table away,
    // then fall back to pdf-inspector's region table detection.
    // The grid follows the table's own shape, so it ignores where the next paragraph
    // starts (that paragraph is often another piece of the same table).
    let grid = grid_table(ctx, page, first.page_height - first.top, 0.0);
    let candidates = grid.into_iter().chain(regions.into_iter().filter_map(|r| extract_region(ctx.buffer, page, r)));
    for table in candidates {
        let table_words = word_set(&table);
        let missing: Vec<&String> = wanted.iter().filter(|w| !table_words.contains(*w)).collect();
        // Following paragraphs made only of the table's words are pieces of it.
        let absorbed: Vec<&str> = following_all
            .iter()
            .take_while(|p| !p.starts_with('#') && { let w = word_set(p); !w.is_empty() && w.is_subset(&table_words) })
            .copied()
            .collect();
        let expected: std::collections::HashSet<String> = wanted.iter().cloned().chain(absorbed.iter().flat_map(|p| word_set(p))).collect();
        // Words the text didn't have mean the area also caught another table,
        // which would then appear twice.
        let extra = table_words.iter().filter(|w| !expected.contains(*w)).count();
        debug(&format!("p{page}: candidate -> {} missing, {extra} extra, absorbs {}", missing.len(), absorbed.len()));
        if missing.is_empty() && extra <= 12.max(expected.len() * 3 / 10) {
            *cursor = start;
            return Some((table, absorbed.len()));
        }
    }
    None
}

/// Rebuild a run-on table from the positioned text on its page.
///
/// `row_top` is the top of the table's first row and `limit` the lowest point it
/// may reach (both measured up from the bottom of the page). The first row holds
/// the phrase that repeats across the columns ("The student"); its pieces give the
/// left edge of every column, and each later appearance of the phrase across the
/// columns starts a new row. Text pieces never cross cell borders, so each goes to
/// the column its left edge falls in. The table ends at a large gap, at text left
/// of the first column, or at a line that runs across two or more column edges.
fn grid_table(ctx: &TableContext, page: u32, row_top: f32, limit: f32) -> Option<String> {
    let mut pieces: Vec<&TextItem> = ctx
        .items
        .iter()
        .filter(|i| i.page == page && matches!(i.item_type, ItemType::Text) && !i.text.trim().is_empty())
        .collect();
    pieces.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
    let mut baselines: Vec<Vec<&TextItem>> = Vec::new();
    for item in pieces {
        match baselines.last_mut() {
            Some(row) if (row[0].y - item.y).abs() <= 2.0 => row.push(item),
            _ => baselines.push(vec![item]),
        }
    }
    // The first row: the highest baseline at or below the run-on text's top.
    let Some(start) = baselines.iter().position(|b| b[0].y <= row_top + 1.0 && b[0].y + b[0].height >= row_top - 14.0) else {
        debug(&format!("grid p{page}: no baseline near top {row_top:.1}"));
        return None;
    };
    let norm = |t: &str| normalize_match_text(t);
    let first = &baselines[start];
    let mut counts: HashMap<String, usize> = HashMap::new();
    first.iter().for_each(|i| *counts.entry(norm(&i.text)).or_default() += 1);
    let (phrase, repeats) = counts.into_iter().filter(|(t, _)| !t.is_empty()).max_by_key(|(_, n)| *n)?;
    if repeats < 3 {
        debug(&format!("grid p{page}: first row '{}' has no repeated phrase", first.iter().map(|i| i.text.as_str()).collect::<Vec<_>>().join("|")));
        return None;
    }
    let mut columns: Vec<f32> = vec![first[0].x];
    columns.extend(first.iter().filter(|i| norm(&i.text) == phrase).map(|i| i.x));
    columns.dedup_by(|a, b| (*a - *b).abs() < 3.0);
    let size = first[0].font_size.max(1.0);
    let column_of = |x: f32| columns.iter().rposition(|c| x >= c - 4.0);

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut previous_y = first[0].y;
    for baseline in &baselines[start..] {
        let y = baseline[0].y;
        if y < limit || previous_y - y > 2.5 * size || baseline.iter().any(|i| i.x < columns[0] - 6.0) {
            debug(&format!("grid p{page}: stop at y={y:.1} (limit {limit:.1}, gap {:.1}) after {} rows", previous_y - y, rows.len()));
            break;
        }
        // A piece reaching across two column edges is a heading or paragraph, not a
        // cell (cell text may overhang the next column's edge a little).
        let crosses = baseline.iter().any(|i| {
            column_of(i.x).and_then(|k| columns.get(k + 2)).is_some_and(|edge| i.x + i.width > edge + 4.0)
        });
        if crosses {
            debug(&format!("grid p{page}: stop at crossing line y={y:.1} after {} rows", rows.len()));
            break;
        }
        let starts_row = baseline.iter().filter(|i| norm(&i.text) == phrase).count() >= 3;
        if starts_row || rows.is_empty() {
            rows.push(vec![String::new(); columns.len()]);
        }
        let row = rows.last_mut().unwrap();
        for item in baseline {
            if let Some(k) = column_of(item.x) {
                if !row[k].is_empty() {
                    row[k].push(' ');
                }
                row[k].push_str(item.text.trim());
            }
        }
        previous_y = y;
    }
    if rows.len() < 2 {
        return None;
    }
    let cells: Vec<Vec<String>> = rows.into_iter().map(|r| r.into_iter().map(|c| clean_line(&c).replace('|', "\\|")).collect()).collect();
    let header: Vec<String> = if columns.len() == 5 {
        ["Categories", "Level 1", "Level 2", "Level 3", "Level 4"].map(String::from).to_vec()
    } else {
        (1..=columns.len()).map(|k| format!("Column {k}")).collect()
    };
    let mut lines = vec![format!("|{}|", header.join("|")), format!("|{}|", vec!["---"; columns.len()].join("|"))];
    lines.extend(cells.iter().map(|r| format!("|{}|", r.join("|"))));
    Some(lines.join("\n"))
}

/// Table Markdown for one page area: a pipe table with a separator row and at
/// least two body rows, or nothing.
fn extract_region(buffer: &[u8], page: u32, region: [f32; 4]) -> Option<String> {
    let result = pdf_inspector::extract_tables_in_regions_mem(buffer, &[(page - 1, vec![region])]).ok()?;
    let table = result.first()?.regions.first()?.text.trim().to_string();
    let rows = table.lines().filter(|l| l.starts_with('|')).count();
    (table.lines().any(|l| l.starts_with("|---")) && rows >= 4).then_some(table)
}

fn word_set(text: &str) -> std::collections::HashSet<String> {
    normalize_match_text(&strip_inline_markdown(text)).split_whitespace().filter(|w| w.len() >= 3).map(str::to_string).collect()
}

/// Diagnostics for table repair, shown when PDF_TO_MD_DEBUG_TABLES is set.
fn debug(message: &str) {
    if std::env::var_os("PDF_TO_MD_DEBUG_TABLES").is_some() {
        eprintln!("tables: {message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spots_flattened_rows() {
        assert!(looks_flattened("**Knowledge of content** The student The student The student The student demonstrates"));
        assert!(looks_flattened("uses uses uses uses planning"));
        assert!(!looks_flattened("The student demonstrates limited knowledge of content."));
        assert!(!looks_flattened("very very very good"));
        assert!(!looks_flattened("|a|a|a|"));
    }
}
