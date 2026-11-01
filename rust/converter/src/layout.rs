//! Group pdf-inspector's text pieces into visual lines, the unit PyMuPDF's
//! `get_text("dict")` lines gave the Python converter.

use crate::text::clean_line;
use pdf_inspector::types::ItemType;
use pdf_inspector::TextItem;
use std::collections::{BTreeMap, HashMap};

/// One visual line of text on a page.
#[derive(Clone, Debug)]
pub struct Line {
    pub page: u32,
    pub text: String,
    /// Largest font size on the line.
    pub size: f32,
    /// Distance from the top of the page to the top of the line.
    pub top: f32,
    /// Distance from the bottom of the page to the lowest baseline.
    pub baseline: f32,
    /// Left and right edges, in points from the left of the page.
    pub left: f32,
    pub right: f32,
    pub page_height: f32,
}

/// Lines for every page, top to bottom. Pieces join into one line when their
/// baselines are within 2 points and no wide gap (1.5 times the font size)
/// separates them, so columns sharing a baseline (including table cells) stay apart.
pub fn page_lines(items: &[TextItem], page_heights: &HashMap<u32, f32>) -> Vec<Line> {
    let mut by_page: BTreeMap<u32, Vec<&TextItem>> = BTreeMap::new();
    for item in items.iter().filter(|i| matches!(i.item_type, ItemType::Text) && !i.text.trim().is_empty()) {
        by_page.entry(item.page).or_default().push(item);
    }
    let mut out = Vec::new();
    for (page, mut page_items) in by_page {
        let height = page_heights.get(&page).copied().unwrap_or(792.0);
        // y is the baseline measured up from the bottom of the page.
        page_items.sort_by(|a, b| b.y.total_cmp(&a.y).then(a.x.total_cmp(&b.x)));
        let mut rows: Vec<Vec<&TextItem>> = Vec::new();
        for item in page_items {
            match rows.last_mut() {
                Some(row) if (row[0].y - item.y).abs() <= 2.0 => row.push(item),
                _ => rows.push(vec![item]),
            }
        }
        for mut row in rows {
            row.sort_by(|a, b| a.x.total_cmp(&b.x));
            let mut segment: Vec<&TextItem> = Vec::new();
            for item in row {
                if let Some(prev) = segment.last() {
                    let gap = item.x - (prev.x + prev.width);
                    let (small, large) = (prev.font_size.min(item.font_size).max(1.0), prev.font_size.max(item.font_size).max(1.0));
                    // A wide gap, or a clear change of type size across a visible gap
                    // (a pull-quote beside a column), starts a new line.
                    if gap > 1.5 * large || (large >= small * 1.15 && gap > 0.5 * small) {
                        out.extend(make_line(page, height, &segment));
                        segment.clear();
                    }
                }
                segment.push(item);
            }
            out.extend(make_line(page, height, &segment));
        }
    }
    out
}

fn make_line(page: u32, page_height: f32, pieces: &[&TextItem]) -> Option<Line> {
    let mut text = String::new();
    let mut last: Option<&TextItem> = None;
    for item in pieces {
        if let Some(prev) = last {
            // Pieces that touch belong to one word; anything wider is a space.
            if item.x - (prev.x + prev.width) > 0.15 * item.font_size.max(1.0) {
                text.push(' ');
            }
        }
        text.push_str(&item.text);
        last = Some(item);
    }
    let text = clean_line(&text);
    if text.is_empty() {
        return None;
    }
    let size = pieces.iter().map(|i| i.font_size).fold(0.0, f32::max);
    let line_top = pieces.iter().map(|i| i.y + i.height).fold(f32::MIN, f32::max);
    let baseline = pieces.iter().map(|i| i.y).fold(f32::MAX, f32::min);
    let left = pieces.first().map_or(0.0, |i| i.x);
    let right = pieces.iter().map(|i| i.x + i.width).fold(left, f32::max);
    Some(Line { page, text, size, top: page_height - line_top, baseline, left, right, page_height })
}
