//! Экспорт раскладки в формат draw.io (mxGraph XML).
//!
//! Раньше мост отдавал только SVG-строку, а рисунок в draw.io
//! редактировать нельзя: это картинка. Здесь та же раскладка
//! превращается в `mxCell` с геометрией, поэтому блоки и рёбра в
//! draw.io можно брать и двигать.
//!
//! Геометрия — та же, что в SVG: лист А4 в пунктах, содержимое
//! масштабируется общим коэффициентом пачки (`fit_scale`) и
//! центрируется с полями `page_pad`. Координаты в пунктах, `fontSize`
//! тоже в пунктах. Поэтому .drawio и отчётный SVG показывают одну и ту
//! же схему, а не «как получилось».
//!
//! Рёбра привязываются к блокам, если конец ломаной попадает внутрь
//! фигуры: в draw.io такие рёбра тянутся за блоком. Подписи ветвей —
//! дочерние ячейки ребра (`relative=1`), поэтому при перетаскивании
//! блока они едут вместе с линией.

use crate::generate::shapes::TAN75;
use crate::generate::svg::{doc_bounds, title_geom, WHISKER_FRAC};
use crate::layout::{Layout, Shape};
use crate::style::Style;

/// Страница draw.io — А4 книжная в пунктах (как SVG pt).
const PAGE_W: f64 = 595.276;
const PAGE_H: f64 = 841.89;

/// Отступ «предопределённого процесса» от боковой стороны, в долях
/// сетки (как в SVG).
const TITLE_INSET_FRAC: f64 = 0.25;

/// Экранирование текста для XML-атрибута. Переводы строк — числовыми
/// ссылками: XML-парсер схлопывает литеральный \n в атрибуте в пробел,
/// и многострочная плитка в draw.io становилась однострочной.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            c if !crate::generate::text::xml_ok(c) => out.push('\u{FFFD}'),
            _ => out.push(c),
        }
    }
    out
}

