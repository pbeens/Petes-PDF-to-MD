//! Writing results: section files for the chosen mode, `outline.json`,
//! `outline.md` and `segments.json`, in the same layout and format as the
//! retired Python converter wrote them. Ported from `write_outputs`, `build_section_markdown`, `build_toc_markdown_table` and
//! `prepare_output_dirs`.

use crate::body::{self, EMPTY_BODY_PLACEHOLDER};
use crate::outline::Heading;
use crate::tables::{self, TableContext};
use crate::text::{clean_line, normalize_match_text, slugify};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Single,
    Major,
    Sections,
}

impl Mode {
    pub fn parse(value: &str) -> Option<Mode> {
        match value {
            "single" => Some(Mode::Single),
            "major" => Some(Mode::Major),
            "sections" => Some(Mode::Sections),
            _ => None,
        }
    }
}

/// One `outline.json` entry.
#[derive(Clone, Debug, Serialize)]
pub struct OutlineEntry {
    pub id: String,
    pub level: u32,
    pub title: String,
    pub page_start: u32,
    pub source: String,
    pub y0: Option<f64>,
    pub line_count: usize,
    pub char_count: usize,
    pub section_file: Option<String>,
}

/// One `segments.json` entry: a written file and the pages it covers.
#[derive(Clone, Debug, Serialize)]
pub struct Segment {
    pub id: String,
    pub title: String,
    pub level: u32,
    pub page_start: u32,
    pub page_end: u32,
    pub file: String,
    pub char_count: usize,
}

pub struct Options<'a> {
    pub mode: Mode,
    pub include_section_metadata: bool,
    /// Write the generated table of contents at the top of single-file output
    /// (only when the outline came from bookmarks or tags).
    pub emit_generated_toc: bool,
    /// File name for single-file output, without `.md` (the PDF's name).
    pub doc_name: &'a str,
    /// Receives a message for each step.
    pub progress: &'a dyn Fn(&str),
}

struct Section {
    start: u32,
    end: u32,
    body: String,
}

fn front_matter() -> OutlineEntry {
    OutlineEntry {
        id: "h0".into(),
        level: 1,
        title: "Front Matter".into(),
        page_start: 1,
        source: "synthetic-front-matter".into(),
        y0: Some(0.0),
        line_count: 0,
        char_count: 0,
        section_file: None,
    }
}

fn section_markdown(entry: &OutlineEntry, section: &Section, include_metadata: bool, combined: bool) -> String {
    // In a combined file the next heading follows directly, so the placeholder is just noise.
    let body = if combined && section.body == EMPTY_BODY_PLACEHOLDER { "" } else { section.body.as_str() };
    let hashes = "#".repeat(entry.level.clamp(1, 6) as usize);
    let metadata = if include_metadata {
        format!("- Level: {}\n- Pages: {}-{}\n- Source: {}\n\n", entry.level, section.start, section.end, entry.source)
    } else {
        String::new()
    };
    format!("{hashes} {}\n\n{metadata}{body}\n", entry.title)
}

fn toc_table(outline: &[OutlineEntry], sections: &[Section]) -> String {
    let mut lines = vec!["## Table of Contents".to_string(), String::new(), "| Section | Page |".into(), "|---|---:|".into()];
    let mut seen = HashSet::new();
    for (entry, section) in outline.iter().zip(sections) {
        let title = clean_line(&entry.title);
        if title.is_empty() || !seen.insert((normalize_match_text(&title), section.start)) {
            continue;
        }
        lines.push(format!("| {} | {} |", title.replace('|', "\\|"), section.start));
    }
    lines.join("\n")
}

/// Create the mode's folder and remove the previous run's Markdown files from it.
/// Only `.md` files directly in that folder are touched, never subfolders.
fn prepare_dirs(out_dir: &Path, mode: Mode) -> io::Result<(std::path::PathBuf, &'static str)> {
    let (target, prefix) = match mode {
        Mode::Sections => (out_dir.join("Sections"), "Sections/"),
        Mode::Major => (out_dir.join("By Major Heading"), "By Major Heading/"),
        Mode::Single => (out_dir.to_path_buf(), ""),
    };
    fs::create_dir_all(&target)?;
    for entry in fs::read_dir(&target)? {
        let path = entry?.path();
        let is_md = path.extension().is_some_and(|e| e == "md");
        let is_outline = path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("outline.md"));
        if path.is_file() && is_md && !(mode == Mode::Single && is_outline) {
            fs::remove_file(&path).map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!("Output file is locked by another process: {}. Close any open section files or preview windows and retry.", path.display()),
                )
            })?;
        }
    }
    Ok((target, prefix))
}

