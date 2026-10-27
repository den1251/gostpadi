use super::column::rail_only;
use super::ctx::{BreakAt, ColEnd, Ctx};
use super::types::Label;
use crate::ir::{Branch, Node, NodeKind, Stmt};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Side {
    L,
    R,
    Axis,
}

/// Ярусы колонок ромба: (индекс среди непустых веток, сторона, ярус).
/// Канон методички (рис. 1–4): у ромба «да» (первая ветка) — вправо,
/// «нет» — влево. У switch «да/нет» нет: кейсы читаются слева направо,
/// средний стоит на оси (first_right = false).
fn plan_slots(n: usize, first_right: bool) -> Vec<(usize, Side, usize)> {
    let mut out = Vec::with_capacity(n);
    for j in 0..n {
        if n % 2 == 1 && j == n / 2 {
            out.push((j, Side::Axis, 0));
            continue;
        }
        let (mut side, t) = if j < n / 2 {
            (Side::L, n / 2 - 1 - j)
        } else {
            (Side::R, j - n / 2 - (n % 2))
        };
        if first_right {
            side = if side == Side::L { Side::R } else { Side::L };
        }
        out.push((j, side, t));
    }
    out
}

/// План колонок: (индекс ветки, сторона, ярус, абсцисса) и внешние
/// края (влево, вправо) от оси. Абсциссы пакуются по фактическим
/// полуширинам ветвей: колонка отстоит от ромба настолько, насколько
/// её содержимое выходит ВНУТРЬ (к оси), и встаёт вплотную к соседу по
/// своему внешнему краю. Общий на всю схему nhe раздувал каждую колонку
/// до самой глубокой в схеме (ЛБ2: ветка из одной плитки уезжала на
/// 524pt от оси), а симметричная оценка удваивала вынос вложенной ветки.
/// hinge — от оси до ближнего края колонки (dw/2 + hgap).
#[allow(clippy::too_many_arguments)]
pub(super) fn pack_plan(
    idxs: &[usize],
    ext: &[(f64, f64)],
    narrow: &[bool],
    hinge: f64,
    side_x: f64,
    colgap: f64,
    tx0: f64,
    first_right: bool,
) -> (Vec<(usize, Side, usize, f64)>, f64, f64) {
    let slots = plan_slots(idxs.len(), first_right);
    // осевая колонка (средний кейс switch): соседи обходят её вынос
    let axis = slots.iter().find(|s| s.1 == Side::Axis).map(|s| s.0);
    let (al, ar) = axis.map(|j| ext[j]).unwrap_or((0.0, 0.0));
    let mut off = vec![0.0f64; idxs.len()];
    let (mut envl, mut envr) = (al, ar);
    for side in [Side::L, Side::R] {
        let mut order: Vec<usize> = (0..slots.len()).filter(|&j| slots[j].1 == side).collect();
        order.sort_by_key(|&j| slots[j].2);
        // ближний край первой колонки: вынос осевой или полоса hgap
        let mut edge = if axis.is_some() {
            (if side == Side::L { ar } else { al }) + colgap
        } else {
            hinge
        }
        .max(hinge);
        for j in order {
            let (l, r) = ext[j];
            // колонка встаёт так, чтобы её внутренний вынос был свободен
            let x = edge + if side == Side::L { r } else { l };
            off[j] = if side == Side::L { -x } else { x };
            if side == Side::L {
                envl = envl.max(x + l);
            } else {
                envr = envr.max(x + r);
            }
            edge = x + if side == Side::L { l } else { r } + colgap;
        }
    }
    let plan = slots
        .iter()
        .map(|&(j, side, tier)| {
            // ветка из одних break/continue плиток не рисует: колонка
            // пустая, и её отлёт на hinge давал пустой прямоугольник.
            // Рельса идёт сразу от боковой вершины ромба вниз.
            let x = if narrow.get(j).copied().unwrap_or(false) {
                let sgn = if side == Side::L { -1.0 } else { 1.0 };
                tx0 + sgn * side_x
            } else {
                tx0 + off[j]
            };
            (idxs[j], side, tier, x)
        })
        .collect();
    (plan, envl, envr)
}

