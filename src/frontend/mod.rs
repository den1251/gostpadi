pub mod c;
#[cfg(feature = "autofix")]
pub mod fix;
pub mod gvn;

use crate::ir::{Branch, TileKind};
use std::sync::atomic::{AtomicBool, Ordering};

/// Мягкая нормализация входа: включена в сборке с feature `autofix` и до
/// первого `--no-fix`. Флаг, а не поле Options: он один на процесс и не
/// должен прорастать через пайплайн, serve и wasm.
static AUTOFIX: AtomicBool = AtomicBool::new(true);

/// `--no-fix` на один запуск: строгий разбор.
pub fn set_autofix(on: bool) {
    AUTOFIX.store(on, Ordering::Relaxed);
}

pub fn autofix_enabled() -> bool {
    cfg!(feature = "autofix") && AUTOFIX.load(Ordering::Relaxed)
}

/// Маркер ручного переноса листа: комментарий `// перенос` в C и строка
/// «перенос» в .gvn. В IR это conn-узел с этим текстом, в split — точка
/// форсированного разрыва; до SVG он не доезжает никогда.
pub const MARKER: &str = "перенос";

/// Единственная граница строкового распознавания ввода-вывода: оба
/// frontend'а зовут её при строительстве плитки; layout и measure
/// читают готовый kind и текст не смотрят.
pub(crate) fn tile_kind(text: &str) -> TileKind {
    if crate::style::Style::DEFAULT.is_io(text) {
        TileKind::Io
    } else {
        TileKind::Act
    }
}

/// C-проваливание: пустая ветка case перед непустой — алиас («3:» перед
/// «4: тело» выполняет тело, а не рисуется рельсой обхода). Метки
/// склеиваются через «, », пустая ветка исчезает. Последняя пустая
/// (или пустая с «-> конец») остаётся как была.
pub(crate) fn merge_case_aliases(branches: &mut Vec<Branch>) {
    for i in (0..branches.len().saturating_sub(1)).rev() {
        let empty_fall = branches[i].stmts.is_empty()
            && !branches[i].to_end
            && !branches[i + 1].stmts.is_empty();
        if empty_fall {
            let label = std::mem::take(&mut branches[i].label);
            let next = std::mem::take(&mut branches[i + 1].label);
            // одинаковый префикс «svar = » не дублируется: «k = 3» + «k = 4»
            // → «k = 3, 4», а не «k = 3, k = 4»
            branches[i + 1].label = match (label.split_once(" = "), next.split_once(" = ")) {
                (Some((p1, v1)), Some((p2, v2))) if p1 == p2 => format!("{p1} = {v1}, {v2}"),
                _ => format!("{label}, {next}"),
            };
            branches.remove(i);
        }
    }
}