/// Split `markdown` by `headings`, clean each section, and write all output files.
pub fn write(
    out_dir: &Path,
    headings: &[Heading],
    markdown: &str,
    furniture: &HashSet<String>,
    page_count: u32,
    opts: &Options,
    table_ctx: &TableContext,
) -> io::Result<(Vec<OutlineEntry>, Vec<Segment>)> {
    let progress = opts.progress;
    let (target_dir, prefix) = prepare_dirs(out_dir, opts.mode)?;

    let mut outline: Vec<OutlineEntry> = headings
        .iter()
        .enumerate()
        .map(|(i, h)| OutlineEntry {
            id: format!("h{}", i + 1),
            level: h.level,
            title: h.title.clone(),
            page_start: h.page_start,
            source: h.source.clone(),
            y0: None,
            line_count: 0,
            char_count: 0,
            section_file: None,
        })
        .collect();
    // Keep title pages and other text that comes before the first heading.
    if outline.first().is_some_and(|first| first.page_start > 1) {
        outline.insert(0, front_matter());
    }

    progress("Splitting text by heading");
    let titles: Vec<(String, u32, bool)> =
        outline.iter().map(|e| (e.title.clone(), e.page_start, e.source == "synthetic-front-matter")).collect();
    let (mut bodies, lead_in) = body::split_by_outline(markdown, &titles, page_count);
    let has_front_matter = outline.first().is_some_and(|e| e.source == "synthetic-front-matter");
    if !lead_in.is_empty() && !has_front_matter {
        outline.insert(0, front_matter());
        bodies.insert(0, lead_in);
    } else if has_front_matter {
        bodies[0] = lead_in;
    }

    let total = outline.len();
    let mut sections: Vec<Section> = Vec::with_capacity(total);
    let mut last_footnote = 0;
    let mut table_cursor = 0;
    for (i, entry) in outline.iter().enumerate() {
        progress(&format!("Extracting section text {}/{}: {}", i + 1, total, entry.title.chars().take(80).collect::<String>()));
        let start = entry.page_start.max(1);
        // Bodies come from the text, not page ranges, so pages are only a label here.
        let end = match outline.get(i + 1) {
            Some(next) if next.page_start > start => next.page_start - 1,
            Some(_) => start,
            None => page_count.max(start),
        };
        let mut text = body::clean_section(&bodies[i], furniture, &mut last_footnote);
        if text.is_empty() {
            text = EMPTY_BODY_PLACEHOLDER.into();
        }
        // A section's text can run onto the page where the next heading starts.
        let text = tables::repair(&text, start..=end + 1, table_ctx, &mut table_cursor);
        let text = body::deduplicate_paragraphs(&body::format_dot_leaders(&text));
        sections.push(Section { start, end, body: text });
    }

    // Section numbers ("2.1.0.0") give each file its place in the outline.
    let max_level = outline.iter().map(|e| e.level).max().unwrap_or(1);
    let depth = max_level.clamp(3, 6) as usize;
    let mut counters = vec![0u32; depth];
    let mut codes = Vec::with_capacity(total);
    for (entry, section) in outline.iter_mut().zip(&sections) {
        let level = (entry.level as usize).clamp(1, depth);
        counters[level..].iter_mut().for_each(|c| *c = 0);
        counters[level - 1] += 1;
        codes.push(counters.iter().map(u32::to_string).collect::<Vec<_>>().join("."));
        entry.line_count = section.body.lines().filter(|l| !l.trim().is_empty()).count();
        entry.char_count = section.body.chars().count();
    }

    let mut segments = Vec::new();
    match opts.mode {
        Mode::Sections => {
            for (i, (entry, section)) in outline.iter_mut().zip(&sections).enumerate() {
                progress(&format!("Writing section markdown {}/{}: {}", i + 1, total, entry.title.chars().take(80).collect::<String>()));
                let file_name = format!("{}-{}.md", codes[i], slugify(&entry.title));
                fs::write(target_dir.join(&file_name), section_markdown(entry, section, opts.include_section_metadata, false))?;
                entry.section_file = Some(format!("{prefix}{file_name}"));
                segments.push(Segment {
                    id: format!("s{}", segments.len() + 1),
                    title: entry.title.clone(),
                    level: entry.level,
                    page_start: section.start,
                    page_end: section.end,
                    file: format!("{prefix}{file_name}"),
                    char_count: section.body.chars().count(),
                });
            }
        }
        Mode::Major => {
            // The highest level that occurs at least twice defines the major headings.
            let mut counts = std::collections::BTreeMap::new();
            outline.iter().for_each(|e| *counts.entry(e.level).or_insert(0) += 1);
            let min_level = counts.keys().next().copied().unwrap_or(1);
            let major_level = counts.iter().find(|(_, n)| **n >= 2).map_or(min_level, |(l, _)| *l);
            let mut majors: Vec<usize> = (0..outline.len()).filter(|&i| outline[i].level == major_level).collect();
            if majors.is_empty() {
                majors.push(0);
            }
            // Sections before the first major heading are filed under it.
            let mut groups: Vec<(usize, Vec<usize>)> = majors.iter().map(|&m| (m, Vec::new())).collect();
            let mut active = 0;
            for i in 0..outline.len() {
                if let Some(pos) = majors.iter().position(|&m| m == i) {
                    active = pos;
                }
                groups[active].1.push(i);
            }
            for (major, members) in groups.into_iter().filter(|(_, m)| !m.is_empty()) {
                let file_name = format!("{}-{}.md", codes[major], slugify(&outline[major].title));
                let chunk: Vec<String> = members
                    .iter()
                    .map(|&i| section_markdown(&outline[i], &sections[i], opts.include_section_metadata, true).trim().to_string())
                    .collect();
                fs::write(target_dir.join(&file_name), chunk.join("\n\n") + "\n")?;
                let entry = &mut outline[major];
                entry.section_file = Some(format!("{prefix}{file_name}"));
                segments.push(Segment {
                    id: format!("s{}", segments.len() + 1),
                    title: entry.title.clone(),
                    level: entry.level,
                    page_start: sections[members[0]].start,
                    page_end: sections[*members.last().unwrap()].end,
                    file: format!("{prefix}{file_name}"),
                    char_count: members.iter().map(|&i| sections[i].body.chars().count()).sum(),
                });
            }
        }
        Mode::Single => {
            let file_name = format!("{}.md", opts.doc_name);
            let mut chunk = Vec::new();
            if opts.emit_generated_toc && !outline.is_empty() {
                chunk.push(toc_table(&outline, &sections));
            }
            for (entry, section) in outline.iter().zip(&sections) {
                chunk.push(section_markdown(entry, section, opts.include_section_metadata, true).trim().to_string());
            }
            fs::write(target_dir.join(&file_name), chunk.join("\n\n") + "\n")?;
            if let Some(first) = outline.first_mut() {
                first.section_file = Some(format!("{prefix}{file_name}"));
            }
            segments.push(Segment {
                id: "s1".into(),
                title: "Document".into(),
                level: 1,
                page_start: sections.first().map_or(1, |s| s.start),
                page_end: sections.last().map_or(page_count, |s| s.end),
                file: format!("{prefix}{file_name}"),
                char_count: sections.iter().map(|s| s.body.chars().count()).sum(),
            });
        }
    }

    fs::write(out_dir.join("outline.json"), serde_json::to_string_pretty(&outline)?)?;
    fs::write(out_dir.join("segments.json"), serde_json::to_string_pretty(&segments)?)?;
    let mut md = vec!["# Outline".to_string(), String::new()];
    for entry in &outline {
        let indent = "  ".repeat(entry.level.saturating_sub(1) as usize);
        let title = match &entry.section_file {
            Some(file) => format!("[{}]({file})", entry.title),
            None => entry.title.clone(),
        };
        md.push(format!("{indent}- L{} p{} {title} (lines: {}, chars: {})", entry.level, entry.page_start, entry.line_count, entry.char_count));
    }
    md.push(String::new());
    fs::write(out_dir.join("outline.md"), md.join("\n"))?;
    Ok((outline, segments))
}
