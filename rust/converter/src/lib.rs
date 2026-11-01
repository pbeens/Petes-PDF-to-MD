//! Pete's PDF to MD converter core: turn a PDF into Markdown split by heading.
//!
//! Text, fonts and accessibility tags come from `pdf-inspector`; bookmarks and
//! page sizes from `lopdf`. No Python is needed.

pub mod body;
pub mod layout;
pub mod outline;
pub mod output;
pub mod reading;
pub mod tables;
pub mod text;

use outline::Heading;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ConvertOptions {
    pub out_dir: PathBuf,
    /// Extra folder inside `<out_dir>/<pdf name>/` (debug builds use the version).
    pub out_subdir: Option<String>,
    pub mode: output::Mode,
    pub include_section_metadata: bool,
}

pub struct ConvertResult {
    pub out_dir: PathBuf,
    pub headings: usize,
    pub segments: usize,
    pub warnings: Vec<String>,
}

/// Page width and height in points, by page number.
fn page_sizes(doc: &lopdf::Document) -> HashMap<u32, (f32, f32)> {
    // MediaBox can be inherited from a parent Pages node.
    fn media_box(doc: &lopdf::Document, mut id: lopdf::ObjectId) -> Option<(f32, f32)> {
        for _ in 0..32 {
            let dict = doc.get_dictionary(id).ok()?;
            if let Ok(lopdf::Object::Array(rect)) = dict.get(b"MediaBox").map(|o| doc.dereference(o).map(|(_, o)| o.clone())).ok()? {
                let nums: Vec<f32> = rect.iter().filter_map(|v| v.as_float().ok().or_else(|| v.as_i64().ok().map(|i| i as f32))).collect();
                if nums.len() == 4 {
                    return Some(((nums[2] - nums[0]).abs(), (nums[3] - nums[1]).abs()));
                }
            }
            id = dict.get(b"Parent").ok()?.as_reference().ok()?;
        }
        None
    }
    doc.get_pages().into_iter().filter_map(|(num, id)| media_box(doc, id).map(|size| (num, size))).collect()
}

/// Text of each page's invisible text layer, one line per paragraph, without
/// pdf-inspector's "[Image: …]" markers. Pages whose layer is empty are left out.
fn hidden_layer_texts(buffer: &[u8], pages: &[u32], sizes: &HashMap<u32, (f32, f32)>) -> Vec<(u32, String)> {
    if pages.is_empty() {
        return Vec::new();
    }
    let regions: Vec<(u32, Vec<[f32; 4]>)> = pages
        .iter()
        .map(|p| {
            let (w, h) = sizes.get(p).copied().unwrap_or((612.0, 792.0));
            (p - 1, vec![[0.0, 0.0, w, h]])
        })
        .collect();
    let Ok(results) = pdf_inspector::extract_text_in_regions_mem(buffer, &regions) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|page| {
            let text: Vec<String> = page.regions.iter().flat_map(|r| r.text.lines()).map(text::clean_line).filter(|l| !l.is_empty() && !l.starts_with("[Image:")).collect();
            (!text.is_empty()).then(|| (page.page + 1, text.join("\n\n")))
        })
        .collect()
}

/// Convert one PDF, printing progress lines (`PROGRESS: …`) to stdout, the way the
/// desktop app's launcher reads them.
pub fn convert(input: &Path, opts: &ConvertOptions) -> Result<ConvertResult, String> {
    convert_with_progress(input, opts, &|message: &str| println!("PROGRESS: {message}"))
}

