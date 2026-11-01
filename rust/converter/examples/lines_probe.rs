//! Print the visual lines of one page with their positions.
//! Usage: cargo run --release --example lines_probe -- <pdf> <page>
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let buffer = std::fs::read(&args[1]).unwrap();
    let page: u32 = args[2].parse().unwrap();
    let items = pdf_inspector::extract_text_with_positions_mem(&buffer).unwrap();
    let heights = std::collections::HashMap::from([(page, 792.0)]);
    for l in petes_pdf_core::layout::page_lines(&items, &heights).iter().filter(|l| l.page == page).take(45) {
        println!("top={:6.1} left={:6.1} right={:6.1} size={:4.1} {}", l.top, l.left, l.right, l.size, l.text.chars().take(60).collect::<String>());
    }
}
