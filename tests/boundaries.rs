//! Boundary gate that `hexa analyze` cannot express (ADR-2610261255).
//!
//! hexa 26.9.12 lets a primary adapter import use cases; CLAUDE.md says primary
//! adapters import `ports/` only. This test reads the real source and holds the
//! stricter line. It was shown to fail on an injected `use crate::usecases::App;`.

use std::fs;
use std::path::Path;

const FORBIDDEN: &[&str] = &["crate::usecases", "crate::domain", "crate::adapters::secondary"];

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn primary_adapters_import_ports_only() {
    let mut files = Vec::new();
    rust_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src/adapters/primary"), &mut files);
    assert!(!files.is_empty(), "no primary adapter found; a gate over nothing passes vacuously");

    let mut hits = Vec::new();
    for f in &files {
        for (n, line) in fs::read_to_string(f).unwrap().lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for bad in FORBIDDEN {
                if code.contains(bad) {
                    hits.push(format!("{}:{}: {}", f.display(), n + 1, line.trim()));
                }
            }
        }
    }
    assert!(hits.is_empty(), "primary adapters must go through ports::Browser:\n{}", hits.join("\n"));
}
