//! Разбивка длинной схемы на листы А4 с кружками-соединителями.

use crate::frontend::MARKER;
use crate::ir::{Node, NodeKind, Stmt};
use crate::layout::measure::{no_sentinel, HEAD, MID, TAIL};
use crate::layout::{layout, Layout, Sizes};
use crate::style::Style;

/// Маркер ручного переноса листа: conn-узел с зарезервированным текстом.
fn is_marker(nd: &Node) -> bool {
    nd.kind == NodeKind::Conn && nd.text == MARKER
}

fn is_marker_stmt(s: &Stmt) -> bool {
    matches!(s, Stmt::Node(nd) if is_marker(nd))
}

/// Узел — кусок разрезанного цикла (split помечает их сентинелом в начале
/// текста): такой цикл уже разрезан, и остаток стоит делить дальше.
fn is_piece(nd: &Node) -> bool {
    matches!(nd.text.chars().next(), Some(HEAD | MID | TAIL))
}

/// Отрезать точку реза можно только перед простым блоком, «если» или
/// циклом. Якорь ПРЕДЫДУЩЕГО узла — низ последнего узла листа: лист
/// доливается до точки, ближайшей к `target` (высота остатка / число
/// листов), но не ниже `cap` (низ листа минус кружок-выход). Цель, а не
/// «последняя под потолком»: иначе лист выходит полным, а следующий —
/// огрызком из двух фигур, и лишний полупустой лист всё равно есть.
/// j ∈ from..stop — и до, и после реза остаётся хотя бы пара узлов (лист
/// не начинается огрызком); тело цикла режется «перед каждым
/// оператором» — там from = 1.
fn auto_cut(
    res: &Layout,
    items: &[Node],
    target: f64,
    cap: f64,
    stop: usize,
    from: usize,
) -> Option<usize> {
    let mut cut: Option<(usize, f64)> = None;
    let mut first_valid = None;
    for j in from..stop {
        if !matches!(
            items[j].kind,
            NodeKind::Act | NodeKind::Io | NodeKind::Decision | NodeKind::Loop
        ) {
            continue;
        }
        // return на top-level обрывает layout: якорей меньше, чем узлов
        let Some(a) = res.anchors.get(j.wrapping_sub(1)) else {
            continue;
        };
        if first_valid.is_none() {
            first_valid = Some(j);
        }
        if a.y > cap {
            continue;
        }
        let d = (a.y - target).abs();
        if cut.is_none_or(|(_, best)| d < best) {
            cut = Some((j, d));
        }
    }
    // Схема из высоких структур (do-while с проверкой ввода) может не
    // иметь ни одного якоря под потолком — тогда любой рез лучше целого
    // переростка в тысячу пунктов: берём самый первый валидный.
    cut.map(|(j, _)| j).or(first_valid)
}

