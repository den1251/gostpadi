use super::ctx::{BreakAt, ColEnd, Ctx};
use super::measure::kind_name;
use super::types::Sizes;
use crate::frontend::MARKER;
use crate::ir::{Branch, NodeKind, Stmt, TileKind};
use crate::style::Style;

/// Колонка состоит только из break/continue: входить в неё со стрелкой
/// нечего — поток уйдёт рельсой или растворится в слиянии кейса.
pub(super) fn rail_only(items: &[Stmt]) -> bool {
    !items.is_empty()
        && items
            .iter()
            .all(|s| matches!(s, Stmt::Break | Stmt::Continue))
}

/// Полуширины содержимого колонки вокруг её оси: (влево, вправо).
/// Меряются по ФАКТИЧЕСКИМ фигурам колонки, а не по полной ширине
/// плитки: колонка из одного printf ужимается до io/2, иначе каждая
/// ветка резервировала под себя самую широкую плитку схемы (act 297), а
/// симметричная оценка удваивала вынос ветки — и глубокая вложенность
/// раздувала лист вдвое.
pub(super) fn extent_lr(sizes: &Sizes, st: &Style, colw: f64, items: &[Stmt]) -> (f64, f64) {
    let (mut hl, mut hr) = (0.0f64, 0.0f64);
    for it in items {
        match it {
            Stmt::Tile { kind, .. } => {
                // ширина своей фигуры: ret/term бывают шире колонки —
                // иначе коридор каскада садился внутрь силуэта плитки
                // (classify.c: вертикаль «return -1» проходила сквозь
                // самый блок)
                let key = match kind {
                    TileKind::Io => "io",
                    TileKind::Act => "act",
                };
                let e = sizes[key].0 / 2.0;
                hl = hl.max(e);
                hr = hr.max(e);
            }
            Stmt::Return(_) => {
                let e = sizes["ret"].0 / 2.0;
                hl = hl.max(e);
                hr = hr.max(e);
            }
            Stmt::Node(nd) if nd.kind == NodeKind::Conn => {
                // кружок-соединитель в конце ветки (рез листа): точка
                // ухода, поток ветки здесь заканчивается
                if nd.text != MARKER {
                    let e = sizes["conn"].0 / 2.0;
                    hl = hl.max(e);
                    hr = hr.max(e);
                }
            }
            Stmt::Node(nd) if nd.kind == NodeKind::Decision => {
                let ne: Vec<&Branch> = nd.branches.iter().filter(|b| !b.stmts.is_empty()).collect();
                let ext: Vec<(f64, f64)> = ne
                    .iter()
                    .map(|b| extent_lr(sizes, st, colw, &b.stmts))
                    .collect();
                let hinge = sizes["if"].0 / 2.0 + st.hgap;
                let side_x = sizes["if"].0 / 2.0;
                // рельсой ветка становится только внутри цикла/switch; в
                // замер её берём широкой (консервативно — не уже рисунка)
                let narrow: Vec<bool> = Vec::new();
                let any_empty = nd.branches.iter().any(|b| b.stmts.is_empty());
                // switch от четырёх кейсов рисуется сеткой (по два в ряд):
                // его ширина — две колонки, а не веер по числу кейсов
                let (el, er) = if nd.switch_var.is_some() && ext.len() >= 4 {
                    let l = ext.iter().map(|e| e.0).fold(0.0, f64::max);
                    let r = ext.iter().map(|e| e.1).fold(0.0, f64::max);
                    (hinge + l + st.grid, hinge + r + st.grid)
                } else if ext.len() == 1 && any_empty {
                    let bi = nd
                        .branches
                        .iter()
                        .position(|b| !b.stmts.is_empty())
                        .unwrap_or(0);
                    let (_, envl, envr) = super::ifnode::single_plan(
                        bi,
                        ext[0],
                        narrow.first().copied().unwrap_or(false),
                        hinge,
                        side_x,
                        0.0,
                    );
                    (envl, envr)
                } else {
                    let idxs: Vec<usize> = (0..ext.len()).collect();
                    let (_, envl, envr) = super::ifnode::pack_plan(
                        &idxs,
                        &ext,
                        &narrow,
                        hinge,
                        side_x,
                        st.colgap,
                        0.0,
                        nd.switch_var.is_none(),
                    );
                    (envl, envr)
                };
                hl = hl.max(el);
                hr = hr.max(er);
            }
            Stmt::Node(nd) => {
                // цикл: две трапеции + тело
                let e = sizes["loop"].0 / 2.0;
                hl = hl.max(e);
                hr = hr.max(e);
                if let Some(body) = &nd.body {
                    let (bl, br) = extent_lr(sizes, st, colw, body);
                    hl = hl.max(bl + st.mgap);
                    hr = hr.max(br + st.mgap);
                }
            }
            Stmt::Break | Stmt::Continue => {
                // после плитки break/continue рисуется шестиугольником:
                // его ширина может превысить узкую плитку колонки
                let e = sizes["exit"].0 / 2.0;
                hl = hl.max(e);
                hr = hr.max(e);
            }
        }
    }
    // колонка без фигур (только break/continue) не схлопывается на ось:
    // её вертикаль совпала бы со стволом
    if hl <= 0.0 && hr <= 0.0 {
        hl = colw / 2.0;
        hr = colw / 2.0;
    }
    (hl, hr)
}