/// Одна непустая ветка (вторая пуста): колонка одна, по канону «да»
/// справа, «нет» слева; пустая ветка — сквозной проход по стволу.
pub(super) fn single_plan(
    bi: usize,
    ext: (f64, f64),
    narrow: bool,
    hinge: f64,
    side_x: f64,
    tx0: f64,
) -> (Vec<(usize, Side, usize, f64)>, f64, f64) {
    let side = if bi == 0 { Side::R } else { Side::L };
    if narrow {
        let sgn = if side == Side::R { 1.0 } else { -1.0 };
        return (vec![(bi, side, 0, tx0 + sgn * side_x)], side_x, side_x);
    }
    let (l, r) = ext;
    if side == Side::R {
        let x = hinge + l;
        (vec![(bi, Side::R, 0, tx0 + x)], hinge.max(x - l), x + r)
    } else {
        let x = hinge + r;
        (vec![(bi, Side::L, 0, tx0 - x)], x + l, hinge.max(x - r))
    }
}

/// Каскад else-if: последняя ветка содержит ровно один вложенный
/// Decision (рекурсивно), у каждого звена ровно 2 ветки, первая
/// непуста, без switch. Возвращает (ромбы, колонки): колонки = ромбы
/// плюс хвост-else, если тот непуст (иначе рельса). Ромбы < 2 —
/// обычный if/else, каскадом не рендерится.
pub(super) fn cascade_cols(nd: &Node) -> Option<(usize, usize)> {
    let mut k = 0usize;
    let mut cur = nd;
    loop {
        if cur.switch_var.is_some() || cur.branches.len() != 2 || cur.branches[0].stmts.is_empty() {
            return if k == 0 { None } else { Some((k, k)) };
        }
        k += 1;
        let last = cur.branches.last().unwrap();
        match last.stmts.as_slice() {
            [Stmt::Node(d)] if d.kind == NodeKind::Decision => cur = d,
            _ => return Some((k, k + usize::from(!last.stmts.is_empty()))),
        }
    }
}