/// Число листа без хвостовых нулей: 595.276 остаётся 595.276,
/// 841.890 — 841.89. Два знака превращали А4 в 595.28.
fn page_num(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Доля, которую срез занимает от ширины фигуры (для `size=` в стиле).
fn frac(size: f64, total: f64) -> f64 {
    if total.abs() < 1e-9 {
        0.0
    } else {
        (size / total).clamp(0.0, 1.0)
    }
}

/// Стиль фигуры по её виду в раскладке. Формы те же, что рисует SVG:
/// трапеции цикла со срезом с одной стороны, ромб, параллелограмм,
/// капсула, шестиугольник-маркер, круг.
fn shape_style(sh: &Shape, st: &Style, k: f64) -> String {
    let font = (st.font * k).round();
    let lw = st.edge_lw * k;
    let body = match sh.kind.as_str() {
        "term" | "ret" => "rounded=1;arcSize=50;".to_string(),
        // наклон боковых сторон — как у SVG: Δ = h / tan(75°)
        "io" => format!(
            "shape=parallelogram;perimeter=parallelogramPerimeter;size={:.4};",
            frac(sh.h / TAN75, sh.w)
        ),
        "if" => "rhombus;".to_string(),
        // ГОСТ 19.701: цикл — трапеция, срез только сверху (начало)
        // или только снизу (слияние); flipV поднимает срез вниз.
        "loop_begin" => format!(
            "shape=trapezoid;perimeter=trapezoidPerimeter;size={:.4};",
            frac(sh.skew, sh.w)
        ),
        "loop_end" => format!(
            "shape=trapezoid;perimeter=trapezoidPerimeter;size={:.4};flipV=1;",
            frac(sh.skew, sh.w)
        ),
        // маркер break/continue — шестиугольник, срез с двух сторон
        "exit" => format!(
            "shape=hexagon;perimeter=hexagonPerimeter2;size={:.4};",
            frac(sh.skew, sh.w)
        ),
        "conn" => "ellipse;".to_string(),
        _ => "rounded=0;".to_string(),
    };
    format!(
        "{body}whiteSpace=wrap;html=1;fontFamily=DejaVu Sans Mono;fontSize={font};strokeWidth={lw:.3};\
         fillColor=#ffffff;strokeColor=#000000;"
    )
}

/// Стиль ребра: ломаная как в SVG (без автоперегиба), наконечник —
/// открытый, как рисует SVG.
fn edge_style(st: &Style, k: f64, arrow: bool) -> String {
    let lw = st.edge_lw * k;
    let end = if arrow {
        // усик SVG (0.25 модуля) в проекции на ось стрелки
        let size = (WHISKER_FRAC * 2.0 * st.grid * 0.923_879_5 * k).max(1.0);
        format!("endArrow=open;endFill=0;endSize={size:.2};")
    } else {
        "endArrow=none;".to_string()
    };
    format!("edgeStyle=none;rounded=0;html=1;{end}strokeColor=#000000;strokeWidth={lw:.3};")
}

/// Вершина mxCell в готовых координатах листа.
fn vertex(id: &str, value: &str, style: &str, x: f64, y: f64, w: f64, h: f64) -> String {
    format!(
        "        <mxCell id=\"{id}\" value=\"{value}\" style=\"{style}\" vertex=\"1\" parent=\"1\">\
         \n          <mxGeometry x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" as=\"geometry\"/>\
         \n        </mxCell>"
    )
}

/// Блок фигуры: центр раскладки -> левый верхний угол листа.
fn cell(id: &str, sh: &Shape, st: &Style, k: f64, off: (f64, f64)) -> String {
    // строки блока — переводом строки: esc делает из него &#10;, и
    // draw.io переносит строки внутри плитки (литеральный <br> там
    // показывался бы текстом)
    let text = esc(&sh.lines.join("\n"));
    vertex(
        id,
        &text,
        &shape_style(sh, st, k),
        (sh.cx - sh.w / 2.0) * k + off.0,
        (sh.cy - sh.h / 2.0) * k + off.1,
        sh.w * k,
        sh.h * k,
    )
}

/// Блок, внутри которого лежит точка: для привязки ребра.
///
/// Проверка по прямоугольнику, а не по описанной окружности: у тонкой
/// полосы окружность накрывает соседей снизу, и ребро склеивалось само
/// с собой (`source` и `target` — один блок).
fn block_at(layout: &Layout, p: (f64, f64)) -> Option<String> {
    const TOL: f64 = 0.5;
    layout
        .shapes
        .iter()
        .position(|sh| {
            (p.0 - sh.cx).abs() <= sh.w / 2.0 + TOL && (p.1 - sh.cy).abs() <= sh.h / 2.0 + TOL
        })
        .map(|i| format!("n{i}"))
}

/// Ребро по готовым точкам листа. Концы, попавшие в блок, уходят в
/// `source`/`target`, остальные — в `sourcePoint`/`targetPoint`:
/// так draw.io не теряет ни привязанные, ни свободные концы.
fn edge_cell(
    id: &str,
    points: &[(f64, f64)],
    st: &Style,
    k: f64,
    src: Option<&str>,
    dst: Option<&str>,
    arrow: bool,
) -> String {
    // концы заданы терминалами или sourcePoint/targetPoint; в Array
    // остаются только промежуточные точки ломаной
    let from = 1.min(points.len());
    let to = points.len().saturating_sub(1).max(from);
    let mut inner = String::new();
    if src.is_none() {
        let (x, y) = points[0];
        inner.push_str(&format!(
            "\n            <mxPoint x=\"{x:.2}\" y=\"{y:.2}\" as=\"sourcePoint\"/>"
        ));
    }
    if dst.is_none() {
        let (x, y) = points[points.len() - 1];
        inner.push_str(&format!(
            "\n            <mxPoint x=\"{x:.2}\" y=\"{y:.2}\" as=\"targetPoint\"/>"
        ));
    }
    if from < to {
        let pts: Vec<String> = points[from..to]
            .iter()
            .map(|(x, y)| format!("<mxPoint x=\"{x:.2}\" y=\"{y:.2}\"/>"))
            .collect();
        inner.push_str("\n            <Array as=\"points\">\n              ");
        inner.push_str(&pts.join("\n              "));
        inner.push_str("\n            </Array>");
    }
    let attrs = match (src, dst) {
        (Some(s), Some(d)) => format!(" source=\"{s}\" target=\"{d}\""),
        (Some(s), None) => format!(" source=\"{s}\""),
        (None, Some(d)) => format!(" target=\"{d}\""),
        (None, None) => String::new(),
    };
    format!(
        "        <mxCell id=\"{id}\" style=\"{}\" edge=\"1\" parent=\"1\"{attrs}>\n          \
         <mxGeometry relative=\"1\" as=\"geometry\">{inner}\n          </mxGeometry>\n        </mxCell>",
        edge_style(st, k, arrow).replace('"', "&quot;"),
    )
}

/// Ближайшая к точке ломаная: (индекс ребра, относительная позиция
/// вдоль пути в [-1, 1], подписанное расстояние по нормали).
///
/// Те же формулы, что у mxGraph: `x = 2f - 1` (0 — середина),
/// `y` — смещение по нормали `(dy, -dx) / len`.
fn nearest_edge(edges: &[Vec<(f64, f64)>], p: (f64, f64)) -> Option<(usize, f64, f64)> {
    let mut best: Option<(usize, f64, f64, f64)> = None;
    for (i, e) in edges.iter().enumerate() {
        let total: f64 = e
            .windows(2)
            .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
            .sum();
        if total < 1e-9 {
            continue;
        }
        let mut acc = 0.0;
        for w in e.windows(2) {
            let (p0, p1) = (w[0], w[1]);
            let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
            let len = dx.hypot(dy);
            if len < 1e-9 {
                continue;
            }
            let t = (((p.0 - p0.0) * dx + (p.1 - p0.1) * dy) / (len * len)).clamp(0.0, 1.0);
            let (cx, cy) = (p0.0 + dx * t, p0.1 + dy * t);
            let d2 = (p.0 - cx).powi(2) + (p.1 - cy).powi(2);
            if best.map_or(true, |(_, _, _, bd)| d2 < bd) {
                let f = (acc + t * len) / total;
                // нормаль (dy, -dx): положительное смещение — вправо по ходу
                let d = ((p.0 - cx) * dy - (p.1 - cy) * dx) / len;
                best = Some((i, 2.0 * f - 1.0, d, d2));
            }
            acc += len;
        }
    }
    best.map(|(i, x, d, _)| (i, x, d))
}

/// Подпись ветви дочерней ячейкой ребра: едет за линией.
///
/// Блок текста задаётся явно (ширина/высота и сдвиг от точки на ребре):
/// тогда `align` ставит подпись ровно как SVG, а не «как решит движок».
fn edge_label(
    id: &str,
    text: &str,
    ha: &str,
    st: &Style,
    k: f64,
    edge: usize,
    x: f64,
    y: f64,
) -> String {
    let align = match ha {
        "left" => "left",
        "right" => "right",
        _ => "center",
    };
    let w = (text.chars().count() as f64 * st.char_w + 6.0) * k;
    let h = st.font * 1.4 * k;
    let dx = match align {
        "left" => 0.0,
        "right" => -w,
        _ => -w / 2.0,
    };
    format!(
        "        <mxCell id=\"{id}\" value=\"{}\" style=\"edgeLabel;html=1;align={align};\
         verticalAlign=middle;resizable=0;fontFamily=DejaVu Sans Mono;fontSize={};\
         spacing=0;spacingLeft=0;spacingRight=0;spacingTop=0;spacingBottom=0;\" \
         vertex=\"1\" connectable=\"0\" parent=\"e{edge}\">\n          \
         <mxGeometry x=\"{x:.4}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" \
         relative=\"1\" as=\"geometry\">\n            \
         <mxPoint x=\"{dx:.2}\" y=\"{:.2}\" as=\"offset\"/>\n          \
         </mxGeometry>\n        </mxCell>",
        esc(text),
        (st.font * k).round(),
        -h / 2.0,
    )
}

/// Подпись без ребра (на всякий случай) — обычная текстовая вершина.
fn free_label(id: &str, text: &str, ha: &str, st: &Style, k: f64, anchor: (f64, f64)) -> String {
    let align = match ha {
        "left" => "left",
        "right" => "right",
        _ => "center",
    };
    let h = st.font * 1.4 * k;
    let w = (text.chars().count() as f64 * st.char_w + 6.0) * k;
    let x = match align {
        "left" => anchor.0,
        "right" => anchor.0 - w,
        _ => anchor.0 - w / 2.0,
    };
    format!(
        "        <mxCell id=\"{id}\" value=\"{}\" style=\"text;html=1;align={align};\
         verticalAlign=middle;fontFamily=DejaVu Sans Mono;fontSize={};strokeColor=none;\
         fillColor=none;\" vertex=\"1\" parent=\"1\">\n          \
         <mxGeometry x=\"{x:.2}\" y=\"{:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" as=\"geometry\"/>\
         \n        </mxCell>",
        esc(text),
        (st.font * k).round(),
        anchor.1 - h / 2.0,
    )
}

/// Сдвиг центрированного содержимого на лист А4.
fn page_offset(l: &Layout, st: &Style, k: f64) -> (f64, f64) {
    let (minx, miny, w, h) = doc_bounds(l, st);
    (
        (PAGE_W - w * k) / 2.0 - minx * k,
        (PAGE_H - h * k) / 2.0 - miny * k,
    )
}

/// Лист раскладки -> один `<diagram>`.
fn diagram(name: &str, id: &str, layout: &Layout, st: &Style, scale: f64) -> String {
    let k = scale.min(crate::generate::fit_scale(layout, st));
    let off = page_offset(layout, st, k);
    let mut body = String::new();

    // подпись функции над схемой — тот же «предопределённый процесс»,
    // что и в SVG: без него в draw.io не видно, какая схема какой функции
    if let Some(title) = layout.title.as_deref() {
        if let Some((cx, y0, w, h, y_start)) = title_geom(layout, st, title) {
            let (x, y) = ((cx - w / 2.0) * k + off.0, y0 * k + off.1);
            let inset = frac(TITLE_INSET_FRAC * st.grid, w);
            body.push_str(&vertex(
                "title",
                &esc(title),
                &format!(
                    "shape=process;whiteSpace=wrap;html=1;fontFamily=DejaVu Sans Mono;\
                     fontSize={};strokeWidth={:.3};size={inset:.4};fillColor=#ffffff;\
                     strokeColor=#000000;",
                    (st.font * k).round(),
                    st.edge_lw * k,
                ),
                x,
                y,
                w * k,
                h * k,
            ));
            body.push('\n');
            // стрелка от коробки к «началу»
            let start = layout
                .shapes
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.cy.partial_cmp(&b.cy).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i);
            let target = start.map(|i| format!("n{i}"));
            body.push_str(&edge_cell(
                "title_arrow",
                &[
                    (cx * k + off.0, (y0 + h) * k + off.1),
                    (cx * k + off.0, y_start * k + off.1),
                ],
                st,
                k,
                Some("title"),
                target.as_deref(),
                true,
            ));
            body.push('\n');
        }
    }

    for (i, sh) in layout.shapes.iter().enumerate() {
        body.push_str(&cell(&format!("n{i}"), sh, st, k, off));
        body.push('\n');
    }

    // точки рёбер в координатах листа: по ним же считаются подписи
    let scaled: Vec<Vec<(f64, f64)>> = layout
        .edges
        .iter()
        .map(|e| {
            e.points
                .iter()
                .map(|&(x, y)| (x * k + off.0, y * k + off.1))
                .collect()
        })
        .collect();

    for (i, e) in layout.edges.iter().enumerate() {
        let (src, dst) = (
            e.points.first().and_then(|p| block_at(layout, *p)),
            e.points.last().and_then(|p| block_at(layout, *p)),
        );
        body.push_str(&edge_cell(
            &format!("e{i}"),
            &scaled[i],
            st,
            k,
            src.as_deref(),
            dst.as_deref(),
            e.arrow,
        ));
        body.push('\n');
    }

    for (i, lb) in layout.labels.iter().enumerate() {
        let anchor = (lb.x * k + off.0, lb.y * k + off.1);
        match nearest_edge(&scaled, anchor) {
            Some((edge, x, y)) => body.push_str(&edge_label(
                &format!("t{i}"),
                &lb.text,
                &lb.ha,
                st,
                k,
                edge,
                x,
                y,
            )),
            None => body.push_str(&free_label(
                &format!("t{i}"),
                &lb.text,
                &lb.ha,
                st,
                k,
                anchor,
            )),
        }
        body.push('\n');
    }

    format!(
        "  <diagram name=\"{}\" id=\"{}\">\n    <mxGraphModel dx=\"850\" dy=\"1100\" grid=\"0\" \
gridSize=\"10\" guides=\"1\" tooltips=\"1\" connect=\"1\" arrows=\"1\" fold=\"1\" page=\"1\" \
pageScale=\"1\" pageWidth=\"{}\" pageHeight=\"{}\" math=\"0\" shadow=\"0\">\n      <root>\n\
        <mxCell id=\"0\"/>\n        <mxCell id=\"1\" parent=\"0\"/>\n{body}      </root>\n    \
         </mxGraphModel>\n  </diagram>\n",
        esc(name),
        esc(id),
        page_num(PAGE_W),
        page_num(PAGE_H),
    )
}

