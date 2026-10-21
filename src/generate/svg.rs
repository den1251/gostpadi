//! Слой-обёртка: шапка SVG, единый масштаб через <g>, порядок слоёв.

use std::fmt::Write as _;

use super::{
    edges::edge_svg,
    shapes::shape_svg,
    text::{label_text, shape_text},
};
use crate::layout::{measure, Edge, Layout};
use crate::style::Style;

/// ГОСТ: длина усика 0.2..0.25 высоты модуля (a = 2*grid); берём верх диапазона.
const WHISKER_FRAC: f64 = 0.25;

/// Зазор между подписью-коробкой и верхом «начала», в долях сетки:
/// в нём помещается стрелка от функции к схеме.
const TITLE_GAP_G: f64 = 2.0;
/// Отступ двойной вертикальной линии от боковой стороны коробки.
const TITLE_INSET_FRAC: f64 = 0.25;

/// Число без хвостовых нулей прямо в буфер: 14.170 -> "14.17", 1.000 -> "1",
/// -0.000 -> "0". Ни одной промежуточной String — только запись в `out`.
pub(crate) fn put(out: &mut String, v: f64) {
    let start = out.len();
    let _ = write!(out, "{v:.3}");
    let mut end = out.len();
    while end > start + 2 && out.as_bytes()[end - 1] == b'0' {
        end -= 1;
    }
    if end > start && out.as_bytes()[end - 1] == b'.' {
        end -= 1;
    }
    out.truncate(end);
    if end - start == 2 && out[start..].starts_with("-0") {
        out.remove(start); // "-0" -> "0"
    }
}

/// Масштаб вписывания раскладки в А4 (не больше 1: мелкие схемы
/// рисуются 1:1, крупные сжимаются). Подпись схемы входит в габарит:
/// иначе коробка функции вылезла бы за лист.
pub fn fit_scale(l: &Layout, st: &Style) -> f64 {
    let (_, _, w, h) = doc_bounds(l, st);
    1.0_f64
        .min((st.a4_w - 2.0 * st.page_pad) / w)
        .min((st.a4_h - 2.0 * st.page_pad) / h)
}

/// (cx, y0, w, h, верх «начала») коробки подписи; None — подписи нет.
fn title_geom(l: &Layout, st: &Style, title: &str) -> Option<(f64, f64, f64, f64, f64)> {
    // верхний блок схемы — терминатор «начало»; подпись встаёт над ним
    let start = l
        .shapes
        .iter()
        .min_by(|a, b| a.cy.partial_cmp(&b.cy).unwrap_or(std::cmp::Ordering::Equal))?;
    let (w, h) = measure(st, "act", title);
    let y_start = start.cy - start.h / 2.0;
    let y0 = y_start - TITLE_GAP_G * st.grid - h;
    Some((start.cx, y0, w, h, y_start))
}

/// Габарит документа с подписью: коробка может быть шире схемы.
fn doc_bounds(l: &Layout, st: &Style) -> (f64, f64, f64, f64) {
    let Some(title) = l.title.as_deref() else {
        return l.bounds;
    };
    let Some((cx, y0, w, _, _)) = title_geom(l, st, title) else {
        return l.bounds;
    };
    let (minx, miny, dw, dh) = l.bounds;
    let x0 = (cx - w / 2.0).min(minx);
    let x1 = (cx + w / 2.0).max(minx + dw);
    let y0 = y0.min(miny);
    (x0, y0, x1 - x0, miny + dh - y0)
}

/// Подпись схемы: прямоугольник с двойными вертикальными линиями —
/// «предопределённый процесс» по ГОСТ 19.701-90 (символ подпрограммы),
/// текст — сигнатура функции. Из низа коробки стрелка идёт в «начало».
fn title_svg(out: &mut String, l: &Layout, st: &Style, whisker: f64) {
    let Some(title) = l.title.as_deref() else {
        return;
    };
    let Some((cx, y0, w, h, y_start)) = title_geom(l, st, title) else {
        return;
    };
    let (x0, x1, y1) = (cx - w / 2.0, cx + w / 2.0, y0 + h);
    out.push_str("<rect x=\"");
    put(out, x0);
    out.push_str("\" y=\"");
    put(out, y0);
    out.push_str("\" width=\"");
    put(out, w);
    out.push_str("\" height=\"");
    put(out, h);
    out.push_str("\" fill=\"white\" stroke=\"black\" stroke-width=\"");
    put(out, st.edge_lw);
    out.push_str("\"/>");
    let inset = TITLE_INSET_FRAC * st.grid;
    for x in [x0 + inset, x1 - inset] {
        out.push_str("<line x1=\"");
        put(out, x);
        out.push_str("\" y1=\"");
        put(out, y0);
        out.push_str("\" x2=\"");
        put(out, x);
        out.push_str("\" y2=\"");
        put(out, y1);
        out.push_str("\" stroke=\"black\" stroke-width=\"");
        put(out, st.edge_lw);
        out.push_str("\"/>");
    }
    let lines = [title.to_string()];
    shape_text(out, &lines, cx, y0 + h / 2.0, st);
    edge_svg(
        out,
        &Edge {
            points: vec![(cx, y1), (cx, y_start)],
            arrow: true,
        },
        st.edge_lw,
        whisker,
    );
}