/// Симметричная полуширина: рельсы, каналы и общий nhe схемы.
pub(super) fn extent(sizes: &Sizes, st: &Style, colw: f64, items: &[Stmt]) -> f64 {
    let (l, r) = extent_lr(sizes, st, colw, items);
    l.max(r)
}

impl Ctx<'_> {
    pub(super) fn extent_lr(&self, items: &[Stmt]) -> (f64, f64) {
        extent_lr(self.sizes, self.st, self.colw, items)
    }

    /// Колонка на абсциссе tx: плитки и вложенные «если»/циклы.
    /// Возвращает (y низа, чем закончена): тупик return закрывает
    /// колонку, рельса break/continue уводит поток к scope-владельцу.
    pub(super) fn render_column(&mut self, items: &[Stmt], tx: f64, top0: f64) -> (f64, ColEnd) {
        let mut prev_bottom: Option<f64> = None;
        for it in items {
            let top = match prev_bottom {
                None => top0,
                Some(pb) => pb + self.vg(),
            };
            match it {
                // маркер ручного переноса внутри ветки: страниц внутри
                // колонки нет, узел глотается молча (раньше падал в sub_loop)
                Stmt::Node(nd) if nd.kind == NodeKind::Conn && nd.text == MARKER => {}
                // кружок-соединитель в конце ветки: поток ветки уходит на
                // другой лист, слияние ромба сюда не тянется
                Stmt::Node(nd) if nd.kind == NodeKind::Conn => {
                    let (_, h_c) = self.sizes["conn"];
                    if let Some(pb) = prev_bottom {
                        self.edge(&[(tx, pb), (tx, top)], true);
                    }
                    self.add("conn", tx, top + h_c / 2.0, &nd.text);
                    return (top + h_c, ColEnd::Rail);
                }
                Stmt::Node(nd) => {
                    if let Some(pb) = prev_bottom {
                        self.edge(&[(tx, pb), (tx, top)], true);
                    }
                    let (y_bot, end) = if nd.kind == NodeKind::Decision {
                        self.sub_if(nd, tx, top)
                    } else {
                        self.sub_loop(nd, tx, top)
                    };
                    if end != ColEnd::Flow {
                        // тупик или рельса: следующие инструкции колонки
                        // недостижимы, слияние колонки не рисуется
                        return (y_bot, end);
                    }
                    prev_bottom = Some(y_bot);
                }
                Stmt::Return(t) => {
                    let (_, h_r) = self.sizes["ret"];
                    if let Some(pb) = prev_bottom {
                        self.edge(&[(tx, pb), (tx, top)], true);
                    }
                    self.add("ret", tx, top + h_r / 2.0, t);
                    return (top + h_r, ColEnd::Return);
                }
                Stmt::Break | Stmt::Continue => {
                    let is_break = matches!(it, Stmt::Break);
                    let text = if is_break { "break" } else { "continue" };
                    let scoped = self.loop_depth > 0 || self.switch_depth > 0;
                    if scoped {
                        // break напрямую в кейс-колонке switch —
                        // растворяется: слияние кейса само доводит поток
                        // до шины switch (в C break покидает только
                        // switch, а не цикл вокруг)
                        if is_break && self.switch_depth > 0 && self.case_direct {
                            return (prev_bottom.unwrap_or(top0), ColEnd::Flow);
                        }
                        // Выход из потока после плитки — отдельный
                        // шестиугольник с подписью ВНУТРИ. Поток входит в
                        // его верхнюю вершину стрелкой, рельса уходит из
                        // нижней. Раньше подпись висела снаружи, а рельса
                        // рисовала под плиткой «пустую» фигуру и шла
                        // насквозь (1.c: eof = 1; break;).
                        let mut y = prev_bottom.unwrap_or(top0);
                        let marked = if let Some(pb) = prev_bottom {
                            let (_, h_e) = self.sizes["exit"];
                            let cy = top + h_e / 2.0;
                            self.add("exit", tx, cy, text);
                            self.edge(&[(tx, pb), (tx, top)], true);
                            y = top + h_e;
                            true
                        } else {
                            false
                        };
                        let exit = BreakAt {
                            tx,
                            y,
                            from_tile: prev_bottom.is_some(),
                            marked,
                        };
                        if is_break {
                            self.breaks.push(exit);
                        } else {
                            self.continues.push(exit);
                        }
                        return (y, ColEnd::Rail);
                    }
                    // вне цикла и switch — прежняя плитка
                    self.draw_tile(text, TileKind::Act, tx, top, &mut prev_bottom);
                }
                Stmt::Tile { kind, text } => {
                    self.draw_tile(text, *kind, tx, top, &mut prev_bottom);
                }
            }
        }
        (prev_bottom.unwrap_or(top0), ColEnd::Flow)
    }

    /// Обычная плитка на колонке.
    fn draw_tile(
        &mut self,
        text: &str,
        kind: TileKind,
        tx: f64,
        top: f64,
        prev_bottom: &mut Option<f64>,
    ) {
        let k = match kind {
            TileKind::Io => "io",
            TileKind::Act => "act",
        };
        let (_, h_k) = self.sizes[k];
        let bottom = top + h_k;
        self.add(k, tx, top + h_k / 2.0, text);
        if let Some(pb) = *prev_bottom {
            self.edge(&[(tx, pb), (tx, top)], true);
        }
        *prev_bottom = Some(bottom);
    }

    /// Простой блок на основной линии. merge_stop: Some(y) — входная
    /// линия кончается T-стыком на y без стрелки (слияние pend над End).
    pub(super) fn simple(
        &mut self,
        nd: &crate::ir::Node,
        prev: Option<(f64, f64)>,
        cursor: f64,
        merge_stop: Option<f64>,
    ) -> ((f64, f64), f64, usize) {
        let key = kind_name(&nd.kind);
        let (_, h) = self.sizes[key];
        let cy = cursor + self.vg() + h / 2.0;
        let i = self.add(key, 0.0, cy, &nd.text);
        match (prev, merge_stop) {
            (Some(p), Some(my)) => self.edge(&[p, (0.0, my)], false),
            (Some(p), None) => self.edge(&[p, (0.0, cy - h / 2.0)], true),
            _ => {}
        }
        self.anchors.push(super::Anchor {
            x: 0.0,
            y: cy + h / 2.0,
        });
        ((0.0, cy + h / 2.0), cy + h / 2.0, i)
    }
}
