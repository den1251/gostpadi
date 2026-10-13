//! Сборка пайплайна: текст (.gvn или C) -> узлы -> страницы SVG.
//! Геометрия живёт в layout, отрисовка в generate; здесь только склейка.

use crate::drawio::book;
use crate::error::ParseError;
use crate::frontend::c::c_to_gvn_functions;
use crate::frontend::gvn::parse;
use crate::generate::{fit_scale, render_svg_at, render_svg_tight};
use crate::ir::Node;
use crate::layout::{
    layout, normalize, split_scheme, strip_markers, uniform_sizes, Edge, Label, Layout, Sizes,
};
use crate::style::Style;

pub struct Options {
    pub labels: String, // "en" | "ru"
    pub font: Option<f64>,
    pub lw: Option<f64>,
    pub no_split: bool,
}

impl Options {
    /// Стиль пайплайна: кегль и толщина пера из опций, остальное —
    /// производные (Style::with_metrics). None — значения по умолчанию.
    pub fn style(&self) -> Style {
        Style {
            no_split: self.no_split,
            ..Style::with_metrics(self.font.unwrap_or(12.0), self.lw.unwrap_or(1.0))
        }
    }
}

/// Путь схемы -> (вход, функция): «dir/main.c#main» -> («dir/main.c», Some("main")).
/// Функции нет (gvn-вход) — второй элемент None.
pub fn scheme_parts(path: &str) -> (&str, Option<&str>) {
    path.split_once('#')
        .map_or((path, None), |(a, b)| (a, Some(b)))
}