/// Не влезает в А4 -> части, соединённые кружками «А», «Б», ...
/// Межстраничный соединитель: буква + номер листа, где продолжение
/// (ГОСТ 19.701-90: первая строка — номер листа).
///
/// Маркер ручного переноса (`// перенос`, в .gvn — строка «перенос»)
/// ломает страницу даже если всё влезло: узел-маркер съедается, на его
/// месте встаёт пара кружков одной буквы, как у автореза.
pub fn split_scheme(mut items: Vec<Node>, sizes: &Sizes, st: &Style) -> Vec<Vec<Node>> {
    let mut parts: Vec<Vec<Node>> = Vec::new();
    let mut li = 0usize;
    let n_letters = st.letters.chars().count();
    let letter = |li: usize| st.letters.chars().nth(li % n_letters).unwrap();
    // Бюджет листа — А4 при масштабе split_scale (пайплайн ставит сюда
    // фактический масштаб пачки, см. render_batch): лист должен быть
    // заполнен, а не порезан под 1.0 и нарисован на пятую часть.
    let budget = (st.a4_h - 2.0 * st.page_pad) / st.split_scale;
    // под последним узлом листа встаёт кружок-выход: якорь реза не может
    // доходить до самого низа
    let exit_room = st.vgap + 2.0 * st.conn_r;
    let cap = budget - exit_room;
    loop {
        // маркер первым в остатке: резать перед ним нечего — глотаем, буква
        // не тратится (иначе вышел бы лист из одного кружка)
        while items.first().is_some_and(is_marker) {
            items.remove(0);
        }
        // маркер в конце остатка (за ним только «конец» или ничего): пустой
        // хвост и хвост из одного «конца» не дают ни листа, ни сиротского
        // кружка — тоже глотаем, с хвоста очереди
        while let Some(f) = items.iter().rposition(is_marker) {
            if items[f + 1..].iter().all(|n| n.kind == NodeKind::Term) {
                items.remove(f);
            } else {
                break;
            }
        }
        if items.is_empty() {
            break;
        }
        // маркер в теле цикла: тело режется по нему, границу метит такой
        // же маркер — листы ломает обычная машина разрезов
        if let Some(i) = items.iter().position(|nd| body_marker(nd).is_some()) {
            if splice_body_at_marker(&mut items, i) {
                continue;
            }
        }
        let res = layout(&items, sizes, st);
        let fits = res.bounds.3 <= budget;
        let marker = items.iter().position(is_marker);
        // политика пользователя: разрез — крайняя мера. Если целая схема
        // (вместе с телом цикла) влезает на А4 с неунизительным масштабом
        // (≥ split_min), она не режется вовсе — ни тело, ни поток
        if marker.is_none() && super::fit(res.bounds, st) >= st.split_min {
            parts.push(items);
            break;
        }
        if fits && marker.is_none() {
            parts.push(items);
            break;
        }
        // цикл, чьё тело на лист не влезает, режем раньше прочих точек
        // реза: иначе поток перед ним уедет на пустоватый лист
        if !fits && split_loop_body(&mut items, sizes, st, &res, cap) {
            continue;
        }
        // короткий остаток делим, только если делить есть что — маркер или
        // уже разрезанный цикл. Иначе выходит лишний пустоватый лист:
        // схема из четырёх узлов и так уезжает на один лист целиком
        let empty_branch_decision = items.iter().any(|nd| {
            nd.kind == NodeKind::Decision
                && nd.switch_var.is_none()
                && nd.branches.iter().any(|b| {
                    !b.stmts.is_empty() && !b.to_end && b.link.is_none() && b.stmts.len() >= 2
                })
        });
        let divisible = items.len() >= 3
            || marker.is_some()
            || items.iter().any(is_piece)
            || empty_branch_decision;
        if !fits && !divisible {
            parts.push(items);
            break;
        }
        // ручной приоритет: маркер режет страницу, если голова до него
        // влезает на лист целиком (с кружком-выходом); не влезает — сперва
        // авторез внутри головы, маркер ждёт следующей страницы
        let head_fits = marker.map_or(true, |f| {
            layout(&items[..f], sizes, st).bounds.3 + exit_room <= budget
        });
        let cut = if fits || (marker.is_some() && head_fits) {
            marker
        } else {
            // маркер — потолок автореза: кусок не должен унести его с собой
            let stop = marker
                .unwrap_or(items.len())
                .min(items.len().saturating_sub(1));
            // остаток делим на равные доли: n листов по rem/n, а не
            // «полный лист + что осталось». Голова до маркера мерится
            // отдельно — маркер и есть потолок.
            let rem = if stop + 1 < items.len() {
                layout(&items[..stop], sizes, st).bounds.3
            } else {
                res.bounds.3
            };
            let n = (rem / budget).ceil().max(1.0);
            let target = rem / n - exit_room;
            auto_cut(&res, &items, target, cap, stop, 2).or(marker)
        };
        let Some(cut) = cut else {
            // верхний уровень резать нечем, а переросток — длинная ветка
            // ромба: режем внутри неё (кружки A/OUT), иначе лист атомарен.
            // Опорный проход (split_scale 1.0) не трогаем: он меряет
            // натуральный масштаб схемы, и дробление ветки там занижает
            // бюджет следующего прохода.
            if !fits
                && st.split_scale < 0.999
                && split_decision_branch(
                    &mut items, sizes, st, budget, &mut parts, &letter, &mut li,
                )
            {
                continue;
            }
            // верхний уровень резать нечем — лист доливается целиком
            parts.push(items);
            break;
        };
        let l = letter(li);
        li += 1;
        let mut head = items[..cut].to_vec();
        head.push(Node::new(NodeKind::Conn, l.to_string()));
        parts.push(head);
        let mut tail: Vec<Node> = items.drain(cut..).collect();
        // маркеры подряд на стыке листов: голова уже кончилась — лишние
        // глотаем, иначе средний лист выходил бы из двух кружков друг под
        // другом (первый же кружок — это и есть разрыв)
        while tail.first().is_some_and(is_marker) {
            tail.remove(0);
        }
        let mut rest = vec![Node::new(NodeKind::Conn, l.to_string())];
        rest.extend(tail);
        items = rest;
    }
    link_branches_to_end(&mut parts, &letter, &mut li);
    glue_loop_pieces(&mut parts);
    parts
}

