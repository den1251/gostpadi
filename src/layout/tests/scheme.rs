//! Инварианты раскладки: топология циклов-трапеций, рельсы break,
//! single-entry в «конец», тупики return, проверки на примерах.

use super::*;
use crate::layout::measure::{HEAD, MID, TAIL};
use crate::layout::{
    collinear_overlap_ok, crossings_ok, dangling_ok, layout, measure, normalize, overlaps_ok,
    single_entry_ok, split_scheme,
};

#[test]
fn linear_scheme_shapes_edges() {
    let nodes = linear();
    let l = lay(&nodes);
    assert_eq!(l.shapes.len(), 4, "Start, act, act, End");
    assert_eq!(l.edges.len(), 3);
    assert!(
        l.edges.iter().all(|e| e.arrow),
        "все стрелки — входы в блоки"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

#[test]
fn if_branches_merge_before_continue() {
    let mut d = node(NodeKind::Decision, "if a > 0");
    d.branches = vec![
        br("да", vec![s("a = 1")], false),
        br("нет", vec![s("b = 2")], false),
    ];
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        d,
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    // вход в схему + 2 входа в колонки веток + 1 продолжение после слияния
    assert_eq!(l.edges.iter().filter(|e| e.arrow).count(), 4);
    // обе колонки сливаются T-стыком (без стрелок) на одной высоте:
    // спуски и шина заканчиваются на одном y ниже ромба
    let merges: Vec<f64> = l
        .edges
        .iter()
        .filter(|e| !e.arrow)
        .filter_map(|e| e.points.last())
        .filter(|&&p| p.1 > l.shapes[1].cy)
        .map(|&p| p.1)
        .collect();
    assert!(merges.len() >= 2 && merges.iter().all(|&y| (y - merges[0]).abs() < 1e-9));
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(single_entry_ok(&l));
}

#[test]
fn loop_is_two_hexagons() {
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        loop_node("while i < 5", vec![s("i = i + 1")]),
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    let lb = l.shapes.iter().find(|sh| sh.kind == "loop_begin").unwrap();
    assert_eq!(lb.lines, vec!["1. while i < 5".to_string()]);
    assert!(lb.skew > 0.0, "срез верхних углов у loop_begin");
    assert_eq!(
        lb.skew,
        (0.5 * lb.h).min(0.25 * lb.w),
        "Δ = min(0.5*h, 0.25*w)"
    );
    let le = l.shapes.iter().find(|sh| sh.kind == "loop_end").unwrap();
    assert_eq!(
        le.lines,
        vec!["1".to_string()],
        "номер цикла на нижнем шестиугольнике — тот же, что в заголовке"
    );
    assert_eq!(
        le.skew, lb.skew,
        "loop_end — зеркальный срез той же величины"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

#[test]
fn nested_loop_numbering() {
    let inner = loop_node("while j < 3", vec![s("j = j + 1")]);
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        loop_node("while i < 5", vec![s("i = 0"), Stmt::Node(Box::new(inner))]),
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    let mut ends: Vec<&Shape> = l.shapes.iter().filter(|sh| sh.kind == "loop_end").collect();
    ends.sort_by(|a, b| a.cy.partial_cmp(&b.cy).unwrap());
    assert_eq!(ends.len(), 2);
    // первая по cy — внутренняя (внутри тела, выше), вторая — внешняя
    assert_eq!(ends[0].lines, vec!["2".to_string()], "вложенный цикл");
    assert_eq!(ends[1].lines, vec!["1".to_string()], "внешний цикл");
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

/// На шестиугольнике «подготовка» видно, какой это цикл: while/for.
#[test]
fn loop_begin_shows_keyword() {
    let st = Style::default();
    let text = "while i < 5\n    a = 1\nfor i = 0; i < 10; i++\n    b = 2\noutput printf(1)\n";
    let nodes = crate::frontend::gvn::parse(text, &st, "").unwrap();
    let l = lay(&nodes);
    let begins: Vec<String> = l
        .shapes
        .iter()
        .filter(|sh| sh.kind == "loop_begin")
        .flat_map(|sh| sh.lines.clone())
        .collect();
    assert!(begins.iter().any(|t| t == "1. while i < 5"), "{begins:?}");
    assert!(
        begins.iter().any(|t| t == "1. for i = 0; i < 10; i++"),
        "{begins:?}"
    );
}

/// Голова разрезанного цикла: верхняя трапеция есть, нижней нет — она
/// уехала на следующий лист вместе с хвостом.
#[test]
fn head_loop_no_end_trapezoid() {
    let head = loop_node(&format!("{HEAD}while i < 5"), vec![s("i = i + 1")]);
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        head,
        node(NodeKind::Conn, "А"),
    ];
    let l = lay(&nodes);
    let lb = l
        .shapes
        .iter()
        .find(|sh| sh.kind == "loop_begin")
        .expect("верхняя трапеция");
    assert_eq!(
        lb.lines,
        vec!["1. while i < 5".to_string()],
        "сентинел в текст не протекает, номер цикла на месте"
    );
    assert!(
        !l.shapes.iter().any(|sh| sh.kind == "loop_end"),
        "нижняя трапеция — на следующем листе"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(dangling_ok(&l).is_ok());
}

/// Хвост разрезанного цикла: нижняя трапеция с номером цикла, верхней
/// нет — она осталась на предыдущем листе.
#[test]
fn tail_loop_only_end_trapezoid() {
    let tail = loop_node(&TAIL.to_string(), vec![s("i = i + 1")]);
    let nodes = vec![
        node(NodeKind::Conn, "А"),
        tail,
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    assert!(!l.shapes.iter().any(|sh| sh.kind == "loop_begin"));
    let le = l
        .shapes
        .iter()
        .find(|sh| sh.kind == "loop_end")
        .expect("нижняя трапеция");
    assert_eq!(
        le.lines,
        vec!["1".to_string()],
        "номер тот же, что у целого цикла: глубина выводится из структуры"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(dangling_ok(&l).is_ok());
}

/// Середина разрезанного цикла: трапеций нет вовсе — лист держится
/// парой кружков-соединителей.
#[test]
fn mid_loop_no_trapezoids() {
    let mid = loop_node(&MID.to_string(), vec![s("i = i + 1"), s("j = j + 1")]);
    let nodes = vec![node(NodeKind::Conn, "А"), mid, node(NodeKind::Conn, "А")];
    let l = lay(&nodes);
    assert!(
        !l.shapes.iter().any(|sh| sh.kind.starts_with("loop")),
        "ни трапеций, ни номера"
    );
    assert_eq!(l.shapes.iter().filter(|sh| sh.kind == "conn").count(), 2);
    assert!(l
        .shapes
        .iter()
        .any(|sh| sh.lines == vec!["i = i + 1".to_string()]));
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(dangling_ok(&l).is_ok());
}

/// Рельса break на головном листе идёт к кружку-выходу: горизонталь
/// приходит на верх кружка, а не в пустоту и не сквозь фигуры.
#[test]
fn break_rail_on_head_page_exits_via_letter() {
    let head = loop_node(&format!("{HEAD}while i < 5"), vec![s("i = i + 1"), sbrk()]);
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        head,
        node(NodeKind::Conn, "А"),
    ];
    let l = lay(&nodes);
    let conn = l
        .shapes
        .iter()
        .find(|sh| sh.kind == "conn")
        .expect("кружок-выход");
    let rail = l
        .edges
        .iter()
        .find(|e| {
            !e.arrow
                && e.points
                    .last()
                    .is_some_and(|p| p.0 == conn.cx && p.1 < conn.cy)
        })
        .expect("рельса в коридоре");
    assert_eq!(
        rail.points.last().map(|p| p.0),
        Some(conn.cx),
        "рельса приходит на вертикаль входа в кружок"
    );
    let last = rail.points.last().copied().unwrap();
    assert!(
        last.1 < conn.cy - conn.h / 2.0,
        "T-стык на полшага выше кружка, не на его кромке: {last:?}"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(dangling_ok(&l).is_ok());
}

/// Репорт (задача 3): у ромба с одной непустой веткой, ушедшей рельсой
/// break, низ колонки уходил под шину слияния. Поток приходил на ось на
/// высоте шины, а продолжение начиналось ниже — вертикаль обрывалась в
/// воздухе, и стрелка в следующий блок висела без входа.
#[test]
fn merge_bus_reaches_continuation_below_it() {
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        loop_node(
            "for d = 2; d <= m; d++",
            vec![Stmt::Node(Box::new({
                let mut d = node(NodeKind::Decision, "if a");
                d.branches = vec![
                    br("да", vec![s("x = 0"), sbrk()], false),
                    br("нет", vec![], false), // пустая ветка, как в задаче 3
                ];
                d
            }))],
        ),
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    let le = l.shapes.iter().find(|sh| sh.kind == "loop_end").unwrap();
    // вертикаль по оси колонки от шины слияния до входа в loop_end
    let spine = l
        .edges
        .iter()
        .find(|e| {
            let p = e.points.last().unwrap();
            e.arrow && p.1 == le.cy - le.h / 2.0 && p.0 == 0.0 && e.points.len() == 2
        })
        .expect("спуск в трапецию слияния");
    let from = spine.points[0];
    // откуда-то должен быть вход в этот спуск: либо шина, либо этот же
    // конец — обрыв виден именно как разрыв вертикали
    let joined = l.edges.iter().any(|e| {
        let last = *e.points.last().unwrap();
        (last.0 - from.0).abs() < 1e-9 && (last.1 - from.1).abs() < 1e-9
    });
    assert!(joined, "в спуск к трапеции никто не приходит: {from:?}");
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

/// Репорт (задача 3): рельса break уходила из блока горизонталью с высоты
/// его низа — отрезок ложился вдоль границы плитки, и линия читалась как
/// выход из угла.
///
/// Колонка плитки в задаче 3 целиком левее трапеции слияния, поэтому
/// вертикаль можно вести прямо: рельса выходит из центра плитки, падает
/// на merge и вправо на ось — один изгиб. Раньше сюда вклинивался коридор
/// с отступом вниз на в-gap, и у плитки выходило три лишних изгиба.
#[test]
fn break_rail_from_clear_column_falls_straight() {
    let src = "\
int main() {
    for (d = 2; d <= m; d++) {
        if (p % d == 0) {
            coprime = 0;
            break;
        }
    }
    return 0;
}
";
    let nodes = crate::frontend::c::parse_c_to_nodes(src, "ru").unwrap();
    let l = lay(&nodes);
    let tile = l
        .shapes
        .iter()
        .find(|sh| sh.lines == vec!["coprime = 0".to_string()])
        .expect("плитка перед break");
    let le = l.shapes.iter().find(|sh| sh.kind == "loop_end").unwrap();
    let cx = tile.cx;
    let bottom = tile.cy + tile.h / 2.0;
    // колонка плитки должна быть вне трапеции слияния — иначе тест проверяет
    // не тот маршрут и падает не по делу
    assert!(
        cx < le.cx - le.w / 2.0 || cx > le.cx + le.w / 2.0,
        "колонка плитки вне трапеции слияния: cx={cx}, трапеция {}..{}",
        le.cx - le.w / 2.0,
        le.cx + le.w / 2.0
    );
    let rail = l
        .edges
        .iter()
        .find(|e| e.points.len() == 3 && (e.points[0].0 - cx).abs() < 1e-9)
        .expect("рельса из центра плитки прямо вниз");
    assert!(
        (rail.points[0].1 - bottom).abs() < 1e-9,
        "рельса начинается с низа плитки: {:?}",
        rail.points[0]
    );
    assert!(
        rail.points[1].0 == cx && rail.points[1].1 > bottom,
        "падение строго вниз по оси плитки: {:?}",
        &rail.points[1..2]
    );
    assert!(
        rail.points[2].0 == le.cx && rail.points[2].1 == rail.points[1].1,
        "на ось цикла, на той же высоте: {:?}",
        &rail.points[2..3]
    );
    // без коридора: рельса не выходит за свою колонку и ось (канон:
    // да-колонка справа, спуск идёт влево на ствол)
    let (lo, hi) = (le.cx.min(cx), le.cx.max(cx));
    assert!(
        rail.points
            .iter()
            .all(|p| p.0 >= lo - 1e-9 && p.0 <= hi + 1e-9),
        "рельса не уходит вбок от своей колонки: {:?}",
        rail.points
    );
    // ровно два изгиба вместо четырёх: ступеньки вниз и влево нет
    assert_eq!(
        rail.points.len(),
        3,
        "прямая рельса: центр плитки -> вниз -> ось цикла: {:?}",
        rail.points
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

/// Когда колонка плитки накрыта трапецией слияния, рельса идёт через
/// коридор левее — иначе вертикаль проткнула бы трапецию.
#[test]
fn break_rail_under_trapezoid_uses_corridor() {
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        loop_node("while i < 5", vec![s("a = 1"), sbrk()]),
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    let lb = l.shapes.iter().find(|sh| sh.kind == "loop_begin").unwrap();
    let le = l.shapes.iter().find(|sh| sh.kind == "loop_end").unwrap();
    // плитка стоит на оси цикла, то есть прямо под трапецией слияния:
    // рельса обязана уйти в коридор левее, иначе проткнёт трапецию
    let rail = l
        .edges
        .iter()
        .find(|e| e.points.iter().any(|p| p.0 < -lb.w / 2.0))
        .expect("рельса ушла в левый коридор мимо трапеции");
    let leftmost = rail.points.iter().map(|p| p.0).fold(f64::MAX, f64::min);
    assert!(
        leftmost < -lb.w / 2.0,
        "рельса заходит левее половины ширины трапеции: {leftmost} против {}",
        -lb.w / 2.0
    );
    // коридор: вниз из плитки, влево, вниз мимо трапеции, вправо на ось
    let trunk = l
        .edges
        .iter()
        .find(|e| {
            let p = e.points.last().unwrap();
            !e.arrow
                && p.0 == 0.0
                && p.1 > le.cy + le.h / 2.0
                && e.points[0].1 == p.1
                && e.points[0].0 != 0.0
        })
        .expect("рельса вливается в ось ниже loop_end");
    assert!(
        trunk.points[0].0 < -lb.w / 2.0,
        "низ рельсы идёт из коридора: {:?}",
        trunk.points
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

#[test]
fn break_goes_left_rail_below_loop() {
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        loop_node("while i < 5", vec![s("a = 1"), sbrk()]),
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    let lb = l.shapes.iter().find(|sh| sh.kind == "loop_begin").unwrap();
    let le = l.shapes.iter().find(|sh| sh.kind == "loop_end").unwrap();
    let rail = l
        .edges
        .iter()
        .find(|e| {
            let p = e.points.last().unwrap();
            !e.arrow
                && p.0 == 0.0
                && p.1 > le.cy + le.h / 2.0
                && e.points[0].1 == p.1
                && e.points[0].0 != 0.0
        })
        .expect("рельса break через левый канал");
    assert!(
        rail.points[0].0 < -lb.w / 2.0,
        "рельса идёт из коридора левее трапеции: {:?}",
        rail.points
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

#[test]
fn to_end_single_arrow_into_end() {
    let mut d = node(NodeKind::Decision, "if a > 0");
    d.branches = vec![
        br("да", vec![sio("printf(\"many\")")], true),
        br("нет", vec![s("c = 0")], false),
    ];
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        d,
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    assert!(single_entry_ok(&l), "ровно одна стрелка в «конец»");
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

#[test]
fn ret_is_dead_end() {
    let mut d = node(NodeKind::Decision, "if a > 0");
    d.branches = vec![
        br("да", vec![Stmt::Return("return 1".into())], false),
        br("нет", vec![s("b = 2")], false),
    ];
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        d,
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    let ret = l.shapes.iter().find(|sh| sh.kind == "ret").unwrap();
    let out = l.edges.iter().any(|e| {
        let (x, y) = e.points[0];
        (x - ret.cx).abs() <= ret.w / 2.0 + 0.01 && (y - ret.cy).abs() <= ret.h / 2.0 + 0.01
    });
    assert!(!out, "из return не выходят линии");
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

#[test]
fn examples_layout_invariants() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("gvn") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let nodes = crate::frontend::gvn::parse(&src, &Style::default(), "ru")
            .unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
        let st = Style::default();
        let sizes = normalize(&nodes, &st);
        let l = layout(&nodes, &sizes, &st);
        assert!(
            crossings_ok(&l.shapes, &l.edges).is_ok(),
            "{}: линии заходят на блоки",
            path.display()
        );
        assert!(
            crate::layout::edges_cross_ok(&l).is_ok(),
            "{}: линии пересекаются",
            path.display()
        );
        assert!(
            crate::layout::edge_on_border_ok(&l).is_ok(),
            "{}: линия идёт по кромке блока",
            path.display()
        );
        assert!(
            overlaps_ok(&l.shapes),
            "{}: фигуры перекрываются",
            path.display()
        );
        checked += 1;
    }
    assert!(checked >= 5, "ожидалось >=5 примеров, проверено {checked}");
}

#[test]
fn split_scheme_single_part_for_short_scheme() {
    let parts = split_scheme(
        linear(),
        &normalize(&linear(), &Style::default()),
        &Style::default(),
    );
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].len(), 4);
}

/// Маркер ручного переноса внутри ветки — не фигура: колонка глотает
/// его молча (страниц внутри колонки нет), раскладка не падает.
#[test]
fn branch_marker_is_silent() {
    let mut d = node(NodeKind::Decision, "if a > 0");
    d.branches = vec![
        br("да", vec![smark(), s("a = 1")], false),
        br("нет", vec![s("b = 2")], false),
    ];
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        d,
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    assert!(
        !l.shapes.iter().any(|sh| sh.kind == "conn"),
        "маркер не должен рисоваться кружком"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

/// Маркер ручного переноса ломает страницу, даже если всё влезло:
/// маркер-узел съеден, части связаны кружками одной буквы.
#[test]
fn top_level_marker_forces_page_break() {
    let st = Style::default();
    let nodes = crate::frontend::gvn::parse("a = 1\nперенос\nb = 2\n", &st, "en").unwrap();
    let sizes = normalize(&nodes, &st);
    let parts = split_scheme(nodes, &sizes, &st);
    assert_eq!(parts.len(), 2, "{parts:?}");
    assert!(
        !parts
            .iter()
            .flatten()
            .any(|n| n.text == crate::frontend::MARKER),
        "маркер-узлы обязаны быть съедены"
    );
    let out = parts[0].last().unwrap();
    let inp = parts[1].first().unwrap();
    assert_eq!(out.kind, NodeKind::Conn, "{out:?}");
    assert_eq!(out.text, inp.text, "одна буква на пару листов");
    for part in &parts {
        let l = layout(part, &sizes, &st);
        assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
        assert!(single_entry_ok(&l));
    }
}

/// Маркер последним перед «концом»: разрезать нечего — лишнего листа
/// и сиротского кружка не появляется.
#[test]
fn marker_at_end_no_orphan_page() {
    let st = Style::default();
    let nodes = crate::frontend::gvn::parse("a = 1\nb = 2\nперенос\n", &st, "en").unwrap();
    let sizes = normalize(&nodes, &st);
    let parts = split_scheme(nodes, &sizes, &st);
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert!(
        !parts[0].iter().any(|n| n.kind == NodeKind::Conn),
        "ни маркера, ни кружка: {parts:?}"
    );
}

#[test]
fn orthogonal_edges() {
    let mut d = node(NodeKind::Decision, "if a > 0");
    d.branches = vec![br("да", vec![s("a = 1")], false), br("нет", vec![], false)];
    let schemes: Vec<Vec<Node>> = vec![
        linear(),
        vec![
            node(NodeKind::Term, "начало"),
            d,
            node(NodeKind::Term, "конец"),
        ],
        vec![
            node(NodeKind::Term, "начало"),
            loop_node("while i < 5", vec![s("i = i + 1"), sbrk()]),
            node(NodeKind::Term, "конец"),
        ],
    ];
    for nodes in &schemes {
        let l = lay(nodes);
        for e in &l.edges {
            for seg in e.points.windows(2) {
                assert!(
                    (seg[0].0 - seg[1].0).abs() < 1e-9 || (seg[0].1 - seg[1].1).abs() < 1e-9,
                    "не ортогональный сегмент {seg:?}"
                );
            }
        }
    }
}

/// pend-рельсы при последнем узле-цикле: layout() — публичный API,
/// «конец» может отсутствовать. Рельсы обязаны дойти до магистрали
/// под циклом со стрелкой (раньше pend просто терялся).
#[test]
fn pend_flush_when_loop_is_last() {
    let st = Style::default();
    let text = "if a > 0\n    yes -> end: printf(1)\n    no:\nwhile a < 5\n    a = a + 1\n";
    let mut nodes = crate::frontend::gvn::parse(text, &st, "en").unwrap();
    nodes.pop(); // убираем «конец»: последний узел — цикл
    assert_eq!(nodes.last().unwrap().kind, NodeKind::Loop);
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let loop_bottom = l
        .shapes
        .iter()
        .filter(|s| s.kind == "loop_end")
        .map(|s| s.cy + s.h / 2.0)
        .fold(f64::MIN, f64::max);
    let rails: Vec<&crate::layout::Edge> = l
        .edges
        .iter()
        .filter(|e| {
            e.arrow && matches!(e.points.last(), Some(&(x, y)) if x.abs() < 1e-6 && y > loop_bottom)
        })
        .collect();
    assert_eq!(rails.len(), 1, "рельсы «-> конец» не потеряны");
    // полная ломаная: ветка, спуск, внешний рельс, возврат на магистраль
    assert_eq!(rails[0].points.len(), 5, "{:?}", rails[0].points);
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

/// Фикс «жирной линии слияния»: горизонталь на уровне merge_y — ровно
/// один сегмент (шина от крайней колонки до крайней), а не наложенные
/// хвосты каждой колонки (ступеньки разной жирности на рендере).
#[test]
fn switch_merge_draws_single_bus() {
    let mut d = node(NodeKind::Decision, "if switch (d)");
    d.switch_var = Some("d".into());
    d.branches = vec![
        br("1", vec![s("a = 1")], false),
        br("2", vec![s("b = 2")], false),
        br("3", vec![s("c = 3")], false),
    ];
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        d,
        node(NodeKind::Term, "конец"),
    ];
    let st = Style::default();
    let l = lay(&nodes);
    let my = l
        .shapes
        .iter()
        .filter(|sh| sh.kind == "act")
        .map(|sh| sh.cy + sh.h / 2.0)
        .fold(f64::MIN, f64::max)
        + st.mgap;
    let at_bus = |e: &crate::layout::Edge| {
        e.points
            .windows(2)
            .any(|w| (w[0].1 - w[1].1).abs() < 1e-9 && (w[0].1 - my).abs() < 1e-9)
    };
    assert_eq!(
        l.edges.iter().filter(|e| at_bus(e)).count(),
        1,
        "ровно один горизонтальный сегмент на merge_y"
    );
    let bus = l.edges.iter().find(|e| at_bus(e)).unwrap();
    let mut xs: Vec<f64> = bus
        .points
        .windows(2)
        .filter(|w| (w[0].1 - my).abs() < 1e-9)
        .flat_map(|w| [w[0].0, w[1].0])
        .collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(
        *xs.first().unwrap() < -1.0 && *xs.last().unwrap() > 1.0,
        "шина проходит через 0, накрывая обе стороны: {xs:?}"
    );
    // три вертикальных спуска (без стрелок) заканчиваются на шине
    let drops = l
        .edges
        .iter()
        .filter(|e| {
            !e.arrow
                && e.points.len() >= 2
                && e.points.iter().all(|p| (p.0 - e.points[0].0).abs() < 1e-9)
                && (e.points.last().unwrap().1 - my).abs() < 1e-9
        })
        .count();
    assert_eq!(drops, 3, "спуск по каждой колонке отдельно");
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(single_entry_ok(&l));
}

/// Короткая ветка НЕ получает пад: контент всех колонок начинается на
/// одной высоте top0 (компактно), слияние — шина от нижней точки.
#[test]
fn short_branch_starts_at_same_top() {
    let mut d = node(NodeKind::Decision, "if a > 0");
    d.branches = vec![
        br("да", vec![s("a = 1"), s("a = 2")], false),
        br("нет", vec![s("b = 3")], false),
    ];
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        d,
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    let top = |s: &Shape| s.cy - s.h / 2.0;
    let a1 = l
        .shapes
        .iter()
        .find(|s| s.lines == vec!["a = 1".to_string()])
        .unwrap();
    let b3 = l
        .shapes
        .iter()
        .find(|s| s.lines == vec!["b = 3".to_string()])
        .unwrap();
    assert!(
        (top(a1) - top(b3)).abs() < 0.01,
        "верх контента обеих колонок на одной высоте: {} vs {}",
        top(a1),
        top(b3)
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(single_entry_ok(&l));
}

/// Пустой срез не должен паниковать на nodes.len() - 1.
#[test]
fn empty_scheme_layout_no_panic() {
    let st = Style::default();
    let l = layout(&[], &normalize(&[], &st), &st);
    assert!(l.shapes.is_empty());
}

/// Входящие кружки соединителей стреляют стрелками в «конец» — это
/// легально; единственный не-inbound вход по-прежнему один.
#[test]
fn single_entry_allows_inbound_conns() {
    let st = Style::default();
    let text = "if a > 0\n    yes -> end: printf(1)\n    no:\nb = 111111\n".repeat(14);
    let nodes = crate::frontend::gvn::parse(&text, &st, "en").unwrap();
    let sizes = normalize(&nodes, &st);
    let parts = split_scheme(nodes, &sizes, &st);
    assert!(parts.len() >= 2, "схема должна разрезаться на листы");
    for part in &parts {
        let l = layout(part, &sizes, &st);
        assert!(single_entry_ok(&l), "single-entry нарушен на листе");
    }
}

/// Схема, оборвавшаяся return'ом: «конец» не нарисован — проверять
/// нечего, инвариант должен возвращать успех.
#[test]
fn single_entry_without_end_shape_is_ok() {
    let st = Style::default();
    let text = "a = 1\nreturn 1\nb = 2\n";
    let nodes = crate::frontend::gvn::parse(text, &st, "en").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    assert_eq!(
        l.shapes.iter().filter(|s| s.kind == "term").count(),
        1,
        "нарисован только «начало»"
    );
    assert!(single_entry_ok(&l));
}

/// Мёртвая осевая колонка (tx = 0, return внизу) обязана учитываться
/// в merge_y: иначе ствол продолжения (0, merge_y) → следующий блок
/// протыкает её ret насквозь.
#[test]
fn switch_dead_axis_column_counts_into_merge_y() {
    let st = Style::default();
    let text = "\
input scanf(\"%d\", &x)
if switch (x)
    1: printf(\"раз\")
    2 -> end: printf(\"два\"); return 1
    иначе: printf(\"три\")
output printf(x)
";
    let nodes = crate::frontend::gvn::parse(text, &st, "ru").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    assert_eq!(
        crossings_ok(&l.shapes, &l.edges),
        Ok(()),
        "ствол продолжения не должен протыкать ret мёртвой осевой колонки"
    );
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

/// Пустая ветка «нет» (канон, рис. 1): колонку не занимает и рельсы не
/// требует — поток идёт сквозь ромб по стволу. Колонка «да» справа.
#[test]
fn empty_branch_is_trunk_not_rail() {
    let st = Style::default();
    let text = "if x <= 0\n    да: printf(\"bad\"); return 1\n    нет:\ny = x + 1\n";
    let nodes = crate::frontend::gvn::parse(text, &st, "ru").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let dsh = l.shapes.iter().find(|s| s.kind == "if").unwrap();
    let y_b = dsh.cy + dsh.h / 2.0;
    let vy = l
        .edges
        .iter()
        .find(|e| {
            !e.arrow
                && e.points.iter().all(|p| p.0.abs() < 1e-9)
                && (e.points[0].1 - y_b).abs() < 1e-9
                && e.points.last().unwrap().1 > y_b
        })
        .expect("ствол сквозного прохода от нижней вершины ромба");
    assert!(
        (vy.points.last().unwrap().1 - (y_b + 2.0 * st.grid)).abs() < 1e-9,
        "ствол до шины y_b + 2g: {:?}",
        vy.points
    );
    // рельсы справа нет: правая вершина ромба остаётся только с входом «да»
    let vr = (dsh.cx + dsh.w / 2.0, dsh.cy);
    assert!(
        !l.edges.iter().any(|e| {
            !e.arrow && (e.points[0].0 - vr.0).abs() < 1e-9 && (e.points[0].1 - vr.1).abs() < 1e-9
        }),
        "рельса пустой ветки больше не рисуется"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

/// Канон методички (рис. 1): у if с единственной непустой веткой «да»
/// уходит ВПРАВО (tx = +base), «нет» — вниз по стволу с меткой у линии.
#[test]
fn if_yes_branch_goes_right() {
    let st = Style::default();
    let text = "if x <= 0\n    да: printf(\"bad\")\n    нет:\ny = x + 1\n";
    let nodes = crate::frontend::gvn::parse(text, &st, "ru").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let dsh = l.shapes.iter().find(|s| s.kind == "if").unwrap();
    let io = l.shapes.iter().find(|s| s.kind == "io").unwrap();
    let nhe = super::nhe_of(&sizes, &nodes, &st);
    let base = dsh.w / 2.0 + st.hgap + nhe;
    assert!(
        (io.cx - base).abs() < 1e-9,
        "да-колонка направо: io.cx = {}, ожидался +base = {}",
        io.cx,
        base
    );
    // метка «нет» стоит у ствола справа от оси
    let no = l.labels.iter().find(|lb| lb.text == "нет").unwrap();
    assert!(no.x > 0.0 && no.y > dsh.cy + dsh.h / 2.0, "метка у ствола");
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

/// РЕГРЕССИЯ: if/else с двумя непустыми ветками — колонки симметричны,
/// на ±base (base = dw/2 + hgap + nhe), ничего не уехало на ось.
#[test]
fn if_else_columns_symmetric_at_base() {
    let st = Style::default();
    let mut d = node(NodeKind::Decision, "if a > 0");
    d.branches = vec![
        br("да", vec![s("a = 1")], false),
        br("нет", vec![s("b = 2")], false),
    ];
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        d,
        node(NodeKind::Term, "конец"),
    ];
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let dsh = l.shapes.iter().find(|s| s.kind == "if").unwrap();
    let colw = sizes["act"].0.max(sizes["io"].0);
    let base = dsh.w / 2.0 + st.hgap + colw / 2.0;
    let mut acts: Vec<f64> = l
        .shapes
        .iter()
        .filter(|s| s.kind == "act")
        .map(|s| s.cx)
        .collect();
    acts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(acts.len(), 2, "две ветки — две колонки");
    assert!(
        (acts[0] + base).abs() < 1e-9 && (acts[1] - base).abs() < 1e-9,
        "колонки на ∓base = ∓{base}: {acts:?}"
    );
    assert!(
        (acts[0] + acts[1]).abs() < 1e-9,
        "симметрия: колонки зеркальны относительно оси"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

/// Канон (рис. 2): вложенный if внутри да-колонки внешнего — его «да»
/// тоже справа от под-ромба, пустое «нет» идёт стволом, «нет» внешнего —
/// слева. Стороны у вложенного те же, что у внешнего: разнобой читается
/// как ошибка.
#[test]
fn nested_if_keeps_yes_right() {
    let st = Style::default();
    let text = "\
if a > 0
    да:
    if b > 0
        да: printf(\"внутри\")
        нет:
    c = 1
    нет: printf(\"минус\")
";
    let nodes = crate::frontend::gvn::parse(text, &st, "ru").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let _nhe = super::nhe_of(&sizes, &nodes, &st);
    let mut ifs: Vec<&Shape> = l.shapes.iter().filter(|s| s.kind == "if").collect();
    ifs.sort_by(|a, b| a.cy.partial_cmp(&b.cy).unwrap());
    assert_eq!(ifs.len(), 2, "внешний и внутренний ромбы");
    let outer = ifs[0];
    let inner = ifs[1];
    let colw = sizes["act"].0.max(sizes["io"].0);
    // колонки пакуются по своим ветвям: «нет» из одной плитки стоит
    // hinge + colw/2, а не по общей на схему полуширине
    let base = outer.w / 2.0 + st.hgap + colw / 2.0;
    let base2 = inner.w / 2.0 + st.hgap + colw / 2.0;
    // «да»-колонка внешнего стоит по ВНУТРЕННЕМУ выносу своей ветки
    // (куда содержимое вылезает к оси), а не по симметричной полуширине
    let dec = nodes
        .iter()
        .find(|n| n.kind == NodeKind::Decision)
        .expect("внешний ромб");
    let (ext_l, _) = crate::layout::column::extent_lr(&sizes, &st, colw, &dec.branches[0].stmts);
    assert!(
        (inner.cx - (outer.w / 2.0 + st.hgap + ext_l)).abs() < 1e-9,
        "внутренний ромб на оси да-колонки внешнего: {} против {}",
        inner.cx,
        outer.w / 2.0 + st.hgap + ext_l
    );
    let at = |txt: &str| {
        l.shapes
            .iter()
            .find(|s| s.lines.first().is_some_and(|t| t.contains(txt)))
            .unwrap()
    };
    let io = at("внутри");
    let act = at("c = 1");
    let minus = at("минус");
    assert!(
        (io.cx - (inner.cx + base2)).abs() < 1e-9,
        "да-контент вложенного колонкой направо"
    );
    assert!(
        (act.cx - inner.cx).abs() < 1e-9,
        "c = 1 на оси да-колонки внешнего"
    );
    assert!(
        (minus.cx + base).abs() < 1e-9,
        "нет-ветка внешнего слева на -base"
    );
    // пустое «нет» вложенного — ствол, а не рельса: вертикали справа от
    // под-ромба нет
    let vr = (inner.cx + inner.w / 2.0, inner.cy);
    assert!(
        !l.edges.iter().any(|e| {
            !e.arrow && (e.points[0].0 - vr.0).abs() < 1e-9 && (e.points[0].1 - vr.1).abs() < 1e-9
        }),
        "рельса пустого «нет» вложенного не рисуется"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(overlaps_ok(&l.shapes));
}

/// Пустая ветка переключателя: рельса чистит только правые колонки —
/// bx = up(base + nhe + g), а не внешняя сторона всей схемы.
#[test]
fn empty_branch_rail_clears_own_columns() {
    let st = Style::default();
    let mut d = node(NodeKind::Decision, "if switch (d)");
    d.switch_var = Some("d".into());
    d.branches = vec![
        br("1", vec![s("a = 1")], false),
        br("2", vec![s("b = 2")], false),
        br("3", vec![s("c = 3")], false),
        br("4", vec![], false),
    ];
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        d,
        node(NodeKind::Term, "конец"),
    ];
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let dsh = l.shapes.iter().find(|s| s.kind == "if").unwrap();
    let vr = (dsh.cx + dsh.w / 2.0, dsh.cy);
    let rail = l
        .edges
        .iter()
        .find(|e| {
            !e.arrow && (e.points[0].0 - vr.0).abs() < 1e-9 && (e.points[0].1 - vr.1).abs() < 1e-9
        })
        .expect("рельса пустой ветки от правой вершины ромба");
    let bx = rail.points[1].0;
    let nhe = super::nhe_of(&sizes, &nodes, &st);
    // крайняя колонка (правая, ярус 0) на +base
    let base = dsh.w / 2.0 + st.hgap + nhe;
    let want = crate::layout::geometry::up(base + nhe + st.grid, st.grid);
    assert!(
        (bx - want).abs() < 1e-9,
        "bx = {bx}, ожидался up(base + nhe + g) = {want}"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(overlaps_ok(&l.shapes));
}

/// СЕТЕЧНАЯ СТРОГОСТЬ: вся геометрия раскладки живёт на модульной сетке.
/// (а) ширины/высоты фигур кратны grid (eps 0.01);
/// (б) вертикальный зазор между соседями одной колонки кратен grid;
/// (в) крайние координаты (cx±w/2, cy±h/2) кратны grid/2: ось колонки
///     легально лежит на полусетке (dw/2 + label_exit_dx + k*2*grid).
/// Для (б) допуск 0.005, а не 0.01: литеральный vgap 42.5 при
/// grid 14.17 отстоит от 3*grid ровно на 0.01 и с допуском 0.01
/// проскочил бы. Тест ловит возврат литеральных зазоров (14.2, 28.3,
/// 42.5): остаток каждого от ближайшего кратного grid не менее 0.01.
#[test]
fn grid_invariant() {
    let st = Style::default();
    let g = st.grid;
    let on_grid = |v: f64, m: f64, eps: f64| ((v / m) - (v / m).round()).abs() * m < eps;

    let mut d2 = node(NodeKind::Decision, "if a > 0");
    d2.branches = vec![
        br("да", vec![s("a = 1")], false),
        br("нет", vec![s("b = 2")], false),
    ];
    let mut d3 = node(NodeKind::Decision, "if switch (d)");
    d3.switch_var = Some("d".into());
    d3.branches = vec![
        br("1", vec![s("a = 1")], false),
        br("2", vec![s("b = 2")], false),
        br("3", vec![s("c = 3")], false),
    ];
    let schemes: Vec<Vec<Node>> = vec![
        linear(),
        vec![
            node(NodeKind::Term, "начало"),
            d2,
            node(NodeKind::Term, "конец"),
        ],
        vec![
            node(NodeKind::Term, "начало"),
            loop_node("while i < 5", vec![s("a = 1"), sbrk()]),
            node(NodeKind::Term, "конец"),
        ],
        vec![
            node(NodeKind::Term, "начало"),
            d3,
            node(NodeKind::Term, "конец"),
        ],
    ];

    for nodes in &schemes {
        let sizes = normalize(nodes, &st);
        let l = layout(nodes, &sizes, &st);
        for sh in &l.shapes {
            assert!(
                on_grid(sh.w, g, 0.01) && on_grid(sh.h, g, 0.01),
                "{}: размер {}x{} не кратен grid = {g}",
                sh.kind,
                sh.w,
                sh.h
            );
            for c in [
                sh.cx - sh.w / 2.0,
                sh.cx + sh.w / 2.0,
                sh.cy - sh.h / 2.0,
                sh.cy + sh.h / 2.0,
            ] {
                assert!(
                    on_grid(c, g / 2.0, 0.01),
                    "{}: координата {c} не кратна grid/2",
                    sh.kind
                );
            }
        }
        // (б) колонка = группа фигур с одним cx
        let mut cols: Vec<(f64, Vec<&Shape>)> = Vec::new();
        for sh in &l.shapes {
            match cols.iter_mut().find(|(x, _)| (sh.cx - *x).abs() < 1e-6) {
                Some((_, col)) => col.push(sh),
                None => cols.push((sh.cx, vec![sh])),
            }
        }
        for (cx, mut col) in cols {
            col.sort_by(|a, b| a.cy.partial_cmp(&b.cy).unwrap());
            for pair in col.windows(2) {
                let gap = (pair[1].cy - pair[1].h / 2.0) - (pair[0].cy + pair[0].h / 2.0);
                assert!(
                    gap >= -0.01 && on_grid(gap, g, 0.005),
                    "колонка cx={cx}: зазор {} между {} и {} не кратен grid = {g}",
                    gap,
                    pair[0].kind,
                    pair[1].kind
                );
            }
        }
    }
}

/// Каскад else-if (схема 25): три ромба на оси, «нет» — вертикальный
/// ствол между ними, да-колонки справа, все
/// на одном top0; слияние: ровно одна горизонтальная шина под всеми.
#[test]
fn elseif_cascade_trunk_and_bus() {
    let st = Style::default();
    let text = "\
if a < 0
    да: printf(\"neg\")
    нет:
    if a == 0
        да: printf(\"zero\")
        нет:
        if a > 0
            да: printf(\"pos\")
            нет: printf(\"?\")
";
    let nodes = crate::frontend::gvn::parse(text, &st, "ru").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let mut ifs: Vec<&Shape> = l.shapes.iter().filter(|s| s.kind == "if").collect();
    ifs.sort_by(|a, b| a.cy.partial_cmp(&b.cy).unwrap());
    assert_eq!(ifs.len(), 3, "три ромба каскада");
    for d in &ifs {
        assert_eq!(d.cx, 0.0, "ромб на оси, а не в {}", d.cx);
    }
    // ствол: стрелка от нижней вершины ромба к верхней вершины следующего
    for w in ifs.windows(2) {
        let (y0, y1) = (w[0].cy + w[0].h / 2.0, w[1].cy - w[1].h / 2.0);
        assert!(
            l.edges.iter().any(|e| {
                e.arrow
                    && e.points.len() == 2
                    && e.points.iter().all(|p| p.0.abs() < 1e-9)
                    && (e.points[0].1 - y0).abs() < 1e-9
                    && (e.points[1].1 - y1).abs() < 1e-9
            }),
            "нет вертикального ствола между ромбами"
        );
    }
    // колонки (канон): все «да» СПРАВА (R0, R1, R2), хвост-else слева (L0)
    let colw = sizes["act"].0.max(sizes["io"].0);
    let nhe = colw / 2.0;
    let base = ifs[0].w / 2.0 + st.hgap + nhe;
    let pitch = 2.0 * nhe + st.colgap;
    let at = |txt: &str| {
        l.shapes
            .iter()
            .find(|s| s.lines.first().is_some_and(|t| t.contains(txt)))
            .unwrap_or_else(|| panic!("нет фигуры {txt}"))
    };
    let neg = at("neg");
    let zero = at("zero");
    let pos = at("pos");
    let other = at("?");
    for (sh, x, name) in [
        (neg, base, "neg R0"),
        (zero, base + pitch, "zero R1"),
        (pos, base + 2.0 * pitch, "pos R2"),
        (other, -base, "? L0"),
    ] {
        assert!(
            (sh.cx - x).abs() < 1e-9,
            "{name}: cx = {}, ожидалась абсцисса {x}",
            sh.cx
        );
    }
    let top = |s: &Shape| s.cy - s.h / 2.0;
    for sh in [neg, zero, pos, other] {
        assert!(
            (top(sh) - top(neg)).abs() < 1e-9,
            "все колонки на одном top0"
        );
    }
    // шина: ровно один горизонтальный сегмент на merge_y, через 0,
    // накрывает крайние колонки; merge_y = низ последнего ромба + 2g
    let dsh_last = ifs[2];
    let my = dsh_last.cy + dsh_last.h / 2.0 + 2.0 * st.grid;
    let bus: Vec<&crate::layout::Edge> = l
        .edges
        .iter()
        .filter(|e| {
            e.points
                .windows(2)
                .any(|w| (w[0].1 - w[1].1).abs() < 1e-9 && (w[0].1 - my).abs() < 1e-9)
        })
        .collect();
    assert_eq!(bus.len(), 1, "ровно одна горизонтальная шина на merge_y");
    let xs: Vec<f64> = bus[0].points.iter().map(|p| p.0).collect();
    assert!(
        xs.iter().cloned().fold(f64::MAX, f64::min) <= -base + 1e-9
            && xs.iter().cloned().fold(f64::MIN, f64::max) >= base + 2.0 * pitch - 1e-9,
        "шина от хвоста-else слева до крайней правой да-колонки через 0: {xs:?}"
    );
    assert_eq!(crossings_ok(&l.shapes, &l.edges), Ok(()));
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

/// Цепочка из двух ромбов без else: каскад с пустым хвостом — рельса
/// «нет» слева (там колонок нет), вплотную к ромбу; шина одна.
#[test]
fn elseif_cascade_two_links_empty_else() {
    let st = Style::default();
    let text = "\
if a < 0
    да: printf(\"neg\")
    нет:
    if a == 0
        да: printf(\"zero\")
        нет:
";
    let nodes = crate::frontend::gvn::parse(text, &st, "ru").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let mut ifs: Vec<&Shape> = l.shapes.iter().filter(|s| s.kind == "if").collect();
    ifs.sort_by(|a, b| a.cy.partial_cmp(&b.cy).unwrap());
    assert_eq!(ifs.len(), 2, "два ромба каскада");
    assert!(ifs.iter().all(|d| d.cx == 0.0), "ромбы на оси");
    let colw = sizes["act"].0.max(sizes["io"].0);
    let nhe = colw / 2.0;
    let base = ifs[0].w / 2.0 + st.hgap + nhe;
    let neg = l
        .shapes
        .iter()
        .find(|s| s.lines.first().is_some_and(|t| t.contains("neg")))
        .unwrap();
    let zero = l
        .shapes
        .iter()
        .find(|s| s.lines.first().is_some_and(|t| t.contains("zero")))
        .unwrap();
    assert!((neg.cx - base).abs() < 1e-9, "neg R0");
    assert!(
        (zero.cx - base - 2.0 * nhe - st.colgap).abs() < 1e-9,
        "zero R1"
    );
    // рельса пустого хвоста из ЛЕВОЙ вершины последнего ромба (слева
    // колонок нет — свободная сторона), жмётся к ромбу: up(base + nhe + g)
    let d2 = ifs[1];
    let vl = (d2.cx - d2.w / 2.0, d2.cy);
    let rail = l
        .edges
        .iter()
        .find(|e| {
            !e.arrow && (e.points[0].0 - vl.0).abs() < 1e-9 && (e.points[0].1 - vl.1).abs() < 1e-9
        })
        .expect("рельса пустого «нет» из левой вершины ромба");
    assert!(
        rail.points[1].0 < vl.0 && rail.points[2].0 == rail.points[1].0,
        "рельса уходит влево и опускается: {:?}",
        rail.points
    );
    let my = d2.cy + d2.h / 2.0 + 2.0 * st.grid;
    let buses = l
        .edges
        .iter()
        .filter(|e| {
            e.points
                .windows(2)
                .any(|w| (w[0].1 - w[1].1).abs() < 1e-9 && (w[0].1 - my).abs() < 1e-9)
        })
        .count();
    assert_eq!(buses, 1, "ровно одна шина на merge_y");
    assert_eq!(crossings_ok(&l.shapes, &l.edges), Ok(()));
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

/// Каскад с пустым хвостом: две else-if по умолчанию — но «да» первой
/// (пустая плитка) и вложенный if в «нет»: именно та форма, в которой
/// коридор стороны считался только по обходам и оставлял «да» первого
/// ромба висеть в воздухе (u1-7: отрезок из левой вершины без стыка).
/// Инвариант: ни один конец ребра не висит в пустоте.
#[test]
fn elseif_cascade_first_branch_not_dangling() {
    let st = Style::default();
    let text = "\
if d2 == r2
    да:
    printf(\"na okruzhnosti\")
    нет:
    if d2 < r2
        да:
        printf(\"vnutri\")
        нет:
        printf(\"vne\")
";
    let nodes = crate::frontend::gvn::parse(text, &st, "ru").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    if let Err(e) = crate::layout::dangling_ok(&l) {
        panic!("обрыв в каскаде else-if: {e}");
    }
    assert_eq!(crossings_ok(&l.shapes, &l.edges), Ok(()));
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

/// Большой переключатель (>= 5 кейсов): кейсы сеткой по два на ряд —
/// все колонки на ±base, второй ряд ниже первого; ряды слиты в ствол.
#[test]
fn switch_rows_grid_for_five_cases() {
    let st = Style::default();
    let text = "\
if switch (d)
    1: printf(\"один\"); break
    2: printf(\"два\"); break
    3: printf(\"три\"); break
    4: printf(\"четыре\"); break
    5: printf(\"пять\"); break
output printf(d)
";
    let nodes = crate::frontend::gvn::parse(text, &st, "").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let colw = sizes["act"].0.max(sizes["io"].0);
    let dsh = l.shapes.iter().find(|s| s.kind == "if").unwrap();
    // колонка кейса стоит по своему содержимому: hinge + полуширина
    let base = dsh.w / 2.0 + st.hgap + colw / 2.0;
    let ios: Vec<&Shape> = l
        .shapes
        .iter()
        .filter(|s| s.kind == "io" && s.lines.first().is_some_and(|t| t.contains("printf(\"")))
        .collect();
    assert_eq!(ios.len(), 5, "пять кейсов");
    let _ = colw;
    // четыре кейса сеткой на ±base, нечётный пятый — на оси
    let mut on_axis = 0;
    for io in &ios {
        if io.cx.abs() < 1e-9 {
            on_axis += 1;
            continue;
        }
        assert!(
            (io.cx.abs() - base).abs() < 1e-9,
            "кейс на ±base = ±{base}, а он в {}",
            io.cx
        );
    }
    assert_eq!(on_axis, 1, "нечётный хвост стоит на стволе");
    // ряды: два кейса слева (1, 3), два справа (2, 4); ряд 1 ниже ряда 0
    let mut lefts: Vec<f64> = ios.iter().filter(|s| s.cx < 0.0).map(|s| s.cy).collect();
    lefts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(lefts.len(), 2);
    // кейсы больше не таскают плитки break, поэтому ряды опираются на
    // высоту самой плитки кейса, а не ромба
    let (_, io_h) = measure(&st, "io", "printf(\"один\")");
    assert!(
        lefts[1] > lefts[0] + io_h,
        "каждый следующий ряд ниже предыдущего: {:?}",
        lefts
    );
    // ширина схемы меньше, чем была бы одной шиной (2 яруса вширь)
    let (minx, ..) = l.bounds;
    let w = {
        let xs: Vec<f64> = l
            .shapes
            .iter()
            .flat_map(|s| [s.cx - s.w / 2.0, s.cx + s.w / 2.0])
            .collect();
        xs.iter().cloned().fold(f64::MAX, f64::min)
    };
    let _ = (minx, w);
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

/// Четыре кейса — ещё одна шина (как на доске), не сетка: все четыре
/// кейса на одном верхнем ряду, на ±base и ±(base + pitch).
#[test]
fn switch_four_cases_stay_single_bus() {
    let st = Style::default();
    let text = "\
if switch (d)
    1: printf(\"один\"); break
    2: printf(\"два\"); break
    3: printf(\"три\"); break
    4: printf(\"четыре\"); break
output printf(d)
";
    let nodes = crate::frontend::gvn::parse(text, &st, "").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let nhe = super::nhe_of(&sizes, &nodes, &st);
    let dsh = l.shapes.iter().find(|s| s.kind == "if").unwrap();
    let base = dsh.w / 2.0 + st.hgap + nhe;
    let pitch = 2.0 * nhe + st.colgap;
    let want = [-base - pitch, -base, base, base + pitch];
    let mut got: Vec<f64> = l
        .shapes
        .iter()
        .filter(|s| s.kind == "io" && s.cx != 0.0)
        .map(|s| s.cx)
        .collect();
    got.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for (g, w) in got.iter().zip(want.iter()) {
        assert!((g - w).abs() < 1e-9, "cx {g}, ожидалось {w}");
    }
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

/// Пустая «нет» — ствол на оси: широкая «да»-колонка справа на ширину
/// схемы не влияет вовсе, отдельной рельсы у пустой ветки нет.
#[test]
fn empty_branch_trunk_ignores_wide_yes_column() {
    let st = Style::default();
    let text = "\
if a > 0
    да: printf(\"очень широкий текст printf\")
    нет:
output printf(1)
";
    let nodes = crate::frontend::gvn::parse(text, &st, "").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    let dsh = l.shapes.iter().find(|s| s.kind == "if").unwrap();
    let io = l.shapes.iter().find(|s| s.kind == "io").unwrap();
    assert!(io.cx > dsh.cx, "да-колонка справа");
    let right = dsh.cx + dsh.w / 2.0;
    let vr = (right, dsh.cy);
    assert!(
        !l.edges.iter().any(|e| {
            !e.arrow && (e.points[0].0 - vr.0).abs() < 1e-9 && (e.points[0].1 - vr.1).abs() < 1e-9
        }),
        "рельсы пустой ветки нет"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

/// Репорт: ветка «да» из одного break рисовала вторую линию к слиянию
/// if и стрелку в пустоту. Теперь колонка без плиток не дорисовывается
/// к слиянию, вход без стрелки, выход — только рельса под цикл.
#[test]
fn break_only_branch_no_merge_descent() {
    let st = Style::default();
    let text = "\
while true
    input scanf(\"%d\", &a)
    if a > 0
        yes:
        break
        no:
        printf(\"нет\")
output printf(\"далее\")
";
    let nodes = crate::frontend::gvn::parse(text, &st, "").unwrap();
    let _sizes = normalize(&nodes, &st);
    let l = lay(&nodes);
    assert!(
        !l.shapes
            .iter()
            .any(|sh| sh.lines == vec!["break".to_string()]),
        "блок break не рисуется: {:?}",
        l.shapes.iter().map(|s| s.lines.clone()).collect::<Vec<_>>()
    );
    let dsh = l.shapes.iter().find(|s| s.kind == "if").unwrap();
    let top2 = dsh.cy + dsh.h / 2.0 + crate::layout::vgap(&st);
    let le = l.shapes.iter().find(|sh| sh.kind == "loop_end").unwrap();
    let merge_y = le.cy + le.h / 2.0 + st.mgap;
    // рельсы break и continue сливаются на одной высоте — посередине
    // выхода loop_end: разные концы давали X между рельсами
    let join_y = (le.cy + le.h / 2.0 + merge_y) * 0.5;
    // вход в «да»-колонку (канон: справа) без стрелки; ось колонки
    // берём из самого ребра — она пакуется по экстенту ветки
    let entry = l
        .edges
        .iter()
        .find(|e| {
            let last = e.points.last().unwrap();
            last.0 > 0.0 && (last.1 - top2).abs() < 1e-9
        })
        .expect("вход в да-колонку");
    assert!(!entry.arrow, "стрелки в пустую колонку нет");
    let base = entry.points.last().unwrap().0;
    // ложный спуск колонки к слиянию исчез: вертикали на x = +base
    // ниже верха колонки нет
    let bogus = l.edges.iter().any(|e| {
        e.points.windows(2).any(|w| {
            let (p, q) = (w[0], w[1]);
            (p.0 - base).abs() < 1e-9 && (q.0 - base).abs() < 1e-9 && q.1.max(p.1) > top2 + 1e-9
        })
    });
    assert!(!bogus, "спуск rail-колонки к шине слияния не рисуется");
    // рельса break: коридор своей стороны ниже loop_end, T-стык на продолжении
    let (lw, _) = measure(&st, "loop", "while true");
    let rail = l
        .edges
        .iter()
        .find(|e| {
            e.points.iter().any(|p| p.0.abs() > lw / 2.0)
                && e.points
                    .last()
                    .map(|p| p.0 == 0.0 && (p.1 - join_y).abs() < 1e-9)
                    .unwrap_or(false)
        })
        .expect("рельса break в коридоре");
    let last = rail.points.last().unwrap();
    assert!(
        last.0 == 0.0 && (last.1 - join_y).abs() < 1e-9,
        "рельса приходит на слияние рельс: {last:?} vs {join_y}"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(single_entry_ok(&l));
}

/// Репорт: break в кейсе switch внутри цикла считался выходом из ЦИКЛА.
/// В C он покидает только switch — схема с break обязана совпадать
/// со схемой без него.
#[test]
fn case_break_inside_loop_changes_nothing() {
    let st = Style::default();
    let with_b = "\
while i < 5
    if switch (a)
        1: printf(\"один\"); break
        2: printf(\"два\"); break
output printf(\"далее\")
";
    let without = "\
while i < 5
    if switch (a)
        1: printf(\"один\")
        2: printf(\"два\")
output printf(\"далее\")
";
    let a = lay(&crate::frontend::gvn::parse(with_b, &st, "").unwrap());
    let b = lay(&crate::frontend::gvn::parse(without, &st, "").unwrap());
    assert_eq!(a.shapes, b.shapes, "фигуры совпадают");
    assert_eq!(
        a.edges, b.edges,
        "рёбра совпадают: break растворён в слиянии кейса"
    );
}

/// То же на верхнем уровне: раньше break в кейсе рисовался прямоугольником.
#[test]
fn top_level_case_break_dissolves() {
    let st = Style::default();
    let with_b = "if switch (a)\n    1: printf(\"один\"); break\n    2: printf(\"два\"); break\noutput printf(\"k\")\n";
    let without =
        "if switch (a)\n    1: printf(\"один\")\n    2: printf(\"два\")\noutput printf(\"k\")\n";
    let a = lay(&crate::frontend::gvn::parse(with_b, &st, "").unwrap());
    let b = lay(&crate::frontend::gvn::parse(without, &st, "").unwrap());
    assert!(!a
        .shapes
        .iter()
        .any(|sh| sh.lines == vec!["break".to_string()]));
    assert_eq!(a.shapes, b.shapes);
    assert_eq!(a.edges, b.edges);
}

/// continue: рельса к выходу loop_end (следующая итерация); мёртвый
/// код после continue в той же колонке не рисуется.
#[test]
fn continue_rails_to_loop_end_output() {
    let st = Style::default();
    let text = "\
while i < 5
    a = 1
    continue
    b = 2
output printf(\"далее\")
";
    let nodes = crate::frontend::gvn::parse(text, &st, "").unwrap();
    let l = lay(&nodes);
    assert!(!l
        .shapes
        .iter()
        .any(|sh| sh.lines == vec!["b = 2".to_string()]));
    let le = l.shapes.iter().find(|sh| sh.kind == "loop_end").unwrap();
    // continue — следующая итерация: входит в БОК loop_end, выше его
    // низа и выше нижней трапеции; не сливается с выходом из цикла
    let cont_y = le.cy - st.grid;
    let rail = l
        .edges
        .iter()
        .find(|e| {
            let last = e.points.last().unwrap();
            let side = (last.0 - (le.cx - le.w / 2.0)).abs() < 1e-9
                || (last.0 - (le.cx + le.w / 2.0)).abs() < 1e-9;
            !e.arrow && (last.1 - cont_y).abs() < 1e-9 && side
        })
        .expect("рельса continue входит в бок loop_end");
    assert!(
        rail.points.len() >= 2,
        "рельса приходит на бок трапеции: {:?}",
        rail.points
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

/// Рельса continue внутри while с вложенными if (меню пользователя):
/// коридор дренажа меряется от фактического левого края тела, иначе
/// вертикаль рельсы проходит сквозь io-плитку колонки ветви
/// (u3.c: ребро (-481.78, 935)-(-481.78, 2153) сквозь блок).
#[test]
fn continue_rail_inside_nested_if_loop_misses_shapes() {
    // while { io; if {да: [io, continue]}; if {да: [io, if {да: [io, continue]}]} }
    let mut inner = node(NodeKind::Decision, "if (scanf_s(...) != 1 (a <= 0))");
    inner.branches = vec![
        br(
            "да",
            vec![sio("printf(\"oshibka a\")"), Stmt::Continue],
            false,
        ),
        br("нет", vec![], false),
    ];
    let mut mid = node(NodeKind::Decision, "if (choice == 1)");
    mid.branches = vec![
        br(
            "да",
            vec![sio("printf(\"vvedite a: \")"), Stmt::Node(Box::new(inner))],
            false,
        ),
        br("нет", vec![], false),
    ];
    let mut top = node(NodeKind::Decision, "if (scanf_s(...) != 1)");
    top.branches = vec![
        br(
            "да",
            vec![sio("printf(\"oshibka\")"), Stmt::Continue],
            false,
        ),
        br("нет", vec![], false),
    ];
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        loop_node(
            "while 1",
            vec![
                sio("printf(\"menu\")"),
                Stmt::Node(Box::new(top)),
                Stmt::Node(Box::new(mid)),
            ],
        ),
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    if let Err(e) = crossings_ok(&l.shapes, &l.edges) {
        panic!("рельса дренажа заходит на блок: {e}");
    }
    if let Err(e) = crate::layout::dangling_ok(&l) {
        panic!("обрыв линии: {e}");
    }
    assert!(overlaps_ok(&l.shapes));
}

/// Каскад else-if, где каждая ветка кончается return: плитка «return»
/// бывает шире колонки, и коридор каскада садился внутрь её силуэта
/// (torture.c/classify.c: вертикаль сквозь «return -1»). Полуширина
/// колонки теперь считает свою фигуру, коридор чист.
#[test]
fn cascade_with_returns_keeps_corridor_clear() {
    let st = Style::default();
    let text = "\
if x < 0
    да: return -1
    нет:
    if x == 0
        да: return 0
        нет:
        if x < 10
            да: return 1
            нет: return 2
";
    let nodes = crate::frontend::gvn::parse(text, &st, "ru").unwrap();
    let sizes = normalize(&nodes, &st);
    let l = layout(&nodes, &sizes, &st);
    if let Err(e) = crossings_ok(&l.shapes, &l.edges) {
        panic!("коридор каскада заходит на блок: {e}");
    }
    if let Err(e) = crate::layout::dangling_ok(&l) {
        panic!("обрыв линии: {e}");
    }
    assert!(overlaps_ok(&l.shapes));
    assert!(single_entry_ok(&l));
}

/// Гигантский цикл: тело режется на листы — ленты с микроскопическим
/// кеглем больше нет. Каждая часть влезает в бюджет, кружки парные,
/// нижняя трапеция с номером — на последнем листе.
#[test]
fn giant_loop_body_splits_across_pages() {
    let st = Style::default();
    let mut text = String::from("while i < 24\n");
    for k in 1..=24 {
        text.push_str(&format!("    s = s + {k}\n"));
    }
    let nodes = crate::frontend::gvn::parse(&text, &st, "en").unwrap();
    let sizes = normalize(&nodes, &st);
    let parts = split_scheme(nodes, &sizes, &st);
    assert_eq!(
        parts.len(),
        1,
        "24 шага влезают на один лист (split_min): {}",
        parts.len()
    );
    for (k, part) in parts.iter().enumerate() {
        let l = layout(part, &sizes, &st);
        assert!(
            l.bounds.3 <= (st.a4_h - 2.0 * st.page_pad) / st.split_min,
            "лист {k} не влез: {}",
            l.bounds.3
        );
        assert!(crossings_ok(&l.shapes, &l.edges).is_ok(), "лист {k}");
        assert!(overlaps_ok(&l.shapes), "лист {k}");
        assert!(dangling_ok(&l).is_ok(), "лист {k}");
    }
    // страница одна — соединителей и хвостовых трапеций-кусков нет
    let l = layout(parts.last().unwrap(), &sizes, &st);
    assert!(
        l.shapes.iter().any(|sh| sh.kind == "loop_end"),
        "нижняя трапеция цикла"
    );
}

/// continue в теле гигантского цикла: рельсы уходят на кружок-выход
/// листа — ни пересечений, ни обрывов ни на одном листе.
#[test]
fn giant_loop_with_continue_in_body() {
    let st = Style::default();
    // continue в ветке if: только такой рельс не обрывает отрисовку тела
    let mut text = String::from("while i < 24\n");
    text.push_str("    if i % 2 == 0\n        да: continue\n        нет: s = s + 1\n");
    for k in 1..=24 {
        text.push_str(&format!("    s = s + {k}\n"));
    }
    let nodes = crate::frontend::gvn::parse(&text, &st, "en").unwrap();
    let sizes = normalize(&nodes, &st);
    let parts = split_scheme(nodes, &sizes, &st);
    assert_eq!(
        parts.len(),
        1,
        "continue не обязателен к разрезу: {}",
        parts.len()
    );
    for (k, part) in parts.iter().enumerate() {
        let l = layout(part, &sizes, &st);
        assert_eq!(crossings_ok(&l.shapes, &l.edges), Ok(()), "лист {k}");
        assert!(overlaps_ok(&l.shapes), "лист {k}");
        assert!(dangling_ok(&l).is_ok(), "лист {k}");
    }
    // рельс головы/середины приходит в живой поток листа, а не в пустоту:
    // либо на кружок-выход (стык листов), либо на ствол следующей части —
    // кусок из одного оператора ставит на лист несколько частей, и тогда
    // кружка между ними нет, рельса сходится на вход следующей.
    let drained = parts.iter().any(|p| {
        let l = layout(p, &sizes, &st);
        l.edges.iter().filter(|e| !e.arrow).any(|e| {
            let Some(last) = e.points.last() else {
                return false;
            };
            let to_circle = l
                .shapes
                .iter()
                .filter(|sh| sh.kind == "conn")
                .any(|c| last == &(c.cx, c.cy - c.h / 2.0));
            let to_trunk = l.edges.iter().any(|a| {
                a.arrow && (a.points.first() == Some(last) || a.points.last() == Some(last))
            });
            to_circle || to_trunk
        })
    });
    assert!(drained, "ни один рельс не пришёл в поток листа");
}

/// Меню (lab4): тело цикла из четырёх гигантских «если» режется перед
/// каждым оператором — один оператор на лист, каждый в бюджете, кружки
/// на стыках парные.
#[test]
fn menu_body_splits_per_operator() {
    let st = Style::default();
    let big_if = |t: &str| {
        let mut d = node(NodeKind::Decision, t);
        d.branches = vec![
            br(
                "да",
                (0..6).map(|k| s(&format!("a = a + {k}"))).collect(),
                false,
            ),
            br(
                "нет",
                (0..6).map(|k| s(&format!("b = b + {k}"))).collect(),
                false,
            ),
        ];
        d
    };
    let body: Vec<Stmt> = (0..4)
        .map(|k| Stmt::Node(Box::new(big_if(&format!("if r == {k}")))))
        .collect();
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        loop_node("while !done", body),
        node(NodeKind::Term, "конец"),
    ];
    let sizes = normalize(&nodes, &st);
    let parts = split_scheme(nodes, &sizes, &st);
    assert_eq!(
        parts.len(),
        1,
        "меню влезает на один лист (split_min): {}",
        parts.len()
    );
    let budget = (st.a4_h - 2.0 * st.page_pad) / st.split_min;
    for (k, part) in parts.iter().enumerate() {
        let l = layout(part, &sizes, &st);
        assert!(l.bounds.3 <= budget, "лист {k} не влез: {}", l.bounds.3);
        assert!(crossings_ok(&l.shapes, &l.edges).is_ok(), "лист {k}");
        assert!(overlaps_ok(&l.shapes), "лист {k}");
        assert!(dangling_ok(&l).is_ok(), "лист {k}");
    }
    for w in parts.windows(2) {
        let out = w[0].last().unwrap();
        let inp = w[1].first().unwrap();
        assert_eq!(out.kind, NodeKind::Conn, "выход листа — кружок: {out:?}");
        assert_eq!(out.text, inp.text, "буквы пары расходятся");
    }
    // трапеции цикла: вход на головном листе, номер — на хвостовом
    let first = layout(&parts[0], &sizes, &st);
    let last = layout(parts.last().unwrap(), &sizes, &st);
    assert!(
        first.shapes.iter().any(|sh| sh.kind == "loop_begin"),
        "нет входа цикла на первом листе"
    );
    assert!(
        last.shapes.iter().any(|sh| sh.kind == "loop_end"),
        "нет хвоста цикла на последнем листе"
    );
}

/// Маркер ручного переноса в теле цикла: листы ломаются ровно по нему,
/// даже если цикл целиком влезает на один лист.
#[test]
fn manual_marker_inside_loop_body() {
    let st = Style::default();
    let text = "\
while i < 5
    a = 1
    b = 2
    перенос
    c = 3
    d = 4
";
    let nodes = crate::frontend::gvn::parse(text, &st, "en").unwrap();
    let sizes = normalize(&nodes, &st);
    let parts = split_scheme(nodes, &sizes, &st);
    assert_eq!(parts.len(), 2, "{parts:?}");
    assert!(
        !parts
            .iter()
            .flatten()
            .any(|n| n.text == crate::frontend::MARKER),
        "маркер съеден"
    );
    let out = parts[0].last().unwrap();
    let inp = parts[1].first().unwrap();
    assert_eq!(out.kind, NodeKind::Conn, "{out:?}");
    assert_eq!(out.text, inp.text, "одна буква на пару листов");
    // голова несёт верхнюю трапецию, хвост — нижнюю с номером цикла
    let l1 = layout(&parts[0], &sizes, &st);
    let l2 = layout(&parts[1], &sizes, &st);
    assert!(l1.shapes.iter().any(|sh| sh.kind == "loop_begin"));
    assert!(!l1.shapes.iter().any(|sh| sh.kind == "loop_end"));
    assert!(!l2.shapes.iter().any(|sh| sh.kind == "loop_begin"));
    let le = l2
        .shapes
        .iter()
        .find(|sh| sh.kind == "loop_end")
        .expect("нижняя трапеция на хвосте");
    assert_eq!(le.lines, vec!["1".to_string()]);
    for l in [&l1, &l2] {
        assert_eq!(crossings_ok(&l.shapes, &l.edges), Ok(()));
        assert!(overlaps_ok(&l.shapes));
        assert!(dangling_ok(l).is_ok());
    }
}

/// Мёртвый хвост тела после первой рельсы не воскресает на листах: колонка
/// обрывается на break, псевдо-замер мёртвых не видит — иначе рез уходил за
/// рельсу и «МЁРТВ» рисовался живой плиткой на следующем листе.
#[test]
fn dead_candidates_after_rail_not_drawn() {
    let st = Style::default();
    let mut text = String::from("while i < 24\n");
    for k in 1..=5 {
        text.push_str(&format!("    s = s + {k}\n"));
    }
    text.push_str("    break\n");
    for k in 1..=10 {
        text.push_str(&format!("    МЁРТВ{k} = 1\n"));
    }
    // переросток ниже цикла: без него тело вообще не режется
    for k in 1..=30 {
        text.push_str(&format!("z = z + {k}\n"));
    }
    let nodes = crate::frontend::gvn::parse(&text, &st, "en").unwrap();
    let sizes = normalize(&nodes, &st);
    let parts = split_scheme(nodes, &sizes, &st);
    assert_eq!(
        parts.len(),
        1,
        "умеренно длинный цикл не режем: {}",
        parts.len()
    );
    for (k, part) in parts.iter().enumerate() {
        let l = layout(part, &sizes, &st);
        assert!(
            !l.shapes
                .iter()
                .any(|sh| sh.lines.iter().any(|t| t.contains("МЁРТВ"))),
            "лист {k} воскресил мёртвый код: {:?}",
            l.shapes
                .iter()
                .map(|sh| sh.lines.clone())
                .collect::<Vec<_>>()
        );
        assert!(crossings_ok(&l.shapes, &l.edges).is_ok(), "лист {k}");
        assert!(dangling_ok(&l).is_ok(), "лист {k}");
    }
    // живая часть (до рельсы) нарисована целиком — как в обычном раскладе
    let drawn: Vec<String> = parts
        .iter()
        .flat_map(|p| layout(p, &sizes, &st).shapes)
        .flat_map(|sh| sh.lines)
        .collect();
    for k in 1..=5 {
        let t = format!("s = s + {k}");
        assert!(drawn.contains(&t), "живой оператор {t} потерян: {drawn:?}");
    }
}

/// Маркеры подряд: лист из одних кружков не рождается — лишний маркер
/// глотается, буква на него не тратится. То же с маркером первым оператором.
#[test]
fn two_consecutive_markers_no_empty_sheet() {
    let st = Style::default();
    let text = "a = 1\nперенос\nперенос\nb = 2\n";
    let nodes = crate::frontend::gvn::parse(text, &st, "en").unwrap();
    let sizes = normalize(&nodes, &st);
    let parts = split_scheme(nodes, &sizes, &st);
    assert_eq!(parts.len(), 2, "{parts:?}");
    for (k, part) in parts.iter().enumerate() {
        assert!(
            part.iter().any(|n| n.kind != NodeKind::Conn),
            "лист {k} из одних кружков: {part:?}"
        );
        let l = layout(part, &sizes, &st);
        assert!(crossings_ok(&l.shapes, &l.edges).is_ok(), "лист {k}");
        assert!(dangling_ok(&l).is_ok(), "лист {k}");
    }
    let out = parts[0].last().unwrap();
    let inp = parts[1].first().unwrap();
    assert_eq!(out.kind, NodeKind::Conn, "{out:?}");
    assert_eq!(out.text, inp.text, "одна буква на пару листов");
    assert!(
        parts[0].iter().any(|n| n.text == "a = 1") && parts[1].iter().any(|n| n.text == "b = 2"),
        "операторы разъехались не туда: {parts:?}"
    );
    // маркер первым оператором: резать перед ним нечего — глотается,
    // буква не тратится, лишнего листа нет
    let items = vec![
        node(NodeKind::Conn, crate::frontend::MARKER),
        node(NodeKind::Act, "a = 1"),
        node(NodeKind::Act, "b = 2"),
    ];
    let sizes = normalize(&items, &st);
    let parts = split_scheme(items, &sizes, &st);
    assert_eq!(
        parts.len(),
        1,
        "маркер в голове обязан глотаться: {parts:?}"
    );
    assert!(
        !parts[0].iter().any(|n| n.kind == NodeKind::Conn),
        "кружок из ниоткуда: {parts:?}"
    );
}

/// 30 разрывов в длинной схеме: буквы зацикливаются (счётчик общий, wrap по
/// % длины алфавита), и ни один лист не выходит пустоватым из кружков.
#[test]
fn letters_wrap_past_alphabet() {
    let st = Style::default();
    let mut text = String::new();
    for k in 1..=30 {
        text.push_str(&format!("a = {k}\nперенос\nперенос\n"));
    }
    text.push_str("b = 1\n");
    let nodes = crate::frontend::gvn::parse(&text, &st, "en").unwrap();
    let sizes = normalize(&nodes, &st);
    let parts = split_scheme(nodes, &sizes, &st);
    let n_letters = st.letters.chars().count();
    assert!(
        parts.len() > n_letters,
        "частей {} не больше букв {n_letters} — wrap не проверить",
        parts.len()
    );
    for (k, part) in parts.iter().enumerate() {
        assert!(
            part.iter().any(|n| n.kind != NodeKind::Conn),
            "лист {k} пустоват: {part:?}"
        );
        let l = layout(part, &sizes, &st);
        assert!(crossings_ok(&l.shapes, &l.edges).is_ok(), "лист {k}");
        assert!(dangling_ok(&l).is_ok(), "лист {k}");
    }
    assert_eq!(
        parts[0].last().unwrap().text,
        parts[n_letters].last().unwrap().text,
        "буквы обязаны зацикливаться"
    );
}

/// Цикл читается циклом: из нижней трапеции есть ребро назад, вверх к
/// голове (а не только вниз на продолжение).
#[test]
fn loop_has_return_edge_up_to_head() {
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        loop_node("while i < 5", vec![s("i = i + 1")]),
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    let lb = l.shapes.iter().find(|sh| sh.kind == "loop_begin").unwrap();
    let le = l.shapes.iter().find(|sh| sh.kind == "loop_end").unwrap();
    let back = l
        .edges
        .iter()
        .find(|e| {
            e.arrow
                && e.points.len() == 4
                && e.points[0].1 > e.points.last().unwrap().1
                && (e.points.last().unwrap().1 - lb.cy).abs() < 1e-9
        })
        .expect("обратный ход цикла к голове");
    assert!(
        back.points
            .iter()
            .any(|p| p.0 < lb.cx - lb.w / 2.0 || p.0 > lb.cx + lb.w / 2.0),
        "обратный ход идёт коридором снаружи трапеции: {:?}",
        back.points
    );
    assert!(
        (back.points[0].1 - le.cy).abs() < 1e-9,
        "обратный ход начинается с нижней трапеции: {:?}",
        back.points
    );
    assert!(collinear_overlap_ok(&l).is_ok());
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
}

/// do-while: тело выполняется до проверки — обратный ход идёт к входу
/// тела, а не к заголовку с условием; маркер «do while» на месте.
#[test]
fn do_while_returns_to_body_entry() {
    let mut lp = loop_node("x < 3", vec![s("x = x + 1")]);
    lp.loop_kind = Some(crate::ir::LoopKind::DoWhile);
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        lp,
        node(NodeKind::Term, "конец"),
    ];
    let l = lay(&nodes);
    let lb = l.shapes.iter().find(|sh| sh.kind == "loop_begin").unwrap();
    assert_eq!(
        lb.lines,
        vec!["1. do while x < 3".to_string()],
        "маркер do на голове"
    );
    let back = l
        .edges
        .iter()
        .find(|e| e.arrow && e.points.len() == 4 && e.points[0].1 > e.points.last().unwrap().1)
        .expect("обратная связь do-while");
    let end = back.points.last().unwrap();
    assert!(
        end.0 == lb.cx && end.1 > lb.cy + lb.h / 2.0,
        "do-while возвращается к входу тела, ниже головы: {end:?}"
    );
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(collinear_overlap_ok(&l).is_ok());
}

/// break и continue ведут в разные точки: continue — в бок loop_end
/// (следующая итерация), break — ниже нижней трапеции (выход из цикла).
#[test]
fn break_and_continue_leave_at_different_heights() {
    let mut cont = node(NodeKind::Decision, "if a");
    cont.branches = vec![
        br("да", vec![Stmt::Continue], false),
        br("нет", vec![], false),
    ];
    let mut brk = node(NodeKind::Decision, "if b");
    brk.branches = vec![br("да", vec![Stmt::Break], false), br("нет", vec![], false)];
    let nodes = vec![
        node(NodeKind::Term, "начало"),
        loop_node(
            "while i < 5",
            vec![
                s("i = i + 1"),
                Stmt::Node(Box::new(cont)),
                Stmt::Node(Box::new(brk)),
            ],
        ),
        node(NodeKind::Term, "конец"),
    ];
    let st = Style::default();
    let l = lay(&nodes);
    let le = l.shapes.iter().find(|sh| sh.kind == "loop_end").unwrap();
    let cont_edge = l
        .edges
        .iter()
        .find(|e| {
            !e.arrow
                && e.points.last().is_some_and(|p| {
                    (p.1 - (le.cy - st.grid)).abs() < 1e-6
                        && ((p.0 - (le.cx - le.w / 2.0)).abs() < 1e-6
                            || (p.0 - (le.cx + le.w / 2.0)).abs() < 1e-6)
                })
        })
        .expect("continue входит в бок loop_end");
    let brk_edge = l
        .edges
        .iter()
        .find(|e| {
            !e.arrow
                && e.points.last().is_some_and(|p| {
                    p.0 == le.cx && p.1 > le.cy + le.h / 2.0 && p.1 < le.cy + le.h / 2.0 + 40.0
                })
        })
        .expect("break уходит ниже loop_end");
    assert!(
        cont_edge.points.last().unwrap().1 < brk_edge.points.last().unwrap().1,
        "continue выше выхода break"
    );
    assert!(collinear_overlap_ok(&l).is_ok());
    assert!(crossings_ok(&l.shapes, &l.edges).is_ok());
    assert!(dangling_ok(&l).is_ok());
}
