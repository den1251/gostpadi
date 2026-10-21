//! Корпус лабораторных 2026: каждый `corpus/labfix-*.c` проходит `--check`
//! и рисуется без паники. Ловит регрессии раскладки и фронтенда на
//! реалистичном студенческом коде (ЛБ1–ЛБ4).

use std::path::Path;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_gostpadi"))
}

#[test]
fn labfix_corpus_checks_and_renders() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("labfix-"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    assert!(files.len() >= 12, "корпус мал: {}", files.len());

    let out = std::env::temp_dir().join(format!("gostpadi-labfix-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    for f in &files {
        let p = f.to_str().unwrap();
        let chk = bin().args(["--check", p]).output().unwrap();
        assert!(
            chk.status.success(),
            "{p} --check: {}",
            String::from_utf8_lossy(&chk.stderr)
        );
        let r = bin()
            .args([p, "-o", out.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            r.status.success(),
            "{p} render: {}",
            String::from_utf8_lossy(&r.stderr)
        );
        let pages: Vec<String> = String::from_utf8_lossy(&r.stdout)
            .lines()
            .map(str::to_string)
            .collect();
        assert!(!pages.is_empty(), "{p}: ни одного листа");
        for page in pages {
            let svg = std::fs::read_to_string(&page).unwrap();
            assert!(
                svg.contains("<svg") && svg.len() > 400,
                "{page}: пустой лист"
            );
        }
    }
    std::fs::remove_dir_all(&out).ok();
}