/// Голова и хвост разрезанного цикла попали на один лист — цикл рисуется
/// целиком. Иначе головная часть уводит рельсы break/continue в ствол
/// листа (rails_to_page), и рельса вливается в продолжение тела: на 1.c
/// горизонталь break садилась на ствол перед следующим if, образуя
/// 4-сторонний узел с неоднозначным продолжением. Части на разных листах
/// не склеиваются — там рельсы честно уходят на кружок-выход.
fn glue_loop_pieces(parts: &mut [Vec<Node>]) {
    for part in parts {
        let mut out: Vec<Node> = Vec::with_capacity(part.len());
        let mut i = 0;
        while i < part.len() {
            let is_head =
                part[i].kind == NodeKind::Loop && part[i].text.chars().next() == Some(HEAD);
            if !is_head {
                out.push(part[i].clone());
                i += 1;
                continue;
            }
            let mut body = part[i].body.clone().unwrap_or_default();
            let mut j = i + 1;
            let mut closed = false;
            while j < part.len() && part[j].kind == NodeKind::Loop {
                match part[j].text.chars().next() {
                    Some(MID) => {
                        body.extend(part[j].body.clone().unwrap_or_default());
                        j += 1;
                    }
                    Some(TAIL) => {
                        body.extend(part[j].body.clone().unwrap_or_default());
                        j += 1;
                        closed = true;
                        break;
                    }
                    _ => break,
                }
            }
            if closed {
                let mut nd = Node::new(NodeKind::Loop, no_sentinel(&part[i].text).to_string());
                nd.body = Some(body);
                out.push(nd);
                i = j;
            } else {
                out.push(part[i].clone());
                i += 1;
            }
        }
        *part = out;
    }
}

/// Маркеры ручного переноса не рисуются там, где страниц нет
/// (no_split, --trim, --drawio): они уходят из дерева целиком.
pub fn strip_markers(nodes: &mut Vec<Node>) {
    nodes.retain(|nd| !is_marker(nd));
    for nd in nodes.iter_mut() {
        strip_children(nd);
    }
}

fn strip_children(nd: &mut Node) {
    for br in &mut nd.branches {
        strip_stmts(&mut br.stmts);
    }
    if let Some(body) = &mut nd.body {
        strip_stmts(body);
    }
}

fn strip_stmts(stmts: &mut Vec<Stmt>) {
    stmts.retain(|s| !is_marker_stmt(s));
    for s in stmts.iter_mut() {
        if let Stmt::Node(nd) = s {
            strip_children(nd);
        }
    }
}

/// Stmt ветки -> узел верхнего уровня (хвост разрезанной ветки едет на
/// следующий лист как обычный поток). Рельсы сюда не попадают: ветку с
/// ними не режем.
fn stmt_node(s: Stmt) -> Option<Node> {
    match s {
        Stmt::Tile { kind, text } => Some(Node::new(NodeKind::from(kind), text)),
        Stmt::Return(t) => Some(Node::new(NodeKind::Return, t)),
        Stmt::Node(nd) => Some(*nd),
        Stmt::Break | Stmt::Continue => None,
    }
}

