//! Диагностика: размеры листов корпуса тем же путём, что CLI-пачка
//! (split + uniform sizes). Запуск:
//! cargo test --quiet dbg_pages -- --ignored --nocapture

use crate::layout::{layout, normalize, split_scheme, uniform_sizes};
use crate::pipeline::{parse_batch, Options, Scheme};
use crate::style::Style;

fn opts() -> Options {
    Options {
        labels: "en".into(),
        font: None,
        lw: None,
        no_split: false,
    }
}

fn parse(paths: &[&str]) -> Vec<Scheme> {
    let inputs: Vec<(String, String, bool)> = paths
        .iter()
        .map(|p| {
            (
                p.to_string(),
                std::fs::read_to_string(p).unwrap(),
                p.ends_with(".c"),
            )
        })
        .collect();
    parse_batch(&inputs, &opts()).unwrap()
}

fn pages_at(paths: &[&str], scale: f64) {
    let st = Style {
        split_scale: scale,
        ..opts().style()
    };
    let schemes = parse(paths);
    let normed: Vec<_> = schemes.iter().map(|s| normalize(&s.nodes, &st)).collect();
    let sizes = uniform_sizes(&normed);
    for s in schemes {
        let parts = split_scheme(s.nodes, &sizes, &st);
        println!("== {}: {} листов", s.path, parts.len());
        for (i, p) in parts.iter().enumerate() {
            let l = layout(p, &sizes, &st);
            let (_, _, w, h) = l.bounds;
            let fw = (st.a4_w - 2.0 * st.page_pad) / w;
            let fh = (st.a4_h - 2.0 * st.page_pad) / h;
            println!(
                "  лист {i}: {w:.0}x{h:.0} fit {:.3}/{:.3} узлов {}",
                fw,
                fh,
                p.len()
            );
            if let Err(e) = crate::layout::crossings_ok(&l.shapes, &l.edges) {
                println!("     ПЕРЕСЕЧЕНИЕ: {e}");
            }
            if fw.min(fh) < 0.35 {
                let mut lo = f64::MAX;
                let mut hi = f64::MIN;
                for sh in &l.shapes {
                    lo = lo.min(sh.cx - sh.w / 2.0);
                    hi = hi.max(sh.cx + sh.w / 2.0);
                }
                println!("     фигуры x {lo:.0}..{hi:.0}");
                for sh in &l.shapes {
                    println!(
                        "     {} ({:.0},{:.0}) {:.0}x{:.0} {:?}",
                        sh.kind, sh.cx, sh.cy, sh.w, sh.h, sh.lines
                    );
                }
            }
        }
    }
}

#[test]
#[ignore]
fn dbg_pages_labs() {
    pages_at(
        &[
            "corpus/oap-lab1.c",
            "corpus/oap-lab2.c",
            "corpus/oap-lab3.c",
            "corpus/oap-lab4.c",
        ],
        1.0,
    );
}

#[test]
#[ignore]
fn dbg_pages_at_cut() {
    pages_at(
        &[
            "corpus/oap-lab1.c",
            "corpus/oap-lab2.c",
            "corpus/oap-lab3.c",
            "corpus/oap-lab4.c",
        ],
        0.22,
    );
}
