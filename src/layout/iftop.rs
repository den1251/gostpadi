//! Ромб «если»/переключатель на основной линии: колонки веток,
//! метки да/нет/кейсов, слияние, рельсы «-> конец» (pend) и кружки link.

use super::column::rail_only;
use super::ctx::{BreakAt, ColEnd, Ctx, Pend};
use super::ifnode::{cascade_cols, pack_plan, single_plan, Side};
use super::types::{Anchor, Label};
use crate::ir::{Branch, Node, NodeKind, Stmt};

impl Ctx<'_> {
    /// Полоса рельсы «-> конец»: снаружи всех колонок схемы. env — край
    /// своих колонок; запас по ярусам (max_tier) и общий nhe страхуют от
    /// чужих фигур ниже, которые рельса проходит насквозь.
    fn rail_lane(&self, env: f64) -> f64 {
        let pitch = 2.0 * self.nhe + self.st.colgap;
        let lane =
            self.nhe + self.st.hgap + self.colw / 2.0 + self.st.rail + self.max_tier as f64 * pitch;
        env.max(lane)
    }

    /// Ромб «если»/переключатель на основной линии.
    pub(super) fn decision_top(
        &mut self,
        nd: &Node,
        prev: Option<(f64, f64)>,
        cursor: f64,
    ) -> (Option<(f64, f64)>, f64) {
        if matches!(cascade_cols(nd), Some((k, _)) if k >= 2) {
            return self.decision_cascade(nd, prev, cursor);
        }
        // большой переключатель: кейсы сеткой, по два на ряд
        let n_ne = nd.branches.iter().filter(|b| !b.stmts.is_empty()).count();
        if nd.switch_var.is_some() && n_ne >= 5 {
            return self.switch_rows(nd, prev, cursor);
        }
        let (dw, dh) = self.sizes["if"];
        let cy = cursor + self.vg() + dh / 2.0;
        self.add("if", 0.0, cy, &nd.text);
        if let Some(p) = prev {
            self.edge(&[p, (0.0, cy - dh / 2.0)], true);
        }
        let vl = (-dw / 2.0, cy);
        let vr = (dw / 2.0, cy);
        let vb = (0.0, cy + dh / 2.0);
        let idxs: Vec<usize> = (0..nd.branches.len())
            .filter(|&i| !nd.branches[i].stmts.is_empty())
            .collect();
        let empty: Vec<String> = nd
            .branches
            .iter()
            .filter(|b| b.stmts.is_empty())
            .map(|b| b.label.clone())
            .collect();
        let n = idxs.len();
        let is_switch = nd.switch_var.is_some();
        let comb = is_switch && n >= 2;
        let top0 = cy + dh / 2.0 + self.vg();
        let y_b = cy + dh / 2.0;
        let ext: Vec<(f64, f64)> = idxs
            .iter()
            .map(|&i| self.extent_lr(&nd.branches[i].stmts))
            .collect();
        let hinge = dw / 2.0 + self.st.hgap;
        let scoped = is_switch || self.loop_depth > 0 || self.switch_depth > 0;
        // ветка из одних break/continue ВНУТРИ scope: колонки нет, рельса
        // идёт от боковой вершины ромба. Вне scope break рисуется плиткой —
        // такая колонка обязана быть нормальной ширины.
        let narrow: Vec<bool> = if scoped {
            idxs.iter()
                .map(|&i| rail_only(&nd.branches[i].stmts))
                .collect()
        } else {
            Vec::new()
        };
        // switch читается слева направо, у ромба «да» — справа
        let (plan, envl, envr) = if n == 1 && !empty.is_empty() {
            single_plan(
                idxs[0],
                ext[0],
                narrow.first().copied().unwrap_or(false),
                hinge,
                dw / 2.0,
                0.0,
            )
        } else {
            pack_plan(
                &idxs,
                &ext,
                &narrow,
                hinge,
                dw / 2.0,
                self.st.colgap,
                0.0,
                !is_switch,
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
        let mut exits: Vec<Option<(f64, f64, bool, ColEnd)>> = vec![None; nd.branches.len()];
        for &(bi, side, t2, txx) in &plan {
            let arrow = !(scoped && rail_only(&nd.branches[bi].stmts));
            self.column_entry(
                side,
                t2,
                txx,
                cy,
                y_b,
                top0,
                comb,
                vl,
                vr,
                &nd.branches[bi].label,
                arrow,
            );
            let (yend, end) = self.render_column(&nd.branches[bi].stmts, txx, top0);
            exits[bi] = Some((
                txx,
                yend,
                nd.branches[bi].to_end && end != ColEnd::Return,
                end,
            ));
        }
        if is_switch {
            self.switch_depth -= 1;
        }
        self.case_direct = saved_direct;
        let mut merge_y = y_b;
        for &(bi, side, ..) in &plan {
            let e = exits[bi].unwrap();
            // Колонка на оси (tx = 0) стоит на стволе продолжения: её низ —
            // живой, мёртвый или «-> конец» — задаёт merge_y, иначе ствол
            // (0, merge_y) → следующий блок протыкает её содержимое.
            // Внесъёмные колонки (tx != 0) ствол не трогают.
            if side == Side::Axis || (!e.2 && e.3 == ColEnd::Flow) {
                merge_y = merge_y.max(e.1 + self.st.mgap);
            }
        }
        // Пустая ветка простого if — сквозной проход: «нет» идёт по стволу
        // вниз, колонку не занимает (канон методички, рис. 1). Пустой кейс
        // switch — отдельная ветвь, он уходит рельсой справа.
        let trunk: Vec<String> = if is_switch { Vec::new() } else { empty.clone() };
        let rails: Vec<String> = if is_switch { empty.clone() } else { Vec::new() };
        if !trunk.is_empty() {
            merge_y = merge_y.max(y_b + 2.0 * self.st.grid);
            self.edge(&[vb, (0.0, merge_y)], false);
            for (k, lbl) in trunk.iter().enumerate() {
                self.labels.push(Label {
                    x: self.st.label_axis_dx,
                    y: y_b + (2.0 + k as f64) * self.st.grid,
                    text: lbl.clone(),
                    ha: "left".into(),
                });
            }
        }
        let col_bottom = plan
            .iter()
            .map(|&(bi, ..)| exits[bi].unwrap().1)
            .fold(y_b, f64::max);
        let mut link_bottom = y_b;
        let mut merge_cols: Vec<(f64, f64)> = Vec::new();
        for &(bi, side, _, _) in &plan {
            let e = exits[bi].unwrap();
            if !e.2 {
                if e.3 == ColEnd::Flow {
                    merge_cols.push((e.0, e.1));
                }
                continue;
            }
            // «-> конец»: рельса снаружи колонок всей схемы, кратно сетке
            let (sgn, key) = if side == Side::L {
                (-1.0, 0usize)
            } else {
                (1.0, 1usize)
            };
            let rail = sgn
                * super::geometry::up(
                    self.rail_lane(env) + self.pend_count[key] as f64 * self.st.rail_step,
                    self.st.grid,
                );
            self.pend_count[key] += 1;
            if let Some(letter) = nd.branches[bi].link {
                // «конец» на другом листе: кружок-соединитель у ветки
                let ccy = col_bottom + self.st.jog + self.vg() + self.st.conn_r;
                self.add("conn", rail, ccy, &letter.to_string());
                self.edge(
                    &[
                        (e.0, e.1),
                        (e.0, col_bottom + self.st.jog),
                        (rail, col_bottom + self.st.jog),
                        (rail, ccy - self.st.conn_r),
                    ],
                    true,
                );
                link_bottom = ccy + self.st.conn_r;
            } else {
                self.pend.push(Pend {
                    x: e.0,
                    y: e.1,
                    cb: col_bottom,
                    rail,
                });
            }
        }
        // шина слияния: один горизонтальный отрезок вместо наложенных
        // хвостов колонок
        self.merge_bus(&merge_cols, 0.0, merge_y);
        let n_merge = plan
            .iter()
            .filter(|&&(bi, ..)| {
                let e = exits[bi].unwrap();
                !e.2 && e.3 == ColEnd::Flow
            })
            .count()
            + rails.len()
            + trunk.len();
        let has_axis = plan.iter().any(|p| p.1 == Side::Axis);
        // Рельса пустого кейса жмётся к ромбу: пол — правая вершина + 2g,
        // дальше вправо — только чтобы очистить правые колонки. Старая
        // «зеркальность самой широкой стороне» при пустом правом боку
        // давала крюк через всю схему: шина росла без контента.
        let clear = (env + self.st.grid).max(dw / 2.0 + 2.0 * self.st.grid);
        for (k, lbl) in rails.iter().enumerate() {
            merge_y = merge_y.max(y_b + 2.0 * self.st.grid);
            let bx = super::geometry::up(clear, self.st.grid) + k as f64 * 2.0 * self.st.grid;
            self.edge(&[vr, (bx, cy), (bx, merge_y), (0.0, merge_y)], false);
            self.labels.push(Label {
                x: dw / 2.0 + self.st.label_exit_dx + k as f64 * 2.0 * self.st.grid,
                y: cy - self.st.label_dy,
                text: lbl.clone(),
                ha: "center".into(),
            });
        }
        if n_merge == 0 && !has_axis {
            self.edge(&[vb, (0.0, merge_y)], false);
        }
        // рельсы break из веток switch (if внутри case): наружу за
        // колонки, вниз к выходу switch, T-стык на стволе
        if is_switch {
            let nested: Vec<BreakAt> = self.breaks.drain(br_mark..).collect();
            let outer = env.max(dw / 2.0 + 2.0 * self.st.grid) + self.st.grid;
            for b in &nested {
                let slot = self.break_slot;
                self.break_slot += 1;
                let sgn = if b.tx <= 0.0 { -1.0 } else { 1.0 };
                let rx = sgn
                    * super::geometry::up(outer + slot as f64 * 2.0 * self.st.grid, self.st.grid);
                self.edge(
                    &[(b.tx, b.y), (rx, b.y), (rx, merge_y), (0.0, merge_y)],
                    false,
                );
            }
        }
        let cursor = merge_y.max(col_bottom).max(link_bottom);
        self.anchors.push(Anchor { x: 0.0, y: cursor });
        (Some((0.0, merge_y)), cursor)
    }

    /// Большой переключатель: кейсы сеткой — по два на ряд, ряды друг
    /// под другом. Ширина постоянная (две колонки на ±base), растёт
    /// только вниз — одна длинная шина кейсов ужимала всю страницу.
    /// Шины рядов соединяет ствол на оси; слияние живых колонок —
    /// одна шина под последним рядом.
    fn switch_rows(
        &mut self,
        nd: &Node,
        prev: Option<(f64, f64)>,
        cursor: f64,
    ) -> (Option<(f64, f64)>, f64) {
        let (dw, dh) = self.sizes["if"];
        let cy = cursor + self.vg() + dh / 2.0;
        self.add("if", 0.0, cy, &nd.text);
        if let Some(p) = prev {
            self.edge(&[p, (0.0, cy - dh / 2.0)], true);
        }
        let saved_direct = self.case_direct;
        let br_mark = self.breaks.len();
        self.switch_depth += 1;
        self.case_direct = true;
        let empty: Vec<String> = nd
            .branches
            .iter()
            .filter(|b| b.stmts.is_empty())
            .map(|b| b.label.clone())
            .collect();
        let cases: Vec<&Branch> = nd.branches.iter().filter(|b| !b.stmts.is_empty()).collect();
        // Абсциссы кейсов — по их собственному содержимому: колонка встаёт
        // вплотную к ромбу, шина ряда не тянется через пол-листа (на
        // strany.c общий nhe давал шину в 900pt на плитку в 150).
        // Нечётный хвост — на оси: отдельная колонка ему не нужна.
        let hinge = dw / 2.0 + self.st.hgap;
        let ext: Vec<(f64, f64)> = cases.iter().map(|b| self.extent_lr(&b.stmts)).collect();
        let mut xs = vec![0.0f64; cases.len()];
        for (ri, row) in cases.chunks(2).enumerate() {
            if row.len() == 1 {
                xs[ri * 2] = 0.0;
            } else {
                xs[ri * 2] = -(hinge + ext[ri * 2].1);
                xs[ri * 2 + 1] = hinge + ext[ri * 2 + 1].0;
            }
        }
        let env = cases
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let (l, r) = ext[i];
                if xs[i] >= 0.0 {
                    xs[i] + r
                } else {
                    -xs[i] + l
                }
            })
            .fold(dw / 2.0, f64::max);
        // коридоры за сеткой: сначала рельсы пустых кейсов, потом
        // «-> конец» — чтобы вертикали не накладывались
        let bx0 = super::geometry::up(
            (env + self.st.grid).max(dw / 2.0 + 2.0 * self.st.grid),
            self.st.grid,
        );
        // шина первого ряда: от нижней вершины ромба по стволу
        let mut y_bus = cy + dh / 2.0 + self.vg();
        let mut trunk_from = cy + dh / 2.0;
        self.edge(&[(0.0, trunk_from), (0.0, y_bus)], false);
        let mut col_bottom = y_bus;
        let mut merge_y = y_bus;
        let mut link_bottom = y_bus;
        let mut merged = false;
        for (ri, row) in cases.chunks(2).enumerate() {
            if ri > 0 {
                // Шина ряда питается от оси switch, а не от слияния прошлого
                // ряда: слив и вход следующего ряда — разные высоты, иначе
                // выходы кейсов ряда 1 читаются как вход кейсов ряда 2.
                y_bus = merge_y + self.vg();
                self.edge(&[(0.0, merge_y), (0.0, y_bus)], false);
                trunk_from = y_bus;
            }
            let mut live: Vec<f64> = Vec::new();
            let mut gone: Vec<(f64, f64, bool, Option<char>)> = Vec::new(); // x, yend, to_end, link
            for (ci, b) in row.iter().enumerate() {
                let left = ci == 0 && row.len() > 1;
                let tx = xs[ri * 2 + ci];
                let top = y_bus + self.vg();
                // шина ряда: от ствола до колонки (два сегмента без
                // наложения — вместе образуют шину через ось); кейс на
                // оси входа не требует — его продолжает сам ствол
                if tx.abs() > 1e-9 {
                    self.edge(&[(0.0, y_bus), (tx, y_bus)], false);
                }
                self.edge(&[(tx, y_bus), (tx, top)], !rail_only(&b.stmts));
                self.labels.push(Label {
                    x: tx
                        + if left {
                            -self.st.label_dx
                        } else {
                            self.st.label_dx
                        },
                    y: top - self.st.label_dy,
                    text: b.label.clone(),
                    ha: if left { "right" } else { "left" }.into(),
                });
                let (yend, end) = self.render_column(&b.stmts, tx, top);
                col_bottom = col_bottom.max(yend);
                if end != ColEnd::Flow {
                    // тупик return или рельса break: слияния от колонки
                    // нет — break дорисует дренаж switch
                    continue;
                }
                if b.to_end {
                    gone.push((tx, yend, true, b.link));
                } else {
                    live.push(tx);
                    gone.push((tx, yend, false, None));
                }
            }
            // слияние ряда: уровень ниже содержимого ряда; капли колонок
            // не могут спускаться ниже — под ними следующие ряды сетки
            merge_y = gone
                .iter()
                .map(|g| g.1)
                .fold(y_bus + 2.0 * self.st.grid, f64::max)
                + self.st.mgap;
            for g in &gone {
                if g.2 {
                    continue; // «-> конец» уходит своим коридором ниже
                }
                self.edge(&[(g.0, g.1), (g.0, merge_y)], false);
            }
            if !live.is_empty() {
                let lo = live.iter().cloned().fold(f64::MAX, f64::min);
                let hi = live.iter().cloned().fold(f64::MIN, f64::max);
                self.edge(&[(lo.min(0.0), merge_y), (hi.max(0.0), merge_y)], false);
                merged = true;
            }
            // ствол на оси между колонками ряда: связь ромба с шиной
            // следующего ряда. У нечётного хвоста на оси стоит его
            // колонка — там ствол не рисуется.
            if row.len() > 1 {
                self.edge(&[(0.0, y_bus), (0.0, merge_y)], false);
            }
            // «-> конец»: спуск до уровня слияния ряда, наружу за сетку,
            // дальше рельсу дорисует layout() над «концом»
            for &(tx, yend, _, link) in gone.iter().filter(|g| g.2) {
                let sgn: f64 = if tx > 0.0 { 1.0 } else { -1.0 };
                let key = usize::from(tx > 0.0);
                let out = sgn * (bx0 + empty.len() as f64 * 2.0 * self.st.grid);
                self.edge(&[(tx, yend), (tx, merge_y), (out, merge_y)], false);
                let rail = sgn
                    * super::geometry::up(
                        self.rail_lane(env) + self.pend_count[key] as f64 * self.st.rail_step,
                        self.st.grid,
                    );
                self.pend_count[key] += 1;
                if let Some(letter) = link {
                    let ccy = col_bottom + self.st.jog + self.vg() + self.st.conn_r;
                    self.add("conn", rail, ccy, &letter.to_string());
                    self.edge(
                        &[
                            (out, merge_y),
                            (out, col_bottom + self.st.jog),
                            (rail, col_bottom + self.st.jog),
                            (rail, ccy - self.st.conn_r),
                        ],
                        true,
                    );
                    link_bottom = link_bottom.max(ccy + self.st.conn_r);
                } else {
                    self.pend.push(Pend {
                        x: out,
                        y: merge_y,
                        cb: col_bottom,
                        rail,
                    });
                }
            }
        }
        // пустые кейсы: рельсы справа, чистят сетку, сливаются на стволе
        if !empty.is_empty() {
            for (k, lbl) in empty.iter().enumerate() {
                let bx = bx0 + k as f64 * 2.0 * self.st.grid;
                self.edge(
                    &[(dw / 2.0, cy), (bx, cy), (bx, merge_y), (0.0, merge_y)],
                    false,
                );
                self.labels.push(Label {
                    x: dw / 2.0 + self.st.label_exit_dx + k as f64 * 2.0 * self.st.grid,
                    y: cy - self.st.label_dy,
                    text: lbl.clone(),
                    ha: "center".into(),
                });
            }
            merged = true;
        }
        if !merged {
            // все живые кейсы в тупиках/«-> конец»: формальное
            // продолжение ствола до точки выхода
            self.edge(&[(0.0, trunk_from), (0.0, merge_y)], false);
        }
        // рельсы break из веток внутри кейсов (if внутри case): наружу
        // за сетку, вниз к выходу switch, T-стык на стволе
        let nested: Vec<BreakAt> = self.breaks.drain(br_mark..).collect();
        self.switch_depth -= 1;
        self.case_direct = saved_direct;
        let outer = env + self.st.grid;
        for b in &nested {
            let slot = self.break_slot;
            self.break_slot += 1;
            let sgn = if b.tx <= 0.0 { -1.0 } else { 1.0 };
            let rx =
                sgn * super::geometry::up(outer + slot as f64 * 2.0 * self.st.grid, self.st.grid);
            self.edge(
                &[(b.tx, b.y), (rx, b.y), (rx, merge_y), (0.0, merge_y)],
                false,
            );
        }
        let cursor = merge_y.max(col_bottom).max(link_bottom);
        self.anchors.push(Anchor { x: 0.0, y: cursor });
        (Some((0.0, merge_y)), cursor)
    }

    /// Каскад else-if: вертикальный ствол с одной шиной. Ромбы цепочки
    /// на оси друг под другом («нет» — ребро от нижней вершины к верхней,
    /// метка справа от линии), «да» каждого ромба — колонка СПРАВА
    /// (канон методички), все колонки на одном top0; хвост-else — слева,
    /// пустой хвост — рельса.
    /// Слияние: спуски живых колонок и ровно одна горизонтальная шина.
    fn decision_cascade(
        &mut self,
        nd: &Node,
        prev: Option<(f64, f64)>,
        cursor: f64,
    ) -> (Option<(f64, f64)>, f64) {
        let (dw, dh) = self.sizes["if"];
        // развёртка цепочки: (ромб, да-ветка) и хвостовая ветка последнего
        let mut links: Vec<(&Node, &Branch)> = Vec::new();
        let mut cur = nd;
        let tail: &Branch = loop {
            links.push((cur, &cur.branches[0]));
            let last = cur.branches.last().unwrap();
            match last.stmts.as_slice() {
                [Stmt::Node(d)] if d.kind == NodeKind::Decision => cur = d,
                _ => break last,
            }
        };
        let k = links.len();
        let step = dh + self.vg();
        let cy0 = cursor + self.vg() + dh / 2.0;
        for (i, (dnd, _)) in links.iter().enumerate() {
            self.add("if", 0.0, cy0 + step * i as f64, &dnd.text);
        }
        if let Some(p) = prev {
            self.edge(&[p, (0.0, cy0 - dh / 2.0)], true);
        }
        // ствол продолжения: «нет» ведёт к следующему ромбу
        for i in 1..k {
            let y0 = cy0 + step * (i - 1) as f64 + dh / 2.0;
            self.edge(&[(0.0, y0), (0.0, y0 + self.vg())], true);
            self.labels.push(Label {
                x: self.st.label_axis_dx,
                y: y0 + self.vg() - self.st.label_dy,
                text: links[i - 1].0.branches.last().unwrap().label.clone(),
                ha: "left".into(),
            });
        }
        let top0 = cy0 + dh / 2.0 + self.vg();
        // Абсциссы колонок — по содержимому своих ветвей (как в остальных
        // ромбах): общий nhe тянул шину через пол-листа.
        let hinge = dw / 2.0 + self.st.hgap;
        let exts: Vec<(f64, f64)> = links
            .iter()
            .map(|(_, b)| self.extent_lr(&b.stmts))
            .collect();
        let tail_ext = if tail.stmts.is_empty() {
            (0.0, 0.0)
        } else {
            self.extent_lr(&tail.stmts)
        };
        let mut xs = vec![0.0f64; exts.len() + 1];
        let mut edge = hinge;
        for (ci, &(l, r)) in exts.iter().enumerate() {
            xs[ci] = edge + l;
            edge = xs[ci] + r + self.st.colgap;
        }
        xs[k] = -(hinge + tail_ext.1);
        let env = exts
            .iter()
            .enumerate()
            .map(|(ci, &(_, r))| xs[ci] + r)
            .fold(hinge + tail_ext.0, f64::max);
        let y_b_last = cy0 + step * (k - 1) as f64 + dh / 2.0;
        let n_cols = k + usize::from(!tail.stmts.is_empty());
        let mut exits: Vec<(f64, f64, ColEnd, bool, Option<char>, bool)> = Vec::new();
        // да-входы: «гребёнка» на каждой стороне. Общий вертикальный
        // участок в коридоре между кромкой ромбов и колонками
        // (dw/2 + g — внутри полосы hgap = 2g), у каждой ветви своя
        // высота горизонтального обхода НАД колонками (top0 - (ci+1)*g),
        // спуск в свою колонку сверху, стрелка в плитку. Высоты колонок
        // не мешают: длинные горизонтали только над верхом колонок.
        let x_v = dw / 2.0 + self.st.grid;
        // высота обхода над колонками: у да-колонок к лестнице (чтобы
        // горизонтали не наложились), у единственного хвоста — своя
        let y_over = |ci: usize| {
            if ci >= k {
                top0 - self.st.grid
            } else {
                top0 - (ci + 1) as f64 * self.st.grid
            }
        };
        let mut ins: Vec<(f64, f64, f64, usize)> = Vec::new(); // sgn, tx, cy, ci
        for ci in 0..n_cols {
            // канон методички: «да» каждого ромба — вправо, «нет» — вниз
            // по стволу к следующему ромбу; хвост-else — слева.
            let sgn: f64 = if ci < k { 1.0 } else { -1.0 };
            let tx = xs[ci];
            let cy = cy0 + step * ci.min(k - 1) as f64;
            ins.push((sgn, tx, cy, ci));
        }
        // ветки каскада — не кейс-колонки: break в них не растворяется
        let saved_direct = self.case_direct;
        self.case_direct = false;
        // общий вертикальный участок стороны: от высшей точки крепления
        // (стык на cy и/или обход над колонками) до низшей, один раз;
        // ветви-ответвления в него не рисуются. Верх берётся и от cy:
        // стык первой левой колонки сидит на cy0, которая ВЫШЕ обхода
        // (top0 - 3g) — коридор, не накрывающий её, оставлял «yes»
        // первого ромба каскада оборванным в воздухе.
        for sgn in [-1.0, 1.0] {
            let side: Vec<&(f64, f64, f64, usize)> = ins.iter().filter(|e| e.0 == sgn).collect();
            if side.is_empty() {
                continue;
            }
            let y_top = side
                .iter()
                .map(|e| e.2.min(y_over(e.3)))
                .fold(f64::MAX, f64::min);
            let y_bot = side
                .iter()
                .map(|e| e.2.max(y_over(e.3)))
                .fold(f64::MIN, f64::max);
            self.edge(&[(sgn * x_v, y_top), (sgn * x_v, y_bot)], false);
        }
        for &(sgn, tx, cy, ci) in &ins {
            // ветвь: боковая вершина -> гребёнка -> обход над колонками ->
            // спуск в свою колонку сверху. Колонка хвоста стоит на своём
            // месте по той же схеме: спуск сверху, а не сквозь плитку.
            let ybr = if ci < k { links[ci].1 } else { tail };
            self.edge(&[(sgn * dw / 2.0, cy), (sgn * x_v, cy)], false);
            self.edge(&[(sgn * x_v, y_over(ci)), (tx, y_over(ci))], false);
            self.edge(
                &[(tx, y_over(ci)), (tx, top0)],
                !(rail_only(&ybr.stmts) && self.loop_depth + self.switch_depth > 0),
            );
            self.labels.push(Label {
                x: sgn * (dw / 2.0 + self.st.label_exit_dx),
                y: cy - self.st.label_dy,
                text: ybr.label.clone(),
                ha: "center".into(),
            });
            let (yend, end) = self.render_column(&ybr.stmts, tx, top0);
            exits.push((tx, yend, end, ybr.to_end, ybr.link, sgn < 0.0));
        }
        self.case_direct = saved_direct;
        let col_bottom = exits.iter().map(|e| e.1).fold(y_b_last, f64::max);
        let mut merge_y = y_b_last + 2.0 * self.st.grid;
        for &(_, yend, end, to_end, _, _) in &exits {
            if end == ColEnd::Flow && !to_end {
                merge_y = merge_y.max(yend + self.st.mgap);
            }
        }
        // единая шина: спуск каждой живой колонки и ровно один
        // горизонтальный сегмент от крайней левой до крайней правой
        let mut bus_xs: Vec<f64> = vec![0.0];
        for &(tx, yend, end, to_end, _, _) in &exits {
            if end == ColEnd::Flow && !to_end {
                self.edge(&[(tx, yend), (tx, merge_y)], false);
                bus_xs.push(tx);
            }
        }
        // «-> конец»: рельса снаружи колонок всей схемы, кружки link
        let mut link_bottom = y_b_last;
        for &(tx, yend, end, to_end, link, left) in &exits {
            if !to_end || end != ColEnd::Flow {
                continue;
            }
            let key = usize::from(!left);
            let sgn: f64 = if left { -1.0 } else { 1.0 };
            let rail = sgn
                * super::geometry::up(
                    self.rail_lane(env) + self.pend_count[key] as f64 * self.st.rail_step,
                    self.st.grid,
                );
            self.pend_count[key] += 1;
            if let Some(letter) = link {
                let ccy = col_bottom + self.st.jog + self.vg() + self.st.conn_r;
                self.add("conn", rail, ccy, &letter.to_string());
                self.edge(
                    &[
                        (tx, yend),
                        (tx, col_bottom + self.st.jog),
                        (rail, col_bottom + self.st.jog),
                        (rail, ccy - self.st.conn_r),
                    ],
                    true,
                );
                link_bottom = ccy + self.st.conn_r;
            } else {
                self.pend.push(Pend {
                    x: tx,
                    y: yend,
                    cb: col_bottom,
                    rail,
                });
            }
        }
        // пустой хвост: рельса «нет» на левой стороне (да-колонки всегда
        // справа — левая свободна), жмётся к ромбу и чистит свои колонки;
        // свою горизонталь не рисует — её накроет шина
        let has_rail = tail.stmts.is_empty();
        if has_rail {
            let sgn: f64 = -1.0;
            let clear = exits
                .iter()
                .filter(|e| e.5)
                .map(|e| e.0.abs() + tail_ext.0 + self.st.grid)
                .fold(dw / 2.0 + 2.0 * self.st.grid, f64::max);
            let bx = super::geometry::up(clear, self.st.grid);
            let cy = cy0 + step * (k - 1) as f64;
            self.edge(
                &[(sgn * dw / 2.0, cy), (sgn * bx, cy), (sgn * bx, merge_y)],
                false,
            );
            bus_xs.push(sgn * bx);
            self.labels.push(Label {
                x: sgn * (dw / 2.0 + self.st.label_exit_dx),
                y: cy - self.st.label_dy,
                text: tail.label.clone(),
                ha: "center".into(),
            });
        }
        let lo = bus_xs.iter().cloned().fold(f64::MAX, f64::min);
        let hi = bus_xs.iter().cloned().fold(f64::MIN, f64::max);
        if hi - lo > 1e-9 {
            self.edge(&[(lo, merge_y), (hi, merge_y)], false);
        }
        let n_merge =
            exits.iter().filter(|e| e.2 == ColEnd::Flow && !e.3).count() + usize::from(has_rail);
        if n_merge == 0 {
            // всё в тупиках: формальное продолжение ствола под ромбами
            self.edge(&[(0.0, y_b_last), (0.0, merge_y)], false);
        }
        let cursor = merge_y.max(col_bottom).max(link_bottom);
        self.anchors.push(Anchor { x: 0.0, y: cursor });
        (Some((0.0, merge_y)), cursor)
    }
}