/// Тело цикла -> псевдо-узлы верхнего уровня + карта обратно в Stmt.
/// Break/Continue в колонке нулевой высоты: в верхнеуровневом замере они
/// врали бы, поэтому пропущены, а карта помнит их места в теле. Рельс
/// обрывает колонку: всё за ним мёртво (column.rs выходит на ColEnd::Rail),
/// так что в замер оно не попадает и резать по нему нечего.
fn pseudo_nodes(body: &[Stmt]) -> (Vec<Node>, Vec<usize>) {
    let mut nodes = Vec::new();
    let mut map = Vec::new();
    for (i, st) in body.iter().enumerate() {
        if is_rail(st) {
            break;
        }
        let nd = match st {
            Stmt::Tile { kind, text } => Node::new(NodeKind::from(*kind), text.clone()),
            Stmt::Return(t) => Node::new(NodeKind::Return, t.clone()),
            Stmt::Node(nd) => (**nd).clone(),
            // недостижимо: рельсы отсечены выше, но match полон
            Stmt::Break | Stmt::Continue => continue,
        };
        nodes.push(nd);
        map.push(i);
    }
    (nodes, map)
}

/// Рельс (break/continue) обрывает колонку: поток после него уходит.
fn is_rail(s: &Stmt) -> bool {
    matches!(s, Stmt::Break | Stmt::Continue)
}

/// Первый маркер ручного переноса прямо в теле цикла. Во вложенных
/// телах не ищем: там маркер молча глотает охрана колонки.
fn body_marker(nd: &Node) -> Option<usize> {
    if nd.kind != NodeKind::Loop {
        return None;
    }
    nd.body.as_ref()?.iter().position(is_marker_stmt)
}

/// Маркер ручного переноса в теле цикла: тело режется по нему, а на
/// границе встаёт такой же маркер — страницу ломает обычная машина
/// разрезов (листы по маркерам выходят, даже если цикл влезал целиком).
/// Маркер у края тела резать не на чем — он съедается.
fn splice_body_at_marker(items: &mut Vec<Node>, i: usize) -> bool {
    let mut body = match items[i].body.clone() {
        Some(b) => b,
        None => return false,
    };
    let Some(f) = body.iter().position(is_marker_stmt) else {
        return false;
    };
    if f == 0 || f + 1 == body.len() {
        body.remove(f);
        items[i].body = Some(body);
        return false;
    }
    let tail = body[f + 1..].to_vec();
    body.truncate(f);
    splice_loop(items, i, body, tail);
    items.insert(i + 1, Node::new(NodeKind::Conn, MARKER));
    true
}

/// items[i] — цикл; тело режется по m: первый кусок остаётся на месте,
/// второй встаёт за ним. Сентинел первого куска — режим исходного узла
/// (голова уже начата — второй кусок середина), второго — хвост/середина.
fn splice_loop(items: &mut Vec<Node>, i: usize, head: Vec<Stmt>, tail: Vec<Stmt>) {
    let nd = &items[i];
    let label = no_sentinel(&nd.loop_label()).to_string();
    let (first, second) = match nd.text.chars().next() {
        Some(HEAD) => (HEAD, MID),
        Some(MID) => (MID, MID),
        Some(TAIL) => (MID, TAIL),
        _ => (HEAD, TAIL),
    };
    let mut h = Node::new(
        NodeKind::Loop,
        if first == HEAD {
            format!("{HEAD}{label}")
        } else {
            MID.to_string()
        },
    );
    h.body = Some(head);
    let mut t = Node::new(NodeKind::Loop, second.to_string());
    t.body = Some(tail);
    items[i] = h;
    items.insert(i + 1, t);
}

