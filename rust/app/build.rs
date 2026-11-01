//! Build script: Tauri's code generation, plus the display version taken from the
//! repository's package.json. A zero patch number is dropped (1.0.0 -> 1.0), and
//! debug builds add the build revision letter (1.0b), the same rule as
//! scripts/bump-revision.js. PDF_TO_MD_DEBUG_BUILD=1 at build time makes a debug
//! build ("(debug version)" in the title, per-version output folders).

fn main() {
    let package = std::fs::read_to_string("../../package.json").unwrap_or_default();
    let pkg: serde_json::Value = serde_json::from_str(&package).unwrap_or_default();
    let version = pkg["version"].as_str().unwrap_or(env!("CARGO_PKG_VERSION")).to_string();
    let debug = std::env::var("PDF_TO_MD_DEBUG_BUILD").map(|v| v == "1").unwrap_or(false);
    // A letter left over from an older version means no build of this version yet.
    let revision = if debug && pkg["buildRevisionOf"].as_str() == Some(version.as_str()) {
        pkg["buildRevision"].as_str().unwrap_or("").to_string()
    } else {
        String::new()
    };
    let short = version.strip_suffix(".0").filter(|v| v.matches('.').count() == 1).unwrap_or(&version);
    println!("cargo:rustc-env=PDF_TO_MD_VERSION={short}{revision}");
    println!("cargo:rustc-env=PDF_TO_MD_DEBUG_BUILD={}", if debug { "1" } else { "" });
    println!("cargo:rerun-if-changed=../../package.json");
    println!("cargo:rerun-if-env-changed=PDF_TO_MD_DEBUG_BUILD");
    tauri_build::build();
}