/// Convert one PDF, reporting each step to `progress` (for example to show it in a window).
pub fn convert_with_progress(input: &Path, opts: &ConvertOptions, progress: &dyn Fn(&str)) -> Result<ConvertResult, String> {
    progress("Opening PDF document");
    let buffer = std::fs::read(input).map_err(|e| format!("Could not read {}: {e}", input.display()))?;
    let mut warnings = Vec::new();

    progress("Extracting text");
    let processed = pdf_inspector::process_pdf_mem(&buffer).map_err(|e| format!("Could not read PDF: {e}"))?;
    let whole_markdown = processed.markdown.unwrap_or_default();
    let page_count = processed.page_count;
    let doc = lopdf::Document::load_mem(&buffer).ok();
    let sizes = doc.as_ref().map(page_sizes).unwrap_or_default();

    // Pages that are pictures with their text in an invisible layer behind them
    // (exported slides, scanned pages that were already OCR'd). pdf-inspector leaves
    // that layer out of its Markdown; read it with region extraction instead.
    let layer_pages: Vec<u32> = processed
        .ocr_reasons_by_page
        .iter()
        .filter(|r| r.reasons.iter().any(|x| x == pdf_inspector::OCR_REASON_INVISIBLE_TEXT_LAYER))
        .map(|r| r.page)
        .collect();
    let layer_texts = hidden_layer_texts(&buffer, &layer_pages, &sizes);
    if !layer_texts.is_empty() {
        progress(&format!(
            "Note: read the hidden text layer on {} pages (text recognized from images; it may contain recognition errors)",
            layer_texts.len()
        ));
    }
    let recovered: std::collections::HashSet<u32> = layer_texts.iter().map(|(p, _)| *p).collect();
    let image_pages: Vec<u32> = processed.pages_needing_ocr.iter().copied().filter(|p| !recovered.contains(p)).collect();
    let image_pages = &image_pages;
    if !image_pages.is_empty() {
        let mut list: Vec<String> = image_pages.iter().take(10).map(u32::to_string).collect();
        if image_pages.len() > 10 {
            list.push(format!("and {} more", image_pages.len() - 10));
        }
        let list = list.join(", ");
        // A cover or a full-page figure is normal; only a mostly scanned document
        // deserves a warning dialog.
        if image_pages.len() * 4 >= page_count as usize {
            warnings.push(format!(
                "{} of {page_count} pages are images with no text (pages {list}); the document looks scanned. \
                 Run it through text recognition (OCR) first to convert those pages.",
                image_pages.len()
            ));
        } else {
            progress(&format!("Note: pages {list} contain only images (for example a cover or a full-page figure)"));
        }
    }
    let items = pdf_inspector::extract_text_with_positions_mem(&buffer).unwrap_or_default();
    let elements = pdf_inspector::extract_structure_elements_mem(&buffer, None).unwrap_or_default();

    progress("Detecting headings");
    let bookmarks = doc.as_ref().map(outline::from_bookmarks).unwrap_or_default();
    let tags = outline::from_tags(&items, &elements);
    let (mut headings, note) = outline::choose(bookmarks, tags);
    if !note.is_empty() {
        progress(note);
    }
    // Bookmarks and accessibility tags are both real document structure.
    let has_structure = !headings.is_empty();
    let heights: HashMap<u32, f32> = sizes.iter().map(|(p, (_, h))| (*p, *h)).collect();
    let widths: HashMap<u32, f32> = sizes.iter().map(|(p, (w, _))| (*p, *w)).collect();
    let lines = layout::page_lines(&items, &heights);
    let fallback_title = input.file_stem().map(|s| s.to_string_lossy().replace('-', " ")).unwrap_or_else(|| "Document".into());
    if headings.is_empty() {
        progress("No embedded TOC found; using text heuristics");
        headings = outline::from_text(&lines);
        // Guessed headings are too noisy for one long file; use a single title instead.
        if headings.is_empty() || opts.mode == output::Mode::Single {
            let title = outline::document_title(&lines, &fallback_title);
            headings = vec![Heading { level: 1, title, page_start: 1, source: "single-fallback".into() }];
        }
    }
    let headings = outline::normalize(headings);

    let margin = body::margin_noise(&lines, page_count);
    // Page by page, with two-column pages put in reading order. PDF_TO_MD_NO_REORDER=1
    // keeps pdf-inspector's own order (for comparisons).
    let whole_markdown = reading::insert_pages(&whole_markdown, &lines, &layer_texts, &headings);
    let markdown = if std::env::var_os("PDF_TO_MD_NO_REORDER").is_some() {
        whole_markdown
    } else {
        let (text, reordered) = reading::document_markdown(&lines, &widths, &whole_markdown);
        if !reordered.is_empty() {
            progress(&format!("Fixed reading order on {} pages", reordered.len()));
        }
        text
    };
    let furniture = body::furniture_lines(&markdown, &margin);

    let doc_name = input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "document".into());
    let mut out_dir = opts.out_dir.join(&doc_name);
    if let Some(sub) = opts.out_subdir.as_deref().filter(|s| !s.is_empty()) {
        out_dir = out_dir.join(Path::new(sub).file_name().unwrap_or_default());
    }
    progress("Writing outline and section markdown files");
    let write_opts = output::Options {
        mode: opts.mode,
        include_section_metadata: opts.include_section_metadata,
        emit_generated_toc: has_structure,
        doc_name: &doc_name,
        progress,
    };
    let table_ctx = tables::TableContext { buffer: &buffer, items: &items, lines: &lines, page_widths: &widths, page_heights: &heights };
    let (outline, segments) = output::write(&out_dir, &headings, &markdown, &furniture, page_count, &write_opts, &table_ctx)
        .map_err(|e| e.to_string())?;
    progress("Finalizing output indexes");
    Ok(ConvertResult { out_dir, headings: outline.len(), segments: segments.len(), warnings })
}