/// Авторез тела цикла: верхний уровень резать нечего, а цикл перерос
/// лист. Тело — псевдо-узлы верхнего уровня, тот же поиск реза; узел
/// items[i] заменяется парой кусков, их дальше режет обычная машина.
/// true — что-то разрезали (внешний цикл обязан пересчитать раскладку).
fn split_loop_body(
    items: &mut Vec<Node>,
    sizes: &Sizes,
    st: &Style,
    res: &Layout,
    cap: f64,
) -> bool {
    let (_, lh) = sizes["loop"];
    for i in 0..items.len() {
        if items[i].kind != NodeKind::Loop {
            continue;
        }
        let Some(body) = items[i].body.clone() else {
            continue;
        };
        let (pseudo, map) = pseudo_nodes(&body);
        if pseudo.len() < 2 {
            continue;
        }
        // чанк меряется от низа прошлого узла: трапеция и оба в-гэпа
        // плюс запас на кружок-выход листа
        let approach = res.anchors.get(i.wrapping_sub(1)).map_or(0.0, |a| a.y);
        let body_limit = cap - approach - lh - 2.0 * st.vgap;
        if body_limit <= 0.0 {
            continue;
        }
        let pres = layout(&pseudo, sizes, st);
        // тело — тоже на равные доли: последний кусок не из одного шага
        let n = (pres.bounds.3 / body_limit).ceil().max(1.0);
        let target = pres.bounds.3 / n;
        let Some(j) = auto_cut(&pres, &pseudo, target, body_limit, pseudo.len(), 1) else {
            continue;
        };
        let m = map[j];
        // рельс и мёртвый хвост за ним остаются в заднем куске: псевдо-замер
        // их не видел, значит рез всегда до рельсы — кусок не начинается
        // рельсом из пустоты, а мёртвый код не попадает на лист живым
        splice_loop(items, i, body[..m].to_vec(), body[m..].to_vec());
        return true;
    }
    false
}

/// Рез внутри длинной ветки ромба: верх ветки остаётся на листе и
/// кончается кружком A, а под ромбом ставится кружок OUT (продолжение
/// после if). Следующий лист: A -> остаток ветки -> OUT -> дальнейший
/// поток. Так крупный атомарный ромб (`if (!eof) { ... }`) перестаёт
/// растягивать весь лист.
///
/// Пустая ветка-заглушка и остальные ветки сливаются на шине ромба и
/// входят в OUT; остаток разрезанной ветки входит в тот же OUT снизу —
/// одинаковые буквы и есть точка слияния (как link «-> конец»).
fn split_decision_branch(
    items: &mut Vec<Node>,
    sizes: &Sizes,
    st: &Style,
    budget: f64,
    parts: &mut Vec<Vec<Node>>,
    letter: &impl Fn(usize) -> char,
    li: &mut usize,
) -> bool {
    for j in 0..items.len() {
        if items[j].kind != NodeKind::Decision || items[j].switch_var.is_some() {
            continue;
        }
        for bi in 0..items[j].branches.len() {
            let br = &items[j].branches[bi];
            // ветка уже уходит на другой лист/в «конец» либо мертва:
            // выносить из неё хвост некуда
            if br.to_end || br.link.is_some() || br.stmts.len() < 2 {
                continue;
            }
            if br.stmts.iter().any(|s| {
                matches!(s, Stmt::Return(_) | Stmt::Break | Stmt::Continue)
                    || matches!(s, Stmt::Node(nd) if nd.kind == NodeKind::Conn && nd.text == MARKER)
            }) {
                continue;
            }
            let mut cut = None;
            for m in (1..br.stmts.len()).rev() {
                let mut d = items[j].clone();
                let mut head = d.branches[bi].stmts[..m].to_vec();
                // буква влияет только на подпись кружка, ширина общая
                head.push(Stmt::Node(Box::new(Node::new(NodeKind::Conn, "А"))));
                d.branches[bi].stmts = head;
                let mut trial: Vec<Node> = items[..j].to_vec();
                trial.push(d);
                trial.push(Node::new(NodeKind::Conn, "Б"));
                if layout(&trial, sizes, st).bounds.3 <= budget {
                    cut = Some(m);
                    break;
                }
            }
            let Some(m) = cut else { continue };
            let a = letter(*li);
            *li += 1;
            let out = letter(*li);
            *li += 1;
            let tail: Vec<Node> = items[j].branches[bi].stmts[m..]
                .iter()
                .cloned()
                .filter_map(stmt_node)
                .collect();
            let mut head_page: Vec<Node> = items[..=j].to_vec();
            let mut hb = head_page[j].branches[bi].stmts[..m].to_vec();
            hb.push(Stmt::Node(Box::new(Node::new(
                NodeKind::Conn,
                a.to_string(),
            ))));
            head_page[j].branches[bi].stmts = hb;
            head_page.push(Node::new(NodeKind::Conn, out.to_string()));
            parts.push(head_page);
            let mut rest = vec![Node::new(NodeKind::Conn, a.to_string())];
            rest.extend(tail);
            rest.push(Node::new(NodeKind::Conn, out.to_string()));
            rest.extend(items[j + 1..].iter().cloned());
            *items = rest;
            return true;
        }
    }
    false
}