/// Пачка входов (путь, текст, это C?) -> (путь, узлы).
/// Ошибка разбора возвращается вместе с путём входа.
/// C-вход с несколькими функциями даёт по схеме на функцию:
/// путь схемы «файл.c#имя_функции» (решение 06.10: рисуем все функции).
pub fn parse_batch(
    inputs: &[(String, String, bool)],
    opts: &Options,
) -> Result<Vec<(String, Vec<Node>)>, (String, ParseError)> {
    let st = opts.style();
    let mut out = Vec::with_capacity(inputs.len());
    for (path, text, is_c) in inputs {
        let res = if *is_c {
            c_to_gvn_functions(text, &opts.labels).and_then(|fs| {
                fs.into_iter()
                    .map(|(fname, gvn)| {
                        // main — схема самого входа (main.c -> main.svg),
                        // прочие функции получают суффикс: main.c#add ->
                        // main-add.svg, иначе схема main переписала бы их
                        let scheme_path = if fname == "main" {
                            path.clone()
                        } else {
                            format!("{path}#{fname}")
                        };
                        parse(&gvn, &st, &opts.labels).map(|n| (scheme_path, n))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        } else {
            parse(text, &st, &opts.labels).map(|n| vec![(path.clone(), n)])
        };
        match res {
            Ok(mut s) => out.append(&mut s),
            Err(e) => return Err((path.clone(), e)),
        }
    }
    Ok(out)
}

/// Экспорт для отчёта: `render_tight_batch` — SVG по содержимому в
/// миллиметрах (одна схема на вход, вставка «миллиметр в миллиметр»),
/// `render_drawio_batch` — книга draw.io с редактируемыми блоками.
/// Оба идут через ту же раскладку, что и экран: вид один, отличается
/// только вывод.

/// Узлы пачки -> раскладки, общие на все файлы: размеры фигур
/// единые (uniform_sizes), как у обычного рендера.
fn export_layouts(schemes: Vec<(String, Vec<Node>)>, st: &Style) -> Vec<(String, Vec<Layout>)> {
    let normed: Vec<Sizes> = schemes.iter().map(|(_, n)| normalize(n, st)).collect();
    let sizes = uniform_sizes(&normed);
    schemes
        .into_iter()
        .map(|(path, mut nodes)| {
            // порезка не нужна: страниц нет, схема одна. Маркеры ручного
            // переноса — тоже: кружок-соединитель на одном листе сирота
            strip_markers(&mut nodes);
            let st = Style {
                no_split: true,
                ..st.clone()
            };
            (path, vec![layout(&nodes, &sizes, &st)])
        })
        .collect()
}

/// SVG по содержимому, без листа: для вставки в отчёт.
pub fn render_tight_batch(schemes: Vec<(String, Vec<Node>)>, st: &Style) -> Vec<(String, String)> {
    export_layouts(schemes, st)
        .into_iter()
        .map(|(path, parts)| {
            let joined = join_parts(&parts);
            (path, render_svg_tight(&joined, st))
        })
        .collect()
}

/// Книга draw.io по той же пачке: один файл со всеми листами.
pub fn render_drawio_batch(schemes: Vec<(String, Vec<Node>)>, st: &Style) -> Vec<(String, String)> {
    export_layouts(schemes.clone(), st)
        .into_iter()
        .map(|(path, parts)| {
            let stem = std::path::Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            let numbered: Vec<(String, Layout)> = parts
                .iter()
                .enumerate()
                .map(|(i, l)| (format!("Лист {}", i + 1), l.clone()))
                .collect();
            let xml = book(&stem, &numbered, st);
            (path, xml)
        })
        .collect()
}

/// Склеивает части порезки в одну раскладку: вертикальные смещения
/// частей складываются, иначе блоки наложатся.
fn join_parts(parts: &[Layout]) -> Layout {
    let Some(first) = parts.first() else {
        return Layout::default();
    };
    if parts.len() == 1 {
        return first.clone();
    }
    let mut out = first.clone();
    let mut shift = 0.0f64;
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            shift = part.bounds.1 - (out.bounds.1 + out.bounds.3);
        }
        for sh in &part.shapes {
            let mut sh = sh.clone();
            sh.cy += shift;
            out.shapes.push(sh);
        }
        for e in &part.edges {
            out.edges.push(Edge {
                points: e.points.iter().map(|(x, y)| (*x, y + shift)).collect(),
                arrow: e.arrow,
            });
        }
        for lb in &part.labels {
            out.labels.push(Label {
                x: lb.x,
                y: lb.y + shift,
                text: lb.text.clone(),
                ha: lb.ha.clone(),
            });
        }
    }
    let xs: Vec<f64> = out
        .shapes
        .iter()
        .flat_map(|sh| [sh.cx - sh.w / 2.0, sh.cx + sh.w / 2.0])
        .collect();
    let ys: Vec<f64> = out
        .shapes
        .iter()
        .flat_map(|sh| [sh.cy - sh.h / 2.0, sh.cy + sh.h / 2.0])
        .collect();
    let minx = xs.iter().cloned().fold(f64::MAX, f64::min);
    let miny = ys.iter().cloned().fold(f64::MAX, f64::min);
    let maxx = xs.iter().cloned().fold(f64::MIN, f64::max);
    let maxy = ys.iter().cloned().fold(f64::MIN, f64::max);
    out.bounds = (minx, miny, maxx - minx, maxy - miny);
    out
}

/// Узлы из parse_batch + стиль -> (путь, страницы SVG). Длинные схемы
/// режутся на листы А4 (или одним листом при no_split — тогда высокий
/// лист ужимает общий масштаб); размеры фигур и масштаб общие на всю
/// пачку: figures одного типа во всех файлах — одного визуального размера.
pub fn render_batch(schemes: Vec<(String, Vec<Node>)>, st: &Style) -> Vec<(String, Vec<String>)> {
    let normed: Vec<Sizes> = schemes.iter().map(|(_, n)| normalize(n, st)).collect();
    let sizes = uniform_sizes(&normed);
    let laid: Vec<(String, Vec<Layout>)> = schemes
        .into_iter()
        .map(|(path, mut nodes)| {
            let pages = if st.no_split {
                // без порезки маркер ручного переноса рисовать нечем: лист один
                strip_markers(&mut nodes);
                vec![layout(&nodes, &sizes, st)]
            } else {
                split_scheme(nodes, &sizes, st)
                    .iter()
                    .map(|part| layout(part, &sizes, st))
                    .collect()
            };
            (path, pages)
        })
        .collect();
    // единый масштаб пачки: минимальный из постраничных вписываний
    let s = laid
        .iter()
        .flat_map(|(_, pages)| pages)
        .map(|l| fit_scale(l.bounds, st))
        .fold(1.0_f64, f64::min);
    laid.into_iter()
        .map(|(path, pages)| {
            let pages = pages.iter().map(|l| render_svg_at(l, st, s)).collect();
            (path, pages)
        })
        .collect()
}

/// Один вход -> страницы SVG.
pub fn render_text(text: &str, is_c: bool, opts: &Options) -> Result<Vec<String>, ParseError> {
    match parse_batch(&[(String::new(), text.to_string(), is_c)], opts) {
        Ok(schemes) => Ok(render_batch(schemes, &opts.style()).remove(0).1),
        Err((_, e)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            labels: "en".into(),
            font: None,
            lw: None,
            no_split: false,
        }
    }

    fn rect_dims(svg: &str) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        // skip(2): мусор до первого <rect, затем белая подложка страницы
        for part in svg.split("<rect").skip(2) {
            let w = part
                .split("width=\"")
                .nth(1)
                .and_then(|s| s.split('"').next())
                .and_then(|v| v.parse::<f64>().ok());
            let h = part
                .split("height=\"")
                .nth(1)
                .and_then(|s| s.split('"').next())
                .and_then(|v| v.parse::<f64>().ok());
            if let (Some(w), Some(h)) = (w, h) {
                out.push((w, h));
            }
        }
        out.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out
    }

    fn metric(svg: &str, key: &str) -> Vec<String> {
        let mut out = Vec::new();
        let pat = format!("{key}=\"");
        for part in svg.split(&pat).skip(1) {
            if let Some(v) = part.split('"').next() {
                out.push(v.to_string());
            }
        }
        out
    }

    /// Значение scale(...) из transform: "translate(.. ..) scale(s)".
    fn scales(svg: &str) -> Vec<String> {
        svg.split("scale(")
            .skip(1)
            .filter_map(|p| p.split(')').next().map(|v| v.to_string()))
            .collect()
    }

    fn rect_widths(svg: &str) -> Vec<f64> {
        let mut out = Vec::new();
        // skip(2): шапка до <rect и белая подложка страницы
        for part in svg.split("<rect").skip(2) {
            if let Some(num) = part
                .split("width=\"")
                .nth(1)
                .and_then(|s| s.split('"').next())
            {
                if let Ok(v) = num.parse::<f64>() {
                    out.push(v);
                }
            }
        }
        out
    }

    #[test]
    fn linear_gvn_renders_one_page() {
        let pages = render_text(
            "input scanf(\"%d\", &a)\nc = a * 2\noutput printf(\"c = %d\", c)\n",
            false,
            &opts(),
        )
        .unwrap();
        assert_eq!(pages.len(), 1);
        assert!(pages[0].starts_with("<?xml"));
    }

    #[test]
    fn c_source_renders_one_page() {
        let pages = render_text("int main(){printf(\"hi\");return 0;}", true, &opts()).unwrap();
        assert_eq!(pages.len(), 1);
        assert!(pages[0].starts_with("<?xml"));
    }

    #[test]
    fn batch_uses_uniform_sizes() {
        // широкая плитка в A должна раздуть act-размер и в B
        let a = "x = aaaaaaaa * bbbbbbbb * cccccccc * dddddddd\noutput printf(1)\n";
        let b = "input scanf(1)\nx = 1\noutput printf(2)\n";
        let schemes = parse_batch(
            &[
                ("a.gvn".into(), a.into(), false),
                ("b.gvn".into(), b.into(), false),
            ],
            &opts(),
        )
        .unwrap();
        let out = render_batch(schemes, &opts().style());
        assert_eq!(out.len(), 2);
        let (mut wa, mut wb) = (rect_widths(&out[0].1[0]), rect_widths(&out[1].1[0]));
        wa.sort_by(|x, y| y.partial_cmp(x).unwrap());
        wb.sort_by(|x, y| y.partial_cmp(x).unwrap());
        assert_eq!(
            wa[0], wb[0],
            "самая широкая фигура одинакова в обеих схемах"
        );
    }

    /// Batch-атомарность: масштаб и кегль во всех файлах пачки
    /// посимвольно одинаковы, фигуры одного типа — одного размера.
    #[test]
    fn batch_schemas_share_scale_font_and_rect_sizes() {
        // широкая схема (переключатель) заставит пачку масштабироваться
        let a = "if switch (d)\n    1: printf(\"один\"); break\n    2: printf(\"два\"); break\n    3: printf(\"три\"); break\n    4: printf(\"четыре\"); break\n    иначе: printf(\"много\"); break\noutput printf(d)\n";
        let b = "input scanf(1)\nx = 1\noutput printf(2)\n";
        let schemes = parse_batch(
            &[
                ("a.gvn".into(), a.into(), false),
                ("b.gvn".into(), b.into(), false),
            ],
            &opts(),
        )
        .unwrap();
        let out = render_batch(schemes, &opts().style());
        assert_eq!(out.len(), 2);
        let sa = scales(&out[0].1[0]);
        let sb = scales(&out[1].1[0]);
        assert_eq!(sa, sb, "scale в пачке одинаков посимвольно");
        assert!(
            !sa.is_empty() && sa[0] != "1",
            "схема A реально масштабирована"
        );
        let fa = metric(&out[0].1[0], "font-size");
        let fb = metric(&out[1].1[0], "font-size");
        // значений может быть разное количество (фигур в A больше),
        // но сами значения кегля — одно и то же множество
        let mut ua = fa.clone();
        ua.sort();
        ua.dedup();
        let mut ub = fb.clone();
        ub.sort();
        ub.dedup();
        assert_eq!(ua, ub, "font-size в пачке одинаков посимвольно");
        // блоков разное количество (в A switch с 5 ветками), но каждый
        // размер фигуры из A встречается и в B — фигуры одного типа
        // одного размера во всей пачке
        let mut ra = rect_dims(&out[0].1[0]);
        ra.sort_by(|a, b| a.partial_cmp(b).unwrap());
        ra.dedup();
        let mut rb = rect_dims(&out[1].1[0]);
        rb.sort_by(|a, b| a.partial_cmp(b).unwrap());
        rb.dedup();
        assert!(ra.iter().all(|d| rb.contains(d)), "{ra:?} vs {rb:?}");
    }

    /// Экспорт для отчёта: SVG по содержимому — без листа и без
    /// вписывания, размер в миллиметрах (в Word встаёт 1:1).
    #[test]
    fn tight_export_has_mm_and_no_scaling() {
        let schemes = parse_batch(
            &[(
                "a.gvn".into(),
                "input scanf(1)\nx = 2\noutput printf(3)\n".into(),
                false,
            )],
            &opts(),
        )
        .unwrap();
        let out = render_tight_batch(schemes, &opts().style());
        assert_eq!(out.len(), 1);
        let svg = &out[0].1;
        assert!(svg.contains("width=\"") && svg.contains("mm\""), "нет мм");
        assert!(!svg.contains("scale("), "tight-экспорт без вписывания");
        assert!(svg.contains("<rect"), "нет подложки");
    }

    /// Draw.io-книга — редактируемый XML со ГОСТ-формами, одна на вход.
    #[test]
    fn drawio_export_is_editable_mxfile() {
        let schemes =
            parse_batch(&[("a.gvn".into(), "x = 1\ny = 2\n".into(), false)], &opts()).unwrap();
        let out = render_drawio_batch(schemes, &opts().style());
        assert_eq!(out.len(), 1);
        let xml = &out[0].1;
        assert!(xml.contains("<mxfile host=\"gostpadi\">"));
        assert!(xml.contains("vertex=\"1\""), "блоки не редактируемые");
        assert!(
            xml.matches("<diagram ").count() == 1,
            "одна схема — один diagram"
        );
    }

    /// Маркер ручного переноса `// перенос` до SVG не доезжает: на
    /// страницах — кружки-соединители, слова «перенос» нет.
    #[test]
    fn marker_never_reaches_pages() {
        let src = "int main(void) {\n    int a = 1;\n    // перенос\n    int b = 2;\n}";
        let pages = render_text(src, true, &opts()).unwrap();
        assert_eq!(pages.len(), 2, "маркер ломает страницу");
        for p in &pages {
            assert!(!p.contains("перенос"), "маркер попал в SVG");
            assert!(p.contains("<circle"), "нет кружка-соединителя");
        }
    }

    /// no_split: маркер съедается — одна страница, ни одного кружка
    /// (--trim/--drawio идут тем же стрипом).
    #[test]
    fn no_split_swallows_markers() {
        let src = "int main(void) {\n    int a = 1;\n    // перенос\n    int b = 2;\n}";
        let mut o = opts();
        o.no_split = true;
        let pages = render_text(src, true, &o).unwrap();
        assert_eq!(pages.len(), 1);
        assert!(!pages[0].contains("перенос"), "маркер попал в SVG");
        assert!(!pages[0].contains("<circle"), "сиротский кружок-маркер");
        // экспорт отчёта — тот же стрип
        let schemes = parse_batch(&[("a.c".into(), src.into(), true)], &opts()).unwrap();
        let xml = &render_drawio_batch(schemes, &opts().style())[0].1;
        assert!(!xml.contains("перенос"), "маркер попал в draw.io");
    }
}
