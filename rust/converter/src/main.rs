//! Command-line converter (`npm run convert -- --input file.pdf`). It writes the
//! same files as the desktop app, which calls the same library in-process.

use petes_pdf_core::{convert, output::Mode, ConvertOptions};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "Usage: petes-pdf-to-md --input <file.pdf> [--out-dir output] [--out-subdir v0.9.0a] \
[--conversion-mode single|major|sections] [--include-section-metadata 0|1]";

fn main() -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut opts = ConvertOptions {
        out_dir: PathBuf::from("output"),
        out_subdir: None,
        mode: Mode::Sections,
        include_section_metadata: true,
    };

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        let result = match arg.as_str() {
            "--input" | "-i" => value(&arg).map(|v| input = Some(PathBuf::from(v))),
            "--out-dir" | "-o" => value(&arg).map(|v| opts.out_dir = PathBuf::from(v)),
            "--out-subdir" => value(&arg).map(|v| opts.out_subdir = Some(v)),
            "--include-section-metadata" => value(&arg).map(|v| opts.include_section_metadata = v != "0"),
            "--conversion-mode" => value(&arg).and_then(|v| Mode::parse(&v).map(|m| opts.mode = m).ok_or(format!("Unknown mode: {v}"))),
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other if input.is_none() && !other.starts_with('-') => {
                input = Some(PathBuf::from(other));
                Ok(())
            }
            other => Err(format!("Unexpected argument: {other}")),
        };
        if let Err(message) = result {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    }

    let Some(input) = input else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if !input.exists() {
        eprintln!("Input file not found: {}", input.display());
        return ExitCode::FAILURE;
    }

    match convert(&input, &opts) {
        Ok(result) => {
            for warning in &result.warnings {
                println!("PROGRESS: Warning: {warning}");
            }
            println!("Conversion complete");
            println!("Input: {}", input.display());
            println!("Output: {}", result.out_dir.display());
            println!("Headings: {}", result.headings);
            println!("Segments: {}", result.segments);
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("Conversion failed: {message}");
            ExitCode::FAILURE
        }
    }
}