/// Ветки «-> конец» в не-последних частях кончаются кружком: сам «конец»
/// живёт на последнем листе, туда же ставятся его кружки.
fn link_branches_to_end(parts: &mut [Vec<Node>], letter: &impl Fn(usize) -> char, li: &mut usize) {
    let mut links: Vec<(char, usize)> = Vec::new();
    for pi in 0..parts.len().saturating_sub(1) {
        for nd in &mut parts[pi] {
            if nd.kind != NodeKind::Decision {
                continue;
            }
            for br in &mut nd.branches {
                if br.to_end && br.link.is_none() {
                    let l = letter(*li);
                    *li += 1;
                    br.link = Some(l);
                    links.push((l, pi + 1));
                }
            }
        }
    }
    links.sort();
    // входящие кружки у «конца»: ir.rs закрыт для правок, поэтому
    // inbound моделируется conn-узлами прямо перед терминатором —
    // layout рисует их слева от «конца» (leads_to_end)
    for (l, _) in &links {
        let last = parts.len() - 1;
        let at = parts[last].len() - 1;
        parts[last].insert(at, Node::new(NodeKind::Conn, l.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::gvn;
    use crate::layout::normalize;

    /// return на top-level: layout обрывается, якорей меньше, чем узлов —
    /// split индексировал anchors[j-1] и паниковал (index out of bounds).
    #[test]
    fn split_after_top_level_return_no_panic() {
        let st = Style::default();
        let mut text = "a = 111111\n".repeat(14);
        text.push_str("return 1\n");
        text.push_str(&"z = 222222\n".repeat(4));
        let nodes = gvn::parse(&text, &st, "en").unwrap();
        let sizes = normalize(&nodes, &st);
        let parts = split_scheme(nodes, &sizes, &st);
        assert!(!parts.is_empty());
        assert!(parts.iter().all(|p| !p.is_empty()));
    }

    /// Длинная ветка ромба — атомарный переросток: верхний уровень
    /// резать нечего (ромб один), рез идёт внутри ветки кружками A/OUT.
    /// Каждый лист влезает в бюджет А4 и остаётся валидной схемой.
    #[test]
    fn long_decision_branch_splits_across_pages() {
        let st = Style::default();
        let mut text = String::from("if c > 0\n    да:\n");
        for i in 1..=70 {
            text.push_str(&format!("    a = {i}\n"));
        }
        let nodes = gvn::parse(&text, &st, "en").unwrap();
        let sizes = normalize(&nodes, &st);
        let cut_st = Style {
            split_scale: st.split_min,
            ..st.clone()
        };
        let parts = split_scheme(nodes, &sizes, &cut_st);
        assert!(parts.len() >= 2, "ветка не разрезана: {}", parts.len());
        let budget = (st.a4_h - 2.0 * st.page_pad) / st.split_min;
        for (i, p) in parts.iter().enumerate() {
            let l = layout(p, &sizes, &st);
            assert!(
                l.bounds.3 <= budget,
                "лист {i}: {:.0} > бюджета {budget:.0}",
                l.bounds.3
            );
            if let Err(e) = crate::layout::crossings_ok(&l.shapes, &l.edges) {
                panic!("лист {i}: {e}");
            }
            if let Err(e) = crate::layout::dangling_ok(&l) {
                panic!("лист {i}: {e}");
            }
            assert!(crate::layout::single_entry_ok(&l), "лист {i}");
        }
        // продолжение начинается с кружка A, а не с середины ветки
        assert!(
            parts
                .iter()
                .skip(1)
                .any(|p| p.first().is_some_and(|n| n.kind == NodeKind::Conn)),
            "нет листа-продолжения с кружком"
        );
    }

    /// Голова и хвост одного цикла на одном листе — снова целый цикл.
    /// Иначе головная часть уводила рельсы в ствол листа, и break
    /// вливался в продолжение тела (1.c: рельса садилась на ствол перед
    /// следующим if, 4-сторонний узел).
    #[test]
    fn head_and_tail_on_one_sheet_glue_into_full_loop() {
        let mut head = Node::new(NodeKind::Loop, format!("{HEAD}while !ok"));
        head.body = Some(vec![Stmt::Tile {
            kind: crate::ir::TileKind::Act,
            text: "eof = 1".into(),
        }]);
        let mut tail = Node::new(NodeKind::Loop, TAIL.to_string());
        tail.body = Some(vec![Stmt::Tile {
            kind: crate::ir::TileKind::Act,
            text: "ok = 1".into(),
        }]);
        let mut parts = vec![vec![head.clone(), tail.clone()]];
        glue_loop_pieces(&mut parts);
        assert_eq!(parts[0].len(), 1, "пара кусков слилась в один узел");
        let nd = &parts[0][0];
        assert_eq!(nd.kind, NodeKind::Loop);
        assert_eq!(no_sentinel(&nd.text), "while !ok");
        assert_eq!(nd.body.as_ref().map(Vec::len), Some(2), "тело обоих кусков");
        assert!(!is_piece(nd), "сентинела больше нет");

        // куски на разных листах (между ними кружок) не склеиваются
        let mut parts = vec![vec![head, Node::new(NodeKind::Conn, "А"), tail]];
        glue_loop_pieces(&mut parts);
        assert_eq!(parts[0].len(), 3);
    }
}

// Диагностика разрезов по corpus-файлу: печать частей (kinds/texts/bounds)
// с --ignored --nocapture, числа фиксируются до правок порогов.
#[cfg(test)]
mod dbg_parts {
    use super::*;

    fn dbg_file(path: &str) {
        let st = Style::default();
        let text = std::fs::read_to_string(path).unwrap();
        let funcs = crate::frontend::c::c_to_gvn_functions(&text, "en").unwrap();
        let schemes: Vec<(String, Vec<Node>)> = funcs
            .into_iter()
            .map(|(name, gvn)| {
                let n = crate::frontend::gvn::parse(&gvn, &st, "en").unwrap();
                (name, n)
            })
            .collect();
        let sizes = crate::layout::uniform_sizes(
            &schemes
                .iter()
                .map(|(_, n)| crate::layout::normalize(n, &st))
                .collect::<Vec<_>>(),
        );
        let budget = (st.a4_h - 2.0 * st.page_pad) / st.split_scale;
        println!("{path}: бюджет части {budget:.0}");
        for (name, nodes) in schemes {
            let parts = split_scheme(nodes, &sizes, &st);
            println!("схема {name}: частей {}", parts.len());
            for (i, p) in parts.iter().enumerate() {
                let l = crate::layout::layout(p, &sizes, &st);
                let kinds: Vec<String> = p.iter().map(|nd| format!("{:?}", nd.kind)).collect();
                let texts: Vec<String> = p
                    .iter()
                    .map(|nd| nd.text.chars().take(18).collect())
                    .collect();
                let anchors: Vec<i64> = l.anchors.iter().map(|a| a.y as i64).collect();
                println!(
                    "  часть {i}: узлов {} {}x{} scale {:.3}/{:.3} kinds {kinds:?}\n    texts {texts:?}\n    anchors {anchors:?}",
                    p.len(),
                    l.bounds.2 as i64,
                    l.bounds.3 as i64,
                    (st.a4_w - 2.0 * st.page_pad) / l.bounds.2.max(1.0),
                    (st.a4_h - 2.0 * st.page_pad) / l.bounds.3.max(1.0),
                );
                if l.bounds.3 > budget || l.bounds.2 > st.a4_w - 2.0 * st.page_pad {
                    for sh in &l.shapes {
                        println!(
                            "    фигура {} ({:.0},{:.0}) {:.0}x{:.0} {:?}",
                            sh.kind, sh.cx, sh.cy, sh.w, sh.h, sh.lines
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[ignore]
    fn dbg_parts_lab1() {
        dbg_file("corpus/oap-lab1.c");
    }

    #[test]
    #[ignore]
    fn dbg_parts_lab4() {
        dbg_file("corpus/oap-lab4.c");
    }
}