/// Пункт в миллиметре.
const PT_PER_MM: f64 = 72.0 / 25.4;

/// Ближайшая координата с фазой полпикселя (не больше `v`): попав в
/// центр пикселя, штрих в один пиксель рисуется ровно в один пиксель.
fn half_px(v: f64) -> f64 {
    let q = v * 4.0 / 3.0;
    ((q - 0.5).floor() + 0.5) * 3.0 / 4.0
}

/// SVG для вставки в отчёт: ровно по содержимому, без пустого листа.
///
/// Лист А4 удобен для печати, но в отчёт его вставлять нельзя —
/// схема занимает треть страницы, а вокруг пустое поле. Здесь
/// `viewBox` равен габариту содержимого, а `width`/`height` заданы
/// в миллиметрах из тех же пунктов, поэтому в Word и LaTeX схема
/// встаёт физического размера «миллиметр в миллиметр».
///
/// Никакого вписывания и `<g transform>`: содержимое рисуется в своих
/// же координатах 1:1. Из-за этого и у всех файлов пачки размер
/// совпадает — вписывания, которое ломало бы единообразие, тут нет.
pub fn render_svg_tight(l: &Layout, st: &Style) -> String {
    let (x, y, w, h) = doc_bounds(l, st);
    // Окно сдвигается на доли пикселя (содержимое остаётся на месте):
    // координаты, кратные модулю, попадают в центр пикселя — линии
    // выходят одной толщины и без `shape-rendering`.
    let (ox, oy) = (half_px(x), half_px(y));
    let (w, h) = (x + w - ox + 0.375, y + h - oy + 0.375);
    let whisker = WHISKER_FRAC * 2.0 * st.grid;

    let mut body = String::new();
    title_svg(&mut body, l, st, whisker);
    for e in &l.edges {
        edge_svg(&mut body, e, st.edge_lw, whisker);
    }
    for sh in &l.shapes {
        shape_svg(&mut body, sh, st.edge_lw);
        shape_text(&mut body, &sh.lines, sh.cx, sh.cy, st);
    }
    for lb in &l.labels {
        label_text(&mut body, lb, st);
    }

    let mut out = String::with_capacity(body.len() + 256);
    out.push_str(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" \
         width=\"",
    );
    put(&mut out, w / PT_PER_MM);
    out.push_str("mm\" height=\"");
    put(&mut out, h / PT_PER_MM);
    out.push_str("mm\" viewBox=\"");
    put(&mut out, ox);
    out.push(' ');
    put(&mut out, oy);
    out.push(' ');
    put(&mut out, w);
    out.push(' ');
    put(&mut out, h);
    out.push_str("\">\n<rect x=\"");
    put(&mut out, ox);
    out.push_str("\" y=\"");
    put(&mut out, oy);
    out.push_str("\" width=\"");
    put(&mut out, w);
    out.push_str("\" height=\"");
    put(&mut out, h);
    out.push_str("\" fill=\"#ffffff\"/>\n");
    out.push_str(&body);
    out.push_str("</svg>\n");
    out
}

/// Раскладка + стиль -> SVG с автоподбором масштаба.
pub fn render_svg(l: &Layout, st: &Style) -> String {
    render_svg_at(l, st, fit_scale(l, st))
}

/// То же с внешним масштабом: пачка схем рисуется одним общим s,
/// чтобы фигуры во всех файлах пачки были одного визуального размера.
/// Всё рисуется в координатах раскладки,
/// единый масштаб применяется одним `<g transform>`: линии под
/// фигурами, фигуры под подписями. Весь документ собирается в двух
/// буферах (тело и документ) — без промежуточных строк на каждый элемент.
pub fn render_svg_at(l: &Layout, st: &Style, s: f64) -> String {
    let (minx, miny, w, h) = doc_bounds(l, st);
    // усики стрелок в локальных единицах: масштаб применит g-обёртка.
    let whisker = WHISKER_FRAC * 2.0 * st.grid;

    let mut body = String::new();
    title_svg(&mut body, l, st, whisker);
    for e in &l.edges {
        edge_svg(&mut body, e, st.edge_lw, whisker);
    }
    for sh in &l.shapes {
        shape_svg(&mut body, sh, st.edge_lw);
        shape_text(&mut body, &sh.lines, sh.cx, sh.cy, st);
    }
    for lb in &l.labels {
        label_text(&mut body, lb, st);
    }

    let mut out = String::with_capacity(body.len() + 256);
    out.push_str(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"",
    );
    put(&mut out, w * s);
    out.push_str("pt\" height=\"");
    put(&mut out, h * s);
    out.push_str("pt\" viewBox=\"0 0 ");
    put(&mut out, w * s);
    out.push(' ');
    put(&mut out, h * s);
    out.push_str("\">\n<rect x=\"0\" y=\"0\" width=\"");
    put(&mut out, w * s);
    out.push_str("\" height=\"");
    put(&mut out, h * s);
    out.push_str("\" fill=\"#ffffff\"/>\n<g transform=\"translate(");
    put(&mut out, -minx * s);
    out.push(' ');
    put(&mut out, -miny * s);
    out.push_str(") scale(");
    put(&mut out, s);
    out.push_str(")\">");
    out.push_str(&body);
    out.push_str("</g>\n</svg>\n");
    out
}
