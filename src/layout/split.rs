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

/// Отрезать точку реза можно только перед простым блоком, «если» или
/// циклом: последняя валидная ≤ limit. Якорь ПРЕДЫДУЩЕГО узла: лист
/// доливается до последнего, что влез. j ∈ 2..stop — и до, и после
/// реза остаётся хотя бы пара узлов (лист не начинается огрызком).
fn auto_cut(res: &Layout, items: &[Node], limit: f64, stop: usize) -> Option<usize> {
    let mut cut = None;
    for j in 2..stop {
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
        if a.y <= limit {
            cut = Some(j);
        }
    }
    cut
}

/// Не влезает в А4 -> части, соединённые кружками «А», «Б», ...
/// Межстраничный соединитель: буква + номер листа, где продолжение
/// (ГОСТ 19.701-90: первая строка — номер листа).
///
/// Маркер ручного переноса (`// перенос`, в .gvn — строка «перенос»)
/// ломает страницу даже если всё влезло: узел-маркер съедается, на его
/// месте встаёт пара кружков одной буквы, как у автореза.
pub fn split_scheme(mut items: Vec<Node>, sizes: &Sizes, st: &Style) -> Vec<Vec<Node>> {
    // поиск точки реза ведётся по нижним 88% страницы: ниже листать
    // бессмысленно, выше — резать слишком рано
    const CUT_SEARCH_FRAC: f64 = 0.88;
    let mut parts: Vec<Vec<Node>> = Vec::new();
    let mut li = 0usize;
    let n_letters = st.letters.chars().count();
    let letter = |li: usize| st.letters.chars().nth(li % n_letters).unwrap();
    let limit = (st.a4_h - 2.0 * st.page_pad) * CUT_SEARCH_FRAC;
    let budget = (st.a4_h - 2.0 * st.page_pad) / st.split_scale;
    loop {
        // маркер у края остатка съедаем: пустой хвост и хвост из одного
        // «конца» не дают ни листа, ни сиротского кружка
        while let Some(f) = items.iter().position(is_marker) {
            if items[f + 1..].iter().all(|n| n.kind == NodeKind::Term) {
                items.remove(f);
            } else {
                break;
            }
        }
        if items.is_empty() {
            break;
        }
        let res = layout(&items, sizes, st);
        let fits = res.bounds.3 <= budget;
        let marker = items.iter().position(is_marker);
        if fits && marker.is_none() {
            parts.push(items);
            break;
        }
        let cut = if fits {
            marker // влезло, но маркер требует разрыв: режем ровно по нему
        } else {
            // маркер — потолок автореза: кусок не должен унести его с собой
            let stop = marker
                .unwrap_or(items.len())
                .min(items.len().saturating_sub(1));
            auto_cut(&res, &items, limit, stop).or(marker)
        };
        let Some(cut) = cut else {
            // верхний уровень резать нечем: пробуем разрезать тело цикла
            if split_loop_body(&mut items, sizes, st, &res, limit) {
                continue;
            }
            parts.push(items);
            break;
        };
        let l = letter(li);
        li += 1;
        let mut head = items[..cut].to_vec();
        head.push(Node::new(NodeKind::Conn, l.to_string()));
        parts.push(head);
        let mut tail: Vec<Node> = items.drain(cut..).collect();
        if is_marker(&tail[0]) {
            tail.remove(0);
        }
        let mut rest = vec![Node::new(NodeKind::Conn, l.to_string())];
        rest.extend(tail);
        items = rest;
    }
    link_branches_to_end(&mut parts, &letter, &mut li);
    parts
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

/// Тело цикла -> псевдо-узлы верхнего уровня + карта обратно в Stmt.
/// Break/Continue в колонке нулевой высоты: в верхнеуровневом замере они
/// врали бы, поэтому пропущены, а карта помнит их места в теле.
fn pseudo_nodes(body: &[Stmt]) -> (Vec<Node>, Vec<usize>) {
    let mut nodes = Vec::new();
    let mut map = Vec::new();
    for (i, st) in body.iter().enumerate() {
        let nd = match st {
            Stmt::Tile { kind, text } => Node::new(NodeKind::from(*kind), text.clone()),
            Stmt::Return(t) => Node::new(NodeKind::Return, t.clone()),
            Stmt::Node(nd) => (**nd).clone(),
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
fn split_loop_body(items: &mut Vec<Node>, sizes: &Sizes, st: &Style, res: &Layout, limit: f64) -> bool {
    let (_, lh) = sizes["loop"];
    for i in 0..items.len() {
        if items[i].kind != NodeKind::Loop {
            continue;
        }
        let Some(body) = items[i].body.clone() else {
            continue;
        };
        let (pseudo, map) = pseudo_nodes(&body);
        if pseudo.len() < 5 {
            continue;
        }
        // чанк меряется от низа прошлого узла: трапеция и оба в-гэпа
        // плюс запас на кружок-выход листа
        let approach = res.anchors.get(i.wrapping_sub(1)).map_or(0.0, |a| a.y);
        let body_limit = limit - approach - lh - 2.0 * st.vgap;
        if body_limit <= 0.0 {
            continue;
        }
        let pres = layout(&pseudo, sizes, st);
        let Some(j) = auto_cut(&pres, &pseudo, body_limit, pseudo.len() - 1) else {
            continue;
        };
        let m = map[j];
        // рельс обрывает колонку: кусок кончается им, но не начинается —
        // иначе лист открывался бы рельсом из пустоты, а операторы за
        // рельсом не попали бы ни на один лист
        let mut cut = body[..m].iter().position(is_rail).map_or(m, |r| r + 1);
        while cut + 1 < body.len() && is_rail(&body[cut]) {
            cut += 1;
        }
        splice_loop(items, i, body[..cut].to_vec(), body[cut..].to_vec());
        return true;
    }
    false
}

/// Ветки «-> конец» в не-последних частях кончаются кружком: сам «конец»
/// живёт на последнем листе, туда же ставятся его кружки.
fn link_branches_to_end(
    parts: &mut [Vec<Node>],
    letter: &impl Fn(usize) -> char,
    li: &mut usize,
) {
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
}
