//! Разбивка длинной схемы на листы А4 с кружками-соединителями.

use crate::frontend::MARKER;
use crate::ir::{Node, NodeKind, Stmt};
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
/// доливается до последнего, что влез.
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
        // короткий остаток без маркера резать нечем: лист доливается
        if marker.is_none() && (fits || items.len() < 6) {
            parts.push(items);
            break;
        }
        let cut = if fits || items.len() < 6 {
            marker // влезло, но маркер требует разрыв: режем ровно по нему
        } else {
            // маркер — потолок автореза: кусок не должен унести его с собой
            let stop = marker.unwrap_or(items.len()).min(items.len() - 2);
            auto_cut(&res, &items, limit, stop).or(marker)
        };
        let Some(cut) = cut else {
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