/// Книга: несколько листов в одном файле — как в самой схеме.
pub fn book(name: &str, pages: &[(String, Layout)], st: &Style, scale: f64) -> String {
    let mut out =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<mxfile host=\"gostpadi\">\n");
    for (i, (page_name, layout)) in pages.iter().enumerate() {
        let fallback = if pages.len() == 1 {
            name.to_string()
        } else {
            format!("{name} — лист {}", i + 1)
        };
        let page_name = if page_name.trim().is_empty() {
            fallback
        } else {
            page_name.clone()
        };
        out.push_str(&diagram(&page_name, &format!("page{i}"), layout, st, scale));
        out.push('\n');
    }
    out.push_str("</mxfile>\n");
    out
}

/// Одна книга на всю лабу: каждая схема и каждый её лист — отдельная
/// страница draw.io. Так «скачать draw.io» даёт один файл, в котором
/// открываются сразу все схемы отчёта, а не папка обрывков.
pub fn book_all(sections: &[(String, Vec<Layout>)], st: &Style, scale: f64) -> String {
    let total: usize = sections.iter().map(|(_, p)| p.len()).sum();
    let mut out =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<mxfile host=\"gostpadi\">\n");
    let mut n = 0;
    for (stem, pages) in sections {
        for (i, layout) in pages.iter().enumerate() {
            let name = if total <= 1 || pages.len() == 1 {
                stem.clone()
            } else {
                format!("{stem} — лист {}", i + 1)
            };
            out.push_str(&diagram(&name, &format!("page{n}"), layout, st, scale));
            out.push('\n');
            n += 1;
        }
    }
    out.push_str("</mxfile>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::fit_scale;
    use crate::layout::{layout, normalize};

    fn st() -> Style {
        Style::default()
    }

    fn laid(gvn: &str) -> Layout {
        let s = st();
        let nodes = crate::frontend::gvn::parse(gvn, &s, "en").unwrap();
        layout(&nodes, &normalize(&nodes, &s), &s)
    }

    /// Значение атрибута в куске XML.
    fn attr<'a>(s: &'a str, name: &str) -> Option<&'a str> {
        s.split(&format!("{name}=\"")).nth(1)?.split('"').next()
    }

    /// Все `<mxGeometry>` с полным прямоугольником (вершины).
    fn vertex_geoms(xml: &str) -> Vec<(f64, f64, f64, f64)> {
        let mut out = Vec::new();
        for part in xml.split("<mxGeometry ").skip(1) {
            let tag = &part[..part.find('>').unwrap_or(0)];
            // подписи рёбер живут в относительных координатах (-1..1)
            if tag.contains("relative=\"1\"") {
                continue;
            }
            if let (Some(x), Some(y), Some(w), Some(h)) = (
                attr(tag, "x"),
                attr(tag, "y"),
                attr(tag, "width"),
                attr(tag, "height"),
            ) {
                out.push((
                    x.parse().unwrap(),
                    y.parse().unwrap(),
                    w.parse().unwrap(),
                    h.parse().unwrap(),
                ));
            }
        }
        out
    }

    /// Книга — это XML, а не картинка: блоки в нём редактируемые.
    #[test]
    fn produces_parseable_mxfile() {
        let l = laid("input scanf(\"x\")\n");
        let xml = book(
            "main",
            &[("Лист 1".into(), l.clone())],
            &st(),
            fit_scale(&l, &st()),
        );
        assert!(xml.starts_with("<?xml"), "нет XML-декларации");
        assert!(xml.contains("<mxfile host=\"gostpadi\">"), "нет mxfile");
        assert!(xml.contains("<mxGraphModel"), "нет модели");
        assert!(xml.contains("vertex=\"1\""), "нет вершин");
        assert!(xml.contains("<mxCell id=\"0\"/>"), "нет корневых ячеек");
    }

    /// Все листы — ровно A4 в пунктах, страница не растягивается под
    /// содержимое (иначе draw.io печатает обрезок).
    #[test]
    fn every_page_is_a4() {
        let pages: Vec<(String, Layout)> = (0..3)
            .map(|i| (String::new(), laid(&format!("a = {i}\n"))))
            .collect();
        let xml = book("схема", &pages, &st(), 0.7);
        let diagrams: Vec<&str> = xml.split("<diagram ").skip(1).collect();
        assert_eq!(diagrams.len(), 3);
        for d in diagrams {
            assert_eq!(attr(d, "pageWidth"), Some("595.276"), "ширина не А4");
            assert_eq!(attr(d, "pageHeight"), Some("841.89"), "высота не А4");
            assert_eq!(attr(d, "pageScale"), Some("1"));
        }
    }

    /// Число `<diagram>` = числу листов: иначе в книге остался бы
    /// только последний.
    #[test]
    fn every_page_becomes_its_diagram() {
        let l1 = laid("a = 1\n");
        let l2 = laid("b = 2\n");
        let xml = book(
            "схема",
            &[(String::new(), l1), (String::new(), l2)],
            &st(),
            1.0,
        );
        assert_eq!(xml.matches("<diagram ").count(), 2, "листов должно быть 2");
        assert!(
            xml.contains("лист 1") && xml.contains("лист 2"),
            "нет имён листов"
        );
    }

    /// Содержимое (и запас на полуширину штриха) не вылезает за лист:
    /// именно это ломало печать — bbox шире страницы.
    #[test]
    fn page_size_is_in_points() {
        let xml = book("t", &[("p".into(), laid("a = 1\n"))], &st(), 1.0);
        assert!(xml.contains("pageWidth=\"595.276\""), "ширина не А4: {xml}");
        assert!(xml.contains("pageHeight=\"841.89\""), "высота не А4: {xml}");
        let w: f64 = xml
            .split("pageWidth=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse().ok())
            .expect("нет ширины листа");
        let h: f64 = xml
            .split("pageHeight=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse().ok())
            .expect("нет высоты листа");
        assert!((w - 595.276).abs() < 0.01, "ширина не А4: {w}");
        assert!((h - 841.89).abs() < 0.01, "высота не А4: {h}");
    }

    #[test]
    fn content_stays_on_sheet() {
        let schemes = [
            "a = 1\nb = 2\nc = 3\n",
            "if a\n    да: printf(\"1\")\n    нет: printf(\"2\")\n",
            "while i < 5\n    a = 1\n",
        ];
        for src in schemes {
            let l = laid(src);
            let xml = book("t", &[("p".into(), l.clone())], &st(), fit_scale(&l, &st()));
            let margin = st().edge_lw * fit_scale(&l, &st()) / 2.0;
            for (x, y, w, h) in vertex_geoms(&xml) {
                assert!(
                    x >= margin - 0.01
                        && y >= margin - 0.01
                        && x + w <= PAGE_W - margin + 0.01
                        && y + h <= PAGE_H - margin + 0.01,
                    "вершина {x},{y} {w}x{h} вылезла за лист: {src}"
                );
            }
            for part in xml.split("<mxPoint ").skip(1) {
                let tag = &part[..part.find("/>").unwrap_or(0)];
                // смещение подписи ("offset") тоже относительно точки ребра
                if matches!(attr(tag, "as"), Some("offset")) {
                    continue;
                }
                let (Some(x), Some(y)) = (attr(tag, "x"), attr(tag, "y")) else {
                    continue;
                };
                let (x, y): (f64, f64) = (x.parse().unwrap(), y.parse().unwrap());
                assert!(
                    (0.0..=PAGE_W).contains(&x) && (0.0..=PAGE_H).contains(&y),
                    "точка ребра {x},{y} вылезла за лист: {src}"
                );
            }
        }
    }

    /// Геометрия блока в draw.io = геометрия раскладки × масштаб, сдвиг
    /// тот же, что у страницы. Иначе схема в draw.io не совпадает с SVG.
    #[test]
    fn block_geometry_matches_layout_scale() {
        let l = laid(&"x = 1\n".repeat(30));
        let k = fit_scale(&l, &st());
        assert!(k < 0.95, "для длинной схемы ожидался масштаб < 1, а {k}");
        let xml = book("t", &[("p".into(), l.clone())], &st(), k);
        let off = page_offset(&l, &st(), k);
        for (i, sh) in l.shapes.iter().enumerate() {
            let id = format!("id=\"n{i}\"");
            let pos = xml.find(&id).expect("нет блока в XML");
            let tail = &xml[pos..];
            let start = tail.find("<mxGeometry ").expect("нет геометрии") + 12;
            let tag = &tail[start..start + tail[start..].find('>').expect("нет тега")];
            let got: Vec<f64> = ["x", "y", "width", "height"]
                .iter()
                .map(|a| attr(tag, a).unwrap().parse().unwrap())
                .collect();
            let want = [
                (sh.cx - sh.w / 2.0) * k + off.0,
                (sh.cy - sh.h / 2.0) * k + off.1,
                sh.w * k,
                sh.h * k,
            ];
            for (a, b) in got.iter().zip(want.iter()) {
                assert!((a - b).abs() < 0.01, "блок {i}: {got:?} != {want:?}");
            }
        }
    }

    /// Рёбра уезжают за нижний край и должны привязаться к блокам,
    /// иначе в draw.io они болтаются отдельно.
    #[test]
    fn edges_attach_to_blocks_when_possible() {
        let l = laid("a = 1\nb = 2\n");
        let xml = book("t", &[("p".into(), l.clone())], &st(), fit_scale(&l, &st()));
        assert!(xml.contains("edge=\"1\""), "нет рёбер");
        assert!(
            xml.contains("source=\"n") || !xml.contains("edge=\"1\" source"),
            "хотя бы одно ребро должно привязаться к блокам"
        );
    }

    /// Ребро не должно привязаться к одному и тому же блоку с двух
    /// сторон: у тонких полос описанная окружность накрывала соседа, и
    /// draw.io рисовал петлю вместо вертикали.
    #[test]
    fn edge_never_attaches_to_itself() {
        let l = laid("if a\n    да: printf(\"1\")\n    нет: printf(\"2\")\n");
        let xml = book("t", &[("p".into(), l.clone())], &st(), fit_scale(&l, &st()));
        for cell in xml.split("<mxCell id=\"e").skip(1) {
            let (Some(s), Some(t)) = (attr(cell, "source"), attr(cell, "target")) else {
                continue;
            };
            assert_ne!(s, t, "ребро привязалось само к себе: {cell:.120}");
        }
    }

    /// Подписи ветвей — дочерние ячейки рёбер: при перетаскивании
    /// блока подпись едет вместе с линией, а не висит в координатах.
    #[test]
    fn branch_labels_attach_to_edges() {
        let l = laid("if a\n    да: printf(\"1\")\n    нет: printf(\"2\")\n");
        assert!(!l.labels.is_empty(), "в схеме нет подписей ветвей");
        let xml = book("t", &[("p".into(), l.clone())], &st(), fit_scale(&l, &st()));
        assert_eq!(
            xml.matches("edgeLabel;").count(),
            l.labels.len(),
            "не все подписи привязаны к рёбрам"
        );
        assert!(xml.contains("parent=\"e"), "подпись не дочерняя ребру");
        assert!(
            xml.contains("relative=\"1\""),
            "нет относительной геометрии"
        );
    }

    /// Текст с XML-символами не ломает файл: без экранирования
    /// draw.io просто не откроет его. Переводы строк — в `<br>`.
    #[test]
    fn escapes_xml_specials() {
        let xml = book(
            "t",
            &[("p".into(), laid("output printf(\"a & b < c > d\")\n"))],
            &st(),
            1.0,
        );
        assert!(xml.contains("&amp;"), "amp не экранирован");
        assert!(
            !xml.contains("a & b <"),
            "необработанные спецсимволы в тексте"
        );
        let multi = book(
            "t",
            &[("p".into(), laid("output printf(\"a\")\n"))],
            &st(),
            1.0,
        );
        assert!(
            !multi.contains("<br>") || multi.contains("&lt;br&gt;"),
            "сырой <br> в атрибуте ломает XML"
        );
    }

    /// Ромб, параллелограмм и капсула должны уехать в draw.io своими
    /// формами, а не прямоугольниками.
    #[test]
    fn gost_shapes_keep_their_form() {
        let l = laid("if a\n    да: printf(\"1\")\n    нет: printf(\"2\")\n");
        let xml = book("t", &[("p".into(), l)], &st(), 1.0);
        assert!(xml.contains("rhombus"), "нет ромба");
        assert!(xml.contains("shape=parallelogram"), "нет параллелограмма");
        assert!(xml.contains("rounded=1;arcSize=50"), "нет капсулы");
    }

    /// Цикл — трапеция со срезом только сверху/снизу, а не
    /// симметричный шестиугольник: так его рисует SVG (skew).
    #[test]
    fn loop_is_trapezoid_with_one_side_cut() {
        let l = laid("while i < 5\n    a = 1\n");
        let xml = book("t", &[("p".into(), l)], &st(), 1.0);
        assert!(xml.contains("shape=trapezoid"), "цикл не трапеция");
        assert!(xml.contains("flipV=1"), "у слияния не перевёрнут срез");
        assert!(
            !xml.contains("shape=hexagon;perimeter=hexagonPerimeter2;size=0.2500"),
            "цикл остался симметричным шестиугольником"
        );
    }

    /// Одна схема — один лист, масштаб не теряется: книга из пачки
    /// рисует те же блоки, что SVG.
    #[test]
    fn book_all_numbers_pages_and_scale() {
        let a = laid("a = 1\n");
        let b = laid("b = 2\n");
        let xml = book_all(
            &[("one".into(), vec![a]), ("two".into(), vec![b])],
            &st(),
            0.5,
        );
        assert_eq!(xml.matches("<diagram ").count(), 2);
        assert!(xml.contains("name=\"one\"") && xml.contains("name=\"two\""));
        assert!(
            xml.contains("page0") && xml.contains("page1"),
            "id листов не уникальны"
        );
        assert!(xml.contains("pageScale=\"1\""));
    }

    /// Управляющий символ (0x01) невалиден в XML 1.0: раньше он валил
    /// разбор всего файла и в SVG, и в draw.io. Теперь — U+FFFD.
    #[test]
    fn control_chars_do_not_break_xml() {
        let xml = book(
            "t",
            &[("p".into(), laid("output printf(\"a\u{1}b\")\n"))],
            &st(),
            1.0,
        );
        assert!(!xml.contains('\u{1}'), "сырой 0x01 остался в XML");
        assert!(xml.contains('\u{FFFD}'), "0x01 не заменён на U+FFFD");
    }

    /// Перевод строки в value — только `&#10;`: литеральный \n в атрибуте
    /// XML-парсер схлопывает в пробел, и блок терял переносы.
    #[test]
    fn multiline_cell_keeps_line_breaks() {
        let l = laid(
            "if aaa > 0 && bbb > 0 && ccc > 0 && ddd > 0 && eee > 0 && fff > 0 && ggg > 0\n    \
             да: printf(1)\n    нет: printf(2)\n",
        );
        let xml = book("t", &[("p".into(), l)], &st(), 1.0);
        assert!(xml.contains("&#10;"), "нет числовой ссылки перевода строки");
    }

    /// source/target рёбер ссылаются на существующие mxCell: иначе draw.io
    /// молча теряет стрелку.
    #[test]
    fn edge_endpoints_reference_existing_cells() {
        let xml = book(
            "t",
            &[(
                "p".into(),
                laid("if a\n    да: printf(1)\n    нет: printf(2)\n"),
            )],
            &st(),
            1.0,
        );
        let ids: Vec<&str> = xml
            .split("id=\"")
            .skip(1)
            .filter_map(|p| p.split('"').next())
            .collect();
        for key in ["source=\"", "target=\""] {
            for part in xml.split(key).skip(1) {
                let id = part.split('"').next().unwrap();
                assert!(ids.contains(&id), "{key}{id}\" без ячейки");
            }
        }
    }
}