impl Ctx<'_> {
    /// Слияние колонок на одну шину: вертикальные спуски остаются по
    /// колонкам, а горизонталь на уровне my рисуется одним отрезком от
    /// крайней колонки до крайней через target — наложенные хвосты
    /// отдельных колонок давали ступеньки разной жирности.
    pub(super) fn merge_bus(&mut self, cols: &[(f64, f64)], target: f64, my: f64) {
        if cols.is_empty() {
            return;
        }
        if cols.len() == 1 {
            let (x, y) = cols[0];
            self.edge(&[(x, y), (x, my), (target, my)], false);
            return;
        }
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for &(x, _) in cols {
            lo = lo.min(x);
            hi = hi.max(x);
        }
        for &(x, y) in cols {
            self.edge(&[(x, y), (x, my)], false);
        }
        self.edge(&[(lo.min(target), my), (hi.max(target), my)], false);
    }

    /// Спуск колонки ветки из ромба: ребро входа + метка да/нет/кейса.
    /// arrow=false — колонка без первой плитки (только break/continue):
    /// стрелка в пустоту запрещена, поток ведёт рельса или слияние.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn column_entry(
        &mut self,
        side: Side,
        t2: usize,
        txx: f64,
        cy: f64,
        y_b: f64,
        top2: f64,
        comb: bool,
        vl: (f64, f64),
        vr: (f64, f64),
        label: &str,
        arrow: bool,
    ) {
        let st = self.st;
        match side {
            Side::Axis => {
                self.edge(&[(txx, y_b), (txx, top2)], arrow);
                self.labels.push(Label {
                    x: txx + st.label_axis_dx,
                    y: top2 - st.label_dy,
                    text: label.into(),
                    ha: "left".into(),
                });
            }
            _ if comb => {
                self.edge(&[(txx, y_b), (txx, top2)], arrow);
                let left = side == Side::L;
                self.labels.push(Label {
                    x: txx + if left { -st.label_dx } else { st.label_dx },
                    y: top2 - st.label_dy,
                    text: label.into(),
                    ha: if left { "right" } else { "left" }.into(),
                });
            }
            _ => {
                let v = if side == Side::L { vl } else { vr };
                self.edge(&[v, (txx, cy), (txx, top2)], arrow);
                let x = if t2 == 0 {
                    v.0 + if side == Side::L {
                        -st.label_exit_dx
                    } else {
                        st.label_exit_dx
                    }
                } else {
                    (v.0 + txx) / 2.0
                };
                self.labels.push(Label {
                    x,
                    y: cy - st.label_dy,
                    text: label.into(),
                    ha: "center".into(),
                });
            }
        }
    }

    pub(super) fn comb_line(&mut self, plan: &[(usize, Side, usize, f64)], y_b: f64) {
        if plan.len() < 2 {
            return;
        }
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for &(_, _, _, x) in plan {
            lo = lo.min(x);
            hi = hi.max(x);
        }
        self.edge(&[(lo, y_b), (hi, y_b)], false);
    }

    /// Переключатель внутри колонки: кейсы сеткой по два в ряд. Веер
    /// из пяти колонок (по одной на кейс) раздувал ветку до 600pt от
    /// оси — а на А4 ширина и есть масштаб. Сетка держит ширину по двум
    /// колонкам, растёт вниз рядами; ствол на оси ведёт от ряда к ряду,
    /// слияние ряда — та же шина, что у веера.
    fn sub_switch_grid(&mut self, nd: &Node, tx: f64, top: f64) -> (f64, ColEnd) {
        let (dw, dh) = self.sizes["if"];
        let cy = top + dh / 2.0;
        self.add("if", tx, cy, &nd.text);
        let y_b = cy + dh / 2.0;
        let hinge = dw / 2.0 + self.st.hgap;
        let empty: Vec<String> = nd
            .branches
            .iter()
            .filter(|b| b.stmts.is_empty())
            .map(|b| b.label.clone())
            .collect();
        let cases: Vec<&Branch> = nd.branches.iter().filter(|b| !b.stmts.is_empty()).collect();
        let ext: Vec<(f64, f64)> = cases.iter().map(|b| self.extent_lr(&b.stmts)).collect();
        let saved_direct = self.case_direct;
        let br_mark = self.breaks.len();
        self.switch_depth += 1;
        self.case_direct = true;
        let mut y_bus = y_b + self.vg();
        self.edge(&[(tx, y_b), (tx, y_bus)], false);
        let mut merge2 = y_bus;
        // последний (нечётный) кейс стоит на оси: его низ — точка
        // продолжения, обходить плитку сбоку не нужно
        let mut axis_tail: Option<f64> = None;
        for (ri, row) in cases.chunks(2).enumerate() {
            if ri > 0 {
                // Шина ряда питается от оси switch, а не от слияния прошлого
                // ряда: слив и вход следующего ряда — разные высоты, иначе
                // выходы кейсов ряда 1 читаются как вход кейсов ряда 2.
                y_bus = merge2 + self.vg();
                self.edge(&[(tx, merge2), (tx, y_bus)], false);
            }
            let top_c = y_bus + self.vg();
            let mut bottoms: Vec<(f64, f64, ColEnd)> = Vec::new();
            for (ci, b) in row.iter().enumerate() {
                // нечётный хвост — на стволе: колонка продолжает ось,
                // отдельная шина ей не нужна
                let odd = row.len() == 1;
                let left = !odd && ci == 0;
                let k = ri * 2 + ci;
                let x = if odd {
                    tx
                } else if left {
                    tx - (hinge + ext[k].1)
                } else {
                    tx + (hinge + ext[k].0)
                };
                if odd {
                    self.edge(&[(tx, y_bus), (tx, top_c)], !rail_only(&b.stmts));
                } else {
                    self.edge(&[(tx, y_bus), (x, y_bus)], false);
                    self.edge(&[(x, y_bus), (x, top_c)], !rail_only(&b.stmts));
                }
                self.labels.push(Label {
                    x: x + if left {
                        -self.st.label_dx
                    } else {
                        self.st.label_dx
                    },
                    y: top_c - self.st.label_dy,
                    text: b.label.clone(),
                    ha: if left { "right" } else { "left" }.into(),
                });
                let (yend, end) = self.render_column(&b.stmts, x, top_c);
                bottoms.push((x, yend, end));
            }
            // шина ряда: капли живых колонок и один горизонтальный
            // отрезок через ствол; ниже неё стоят следующие ряды сетки.
            merge2 = bottoms
                .iter()
                .filter(|&&(_, _, e)| e == ColEnd::Flow)
                .map(|&(_, y, _)| y)
                .fold(y_bus + 2.0 * self.st.grid, f64::max)
                + self.st.mgap;
            let mut xs: Vec<f64> = vec![tx];
            for &(x, y, e) in &bottoms {
                if e == ColEnd::Flow {
                    self.edge(&[(x, y), (x, merge2)], false);
                    xs.push(x);
                }
            }
            let lo = xs.iter().cloned().fold(f64::MAX, f64::min);
            let hi = xs.iter().cloned().fold(f64::MIN, f64::max);
            if hi - lo > 1e-9 {
                self.edge(&[(lo, merge2), (hi, merge2)], false);
            }
            // Ствол между рядами: связь ромба с шиной следующего ряда.
            // У нечётного хвоста на оси стоит его колонка — там ствол не
            // рисуется. Ствол сдвинут на шаг от оси: вертикаль сквозь
            // точку шины давала узел с четырьмя сторонами (плюс), который
            // читается как пересечение двух линий, а не как ответвление.
            if row.len() > 1 {
                // живых колонок нет — шины слияния тоже нет: сдвиг и
                // колено ниже ряда возвращают ствол на ось
                let live = xs.len() > 1;
                let off = if hi > tx + self.st.grid {
                    self.st.grid
                } else if lo < tx - self.st.grid {
                    -self.st.grid
                } else {
                    self.st.grid
                };
                self.edge(&[(tx + off, y_bus), (tx + off, merge2)], false);
                if !live {
                    self.edge(&[(tx + off, merge2), (tx, merge2)], false);
                }
            }
            axis_tail = bottoms
                .iter()
                .find(|&&(x, _, _)| (x - tx).abs() < 1e-9)
                .map(|&(_, y, _)| y);
        }
        // пустой кейс: рельса справа за сеткой, слияние на ствол
        if !empty.is_empty() {
            let clear = ext.iter().map(|e| e.1).fold(hinge, f64::max) + hinge + self.st.grid;
            let bx = tx + super::geometry::up(clear, self.st.grid);
            for (k, lbl) in empty.iter().enumerate() {
                let bxk = bx + k as f64 * 2.0 * self.st.grid;
                self.edge(
                    &[(tx + dw / 2.0, cy), (bxk, cy), (bxk, merge2), (tx, merge2)],
                    false,
                );
                self.labels.push(Label {
                    x: tx + dw / 2.0 + self.st.label_exit_dx,
                    y: cy - self.st.label_dy,
                    text: lbl.clone(),
                    ha: "center".into(),
                });
            }
        }
        // рельсы break из вложенных if внутри кейсов: наружу за сетку
        let nested: Vec<BreakAt> = self.breaks.drain(br_mark..).collect();
        let outer = ext.iter().map(|e| e.1.max(e.0)).fold(hinge, f64::max) + hinge + self.st.grid;
        for b in &nested {
            let slot = self.break_slot;
            self.break_slot += 1;
            let sgn = if b.tx <= tx { -1.0 } else { 1.0 };
            let rx = tx
                + sgn * super::geometry::up(outer + slot as f64 * 2.0 * self.st.grid, self.st.grid);
            self.edge(&[(b.tx, b.y), (rx, b.y), (rx, merge2), (tx, merge2)], false);
        }
        self.switch_depth -= 1;
        self.case_direct = saved_direct;
        (merge2.max(axis_tail.unwrap_or(f64::MIN)), ColEnd::Flow)
    }

    /// Вложенный ромб внутри колонки: под-колонки вокруг tx, слияние
    /// обратно на ось колонки. Возвращает (y продолжения, конец).
    /// Для switch (switch_var) — это ещё и scope: кейс-колонки напрямую
    /// растворяют break в слиянии, вложенные уходят рельсой к выходу.
    pub(super) fn sub_if(&mut self, nd: &Node, tx: f64, top: f64) -> (f64, ColEnd) {
        // переключатель от трёх кейсов — сеткой, не веером: в веере
        // средний кейс стоит на оси, и его спуск со спуском продолжения
        // дают на шине узел с четырьмя сторонами (плюс)
        if nd.switch_var.is_some()
            && nd.branches.iter().filter(|b| !b.stmts.is_empty()).count() >= 3
        {
            return self.sub_switch_grid(nd, tx, top);
        }
        let (dw, dh) = self.sizes["if"];
        let cy = top + dh / 2.0;
        self.add("if", tx, cy, &nd.text);
        let vl = (tx - dw / 2.0, cy);
        let vr = (tx + dw / 2.0, cy);
        let idxs: Vec<usize> = (0..nd.branches.len())
            .filter(|&i| !nd.branches[i].stmts.is_empty())
            .collect();
        let empty: Vec<String> = nd
            .branches
            .iter()
            .filter(|b| b.stmts.is_empty())
            .map(|b| b.label.clone())
            .collect();
        // пустая ветка — короткое колено в свою сторону (0 — «да», 1 —
        // «нет»), а не ствол из нижней вершины ромба
        let empty_idx: Vec<(usize, String)> = nd
            .branches
            .iter()
            .enumerate()
            .filter(|(_, b)| b.stmts.is_empty())
            .map(|(i, b)| (i, b.label.clone()))
            .collect();
        let n = idxs.len();
        let ext: Vec<(f64, f64)> = idxs
            .iter()
            .map(|&i| self.extent_lr(&nd.branches[i].stmts))
            .collect();
        let hinge = dw / 2.0 + self.st.hgap;
        let is_switch = nd.switch_var.is_some();
        let scoped = is_switch || self.loop_depth > 0 || self.switch_depth > 0;
        // ветка из одних break/continue ВНУТРИ scope: колонки нет, рельса
        // идёт сразу от боковой вершины. Вне scope break рисуется плиткой,
        // и такая колонка обязана быть нормальной ширины.
        let narrow: Vec<bool> = if scoped {
            idxs.iter()
                .map(|&i| rail_only(&nd.branches[i].stmts))
                .collect()
        } else {
            Vec::new()
        };
        let comb = is_switch && n >= 2;
        let top2 = cy + dh / 2.0 + self.vg();
        let y_b = cy + dh / 2.0;
        let (plan, envl, envr) = if n == 1 && !empty.is_empty() {
            single_plan(
                idxs[0],
                ext[0],
                narrow.first().copied().unwrap_or(false),
                hinge,
                dw / 2.0,
                tx,
            )
        } else {
            pack_plan(
                &idxs,
                &ext,
                &narrow,
                hinge,
                dw / 2.0,
                self.st.colgap,
                tx,
                true,
            )
        };
        let env = envl.max(envr);
        if comb {
            self.comb_line(&plan, y_b);
        }
        let saved_direct = self.case_direct;
        let br_mark = self.breaks.len();
        if is_switch {
            self.switch_depth += 1;
            self.case_direct = true;
        } else {
            self.case_direct = false;
        }
        let mut bottoms: Vec<(usize, f64, ColEnd)> = Vec::new();
        for &(bi, side, t2, txx) in &plan {
            let arrow = !(scoped && rail_only(&nd.branches[bi].stmts));
            self.column_entry(
                side,
                t2,
                txx,
                cy,
                y_b,
                top2,
                comb,
                vl,
                vr,
                &nd.branches[bi].label,
                arrow,
            );
            let (y, end) = self.render_column(&nd.branches[bi].stmts, txx, top2);
            bottoms.push((bi, y, end));
        }
        self.case_direct = saved_direct;
        if is_switch {
            self.switch_depth -= 1;
        }
        let mut merge2 = y_b;
        // Осевая под-колонка (txx == tx) стоит на продолжении колонки:
        // её низ учитывается даже если колонка мёртвая (return), иначе
        // спуск продолжения протыкает ret насквозь.
        for (&(_, side, ..), &(_, y, end)) in plan.iter().zip(bottoms.iter()) {
            if side == Side::Axis || end == ColEnd::Flow {
                merge2 = merge2.max(y + self.st.mgap);
            }
        }
        // Пустая ветка простого if — короткое колено в свою сторону, а не
        // ствол из нижней вершины ромба. Пустой кейс switch — своя ветвь,
        // он уходит рельсой справа.
        let stub: Vec<(usize, String)> = if is_switch {
            Vec::new()
        } else {
            empty_idx.clone()
        };
        let stubs: Vec<(f64, f64, String)> = stub
            .iter()
            .map(|(i, lbl)| {
                let sgn = if *i == 0 { 1.0 } else { -1.0 };
                (tx + sgn * (dw / 2.0 + self.st.hgap), sgn, lbl.clone())
            })
            .collect();
        let rails: Vec<String> = if is_switch { empty.clone() } else { Vec::new() };
        if !stubs.is_empty() {
            merge2 = merge2.max(y_b + 2.0 * self.st.grid);
        }
        let mut cols: Vec<(f64, f64)> = bottoms
            .iter()
            .filter(|&&(_, _, end)| end == ColEnd::Flow)
            .map(|&(bi, y, _)| (plan.iter().find(|p| p.0 == bi).unwrap().3, y))
            .collect();
        for (sx, sgn, lbl) in &stubs {
            self.edge(
                &[(tx + *sgn * dw / 2.0, cy), (*sx, cy), (*sx, merge2)],
                false,
            );
            self.labels.push(Label {
                x: tx + *sgn * (dw / 2.0 + self.st.grid),
                y: cy - self.st.label_dy,
                text: lbl.clone(),
                ha: "center".into(),
            });
            cols.push((*sx, merge2));
        }
        self.merge_bus(&cols, tx, merge2);
        // рельсы break из веток внутри кейсов (if внутри case): наружу
        // за колонки switch, вниз к слиянию switch, T-стык на стволе
        if is_switch {
            let nested: Vec<BreakAt> = self.breaks.drain(br_mark..).collect();
            let outer = env.max(dw / 2.0 + 2.0 * self.st.grid) + self.st.grid;
            for b in &nested {
                let slot = self.break_slot;
                self.break_slot += 1;
                let sgn = if b.tx <= tx { -1.0 } else { 1.0 };
                let rx = tx
                    + sgn
                        * super::geometry::up(
                            outer + slot as f64 * 2.0 * self.st.grid,
                            self.st.grid,
                        );
                self.edge(&[(b.tx, b.y), (rx, b.y), (rx, merge2), (tx, merge2)], false);
            }
        }
        if !rails.is_empty() {
            merge2 = merge2.max(y_b + 2.0 * self.st.grid);
            // рельса жмётся к под-ромбу: пол — вершина + 2g, дальше —
            // ровно за внешний край своих колонок
            let clear = (env + self.st.grid).max(dw / 2.0 + 2.0 * self.st.grid);
            let bx2 = tx + super::geometry::up(clear, self.st.grid);
            for (k, lbl) in rails.iter().enumerate() {
                self.edge(
                    &[
                        vr,
                        (bx2 + k as f64 * 2.0 * self.st.grid, cy),
                        (bx2 + k as f64 * 2.0 * self.st.grid, merge2),
                        (tx, merge2),
                    ],
                    false,
                );
                self.labels.push(Label {
                    x: tx + dw / 2.0 + self.st.label_exit_dx,
                    y: cy - self.st.label_dy,
                    text: lbl.clone(),
                    ha: "center".into(),
                });
            }
        }
        let all_dead = !bottoms.is_empty()
            && bottoms.iter().all(|&(_, _, e)| e == ColEnd::Return)
            && empty.is_empty();
        // все ветки ушли рельсами: продолжение колонки недостижимо,
        // разве что есть пустые ветки — те падают сквозь if
        let all_rail = !all_dead
            && !bottoms.is_empty()
            && bottoms.iter().all(|&(_, _, e)| e == ColEnd::Rail)
            && empty.is_empty();
        let end = if all_dead {
            ColEnd::Return
        } else if all_rail {
            ColEnd::Rail
        } else {
            ColEnd::Flow
        };
        // Низ продолжения: ниже всех колонок, но не ниже шины слияния.
        // Ветка, ушедшая рельсой (break/continue), плиток не рисует, но её
        // низ учитывается — иначе спуск продолжения проткнул бы её плитки.
        let cont = bottoms
            .iter()
            .map(|&(_, y, _)| y)
            .fold(y_b, f64::max)
            .max(merge2);
        // Поток на ось колонки приходит по шине на высоте merge2, а
        // продолжение начинается ниже — у ветки с рельсой или return низ
        // колонки уходит под шину. Без этого стыка вертикаль обрывается
        // в воздухе и стрелка в следующий блок висит без входа.
        // Продолжение на оси есть только у живого потока: если все ветки
        // ушли рельсами или тупиком, тянуть ствол вниз некуда — конец
        // линии повисает в воздухе между плитками соседних колонок.
        if cont > merge2 && end == ColEnd::Flow {
            self.edge(&[(tx, merge2), (tx, cont)], false);
        }
        (cont, end)
    }
}
