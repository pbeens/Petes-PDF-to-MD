//! Print the text pdf-inspector recovers from a page's invisible (OCR) text layer.
//! Usage: cargo run --release --example ocr_layer_probe -- <pdf> <page> [<page> ...]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let buffer = std::fs::read(&args[1]).unwrap();
    let doc = lopdf::Document::load_mem(&buffer).unwrap();
    for page in &args[2..] {
        let page: u32 = page.parse().unwrap();
        let id = doc.get_pages()[&page];
        let rect = doc.get_dictionary(id).unwrap().get(b"MediaBox").unwrap().as_array().unwrap().iter().map(|v| v.as_float().unwrap_or(0.0)).collect::<Vec<_>>();
        let regions = vec![(page - 1, vec![[0.0, 0.0, rect[2], rect[3]]])];
        let result = pdf_inspector::extract_text_in_regions_mem(&buffer, &regions).unwrap();
        let r = &result[0].regions[0];
        println!("=== page {page} needs_ocr={} reason={:?}\n{}", r.needs_ocr, r.ocr_reason, r.text);
    }
}
