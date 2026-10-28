use super::ctx::{BreakAt, ColEnd, Ctx};
use super::measure::{no_sentinel, HEAD, MID, TAIL};
use super::types::{Anchor, Label, Shape};
use crate::ir::{Node, NodeKind, Stmt};

/// Кусок разрезанного цикла: split заменяет цикл парой узлов того же
/// kind, а режим помечает сентинелом в начале текста. Так лист N несёт
/// верхнюю трапецию, лист N+1 — нижнюю с номером, а середины — только тело.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Piece {
    /// целый цикл
    Full,
    /// голова: верхняя трапеция и начало тела
    Head,
    /// середина: только тело
    Mid,
    /// хвост: конец тела и нижняя трапеция с номером
    Tail,
}

fn piece(text: &str) -> Piece {
    match text.chars().next() {
        Some(HEAD) => Piece::Head,
        Some(MID) => Piece::Mid,
        Some(TAIL) => Piece::Tail,
        _ => Piece::Full,
    }
}

/// Сколько уровней циклов ещё вложено в тело: внешний цикл получает
/// коридор на столько шагов дальше вложенного, сколько под ним уровней.
/// Счёт по вложенности, а не глобальный: иначе коридор внешнего цикла
/// уезжал за край листа (labfix-06: 425pt левее оси из-за слотов switch).
fn nested_loops(items: &[Stmt]) -> usize {
    items
        .iter()
        .map(|it| match it {
            Stmt::Node(nd) if nd.kind == NodeKind::Loop => {
                1 + nd.body.as_deref().map(nested_loops).unwrap_or(0)
            }
            Stmt::Node(nd) => nd
                .branches
                .iter()
                .map(|b| nested_loops(&b.stmts))
                .max()
                .unwrap_or(0),
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

/// Точка ухода рельсы на коридор. `by` — высота горизонтали: для
/// плитки на полшага ниже её низа, иначе верх пустой колонки.
struct Rail {
    sx: f64,
    sy: f64,
    by: f64,
    /// continue, а не break: при обоих смыслах в одном коридоре
    /// горизонтали разводятся по высоте, иначе две ветви сливаются
    /// в одну линию и не читаются
    cont: bool,
}

impl Ctx<'_> {
    /// Цикл на основной линии: entry-стрелка сверху, две трапеции
    /// (loop_begin сверху, loop_end снизу), тело между ними.
    pub(super) fn loop_top(
        &mut self,
        nd: &Node,
        prev: Option<(f64, f64)>,
        cursor: f64,
    ) -> (Option<(f64, f64)>, f64) {
        if let Some(p) = prev {
            self.edge(&[p, (0.0, cursor + self.vg())], true);
        }
        let (merge, _) = self.sub_loop(nd, 0.0, cursor + self.vg());
        self.anchors.push(Anchor { x: 0.0, y: merge });
        (Some((0.0, merge)), merge)
    }

    /// Цикл внутри колонки. Возвращает (y продолжения, конец): из двух
    /// трапеций поток всегда продолжается вниз (ColEnd::Flow), разве что
    /// тело целиком в тупике return.
    pub(super) fn sub_loop(&mut self, nd: &Node, tx: f64, top: f64) -> (f64, ColEnd) {
        let (lw, lh) = self.sizes["loop"];
        let p = piece(&nd.text);
        // трапеции только у целого цикла и у его краёв: середина и
        // продолжение живут на соседних листах, их связывает кружок
        let begins = matches!(p, Piece::Full | Piece::Head);
        let ends = matches!(p, Piece::Full | Piece::Tail);
        let num = self.loop_depth + 1;
        // коридоры этого цикла: база — число вложенных уровней, счётчики
        // сторон свои (вложенный цикл вернёт родительские значения)
        let saved_lanes = (self.lane_base, self.lane_left, self.lane_right);
        self.lane_base = nd.body.as_deref().map(nested_loops).unwrap_or(0);
        self.lane_left = 0;
        self.lane_right = 0;
        let cy1 = top + lh / 2.0;
        if begins {
            // Номер на обеих трапециях: по нему верх и низ цикла
            // сопоставляются на листе (раньше низ нёс голый номер, а
            // верх — условие без номера).
            let label = format!("{num}. {}", no_sentinel(&nd.loop_label()));
            self.add("loop_begin", tx, cy1, &label);
        }
        self.loop_depth += 1;
        // коридор: шире половины трапеции и любого вложенного содержимого,
        // чтобы рельсы break/continue не задевали фигуры
        let top0 = if begins {
            cy1 + lh / 2.0 + self.vg()
        } else {
            top
        };
        let mark = self.breaks.len();
        let mark_c = self.continues.len();
        let mark_sh = self.shapes.len();
        let saved_direct = self.case_direct;
        self.case_direct = false;
        let (yend, end) = match &nd.body {
            Some(body) => {
                if begins {
                    self.edge(&[(tx, cy1 + lh / 2.0), (tx, top0)], true);
                }
                self.render_column(body, tx, top0)
            }
            None if begins => (cy1 + lh / 2.0, ColEnd::Flow),
            None => (top, ColEnd::Flow),
        };
        self.case_direct = saved_direct;
        // нижняя трапеция: вход сверху (из тела), штатный выход вниз
        let cy2 = yend + self.vg() + lh / 2.0;
        if ends {
            self.add("loop_end", tx, cy2, &num.to_string());
            if end != ColEnd::Return {
                self.edge(&[(tx, yend), (tx, cy2 - lh / 2.0)], true);
            }
        }
        let merge = if ends {
            let merge = cy2 + lh / 2.0 + self.st.mgap;
            self.edge(&[(tx, cy2 + lh / 2.0), (tx, merge)], false);
            merge
        } else {
            // голова/середина: поток листа уходит вниз кружком-выходом,
            // его нарисует следующий узел части — от этого же низа
            yend
        };
        // Фактический левый край тела: рельсы дренажа (continue/break)
        // идут левее ВСЕХ нарисованных фигур тела. Мерять коридор от
        // nhe нельзя: вложенные if/switch дают колонки шире оценки
        // extent, и вертикаль рельсы садилась на плитку (u3.c: continue
        // внутри while -> ребро сквозь io-блок). Своим краем входит и
        // трапеция: рельса обязана обойти и её.
        let body_left = self.shapes[mark_sh..]
            .iter()
            .map(|s| s.cx - s.w / 2.0)
            .fold(f64::INFINITY, f64::min)
            .min(tx - lw / 2.0);
        // Симметричный правый край: рельса из правой колонки уходит в
        // правый коридор, а не через ствол налево (её горизонталь на
        // высоте низа плитки резала ствол цикла).
        let body_right = self.shapes[mark_sh..]
            .iter()
            .map(|s| s.cx + s.w / 2.0)
            .fold(f64::NEG_INFINITY, f64::max)
            .max(tx + lw / 2.0);
        // «clear»-проверка «рельса падает из центра плитки прямо на
        // merge» валидна только когда на вертикали плитки ниже неё нет
        // чужих фигур: две да-колонки разных if совпадают по абсциссе,
        // и прямой спуск протыкал нижнюю плитку (eof-брейк сквозь
        // ok = 1). Проверяем каждую вертикаль по телу цикла.
        let body_shapes: Vec<Shape> = self.shapes[mark_sh..].to_vec();
        let blocked = move |x: f64, y0: f64, y1: f64| {
            body_shapes.iter().any(|s| {
                (s.cy + s.h / 2.0 > y0 + 1.0)
                    && (s.cy - s.h / 2.0 < y1 - 1.0)
                    // x-диапазон с запасом 1pt: рельса не должна идти по
                    // боковой кромке фигуры (иначе линия сливается с
                    // границей блока)
                    && (s.cx + s.w / 2.0 > x - 1.0)
                    && (s.cx - s.w / 2.0 < x + 1.0)
            })
        };
        let breaks: Vec<BreakAt> = self.breaks.drain(mark..).collect();
        let conts: Vec<BreakAt> = self.continues.drain(mark_c..).collect();
        if ends {
            // break выходит из цикла: ниже loop_end, на продолжение.
            // continue — следующая итерация: входит в БОК loop_end,
            // выше его низа. Разные высоты, разные горизонтали.
            let yj = (cy2 + lh / 2.0 + merge) * 0.5;
            self.rails_to_end(
                &breaks,
                &conts,
                tx,
                lw,
                if begins { Some(cy1) } else { None },
                cy2,
                yj,
                body_left,
                body_right,
                &blocked,
            );
            // Обратная стрелка не рисуется: начало и конец тела
            // связывают одинаковые номера на трапециях (как в
            // ГОСТ-примерах и методичке).
            let _ = (begins, cy1, top0);
        } else {
            // голова/середина: выход листа — кружок под телом. Рельсы
            // сводятся на полшага ниже тела: на в-гэпе стоит верх
            // следующего блока, и горизонталь ложилась на его кромку
            // (labfix-09: рельса break по верхней грани io).
            //
            // Нижней трапеции на листе нет (голова или середина разреза):
            // continue и break оба уходят на следующий лист, но разными
            // высотами и с подписями — до бока loop_end на этом листе
            // не дотянуться.
            let y_exit = yend + self.st.grid;
            self.rails_to_page(&breaks, &conts, tx, y_exit, body_left, body_right);
        }
        self.loop_depth -= 1;
        (self.lane_base, self.lane_left, self.lane_right) = saved_lanes;
        (merge, ColEnd::Flow)
    }

    /// Рельсы стороны `right` сходятся в один коридор. Отрезки вертикали
    /// стыкуются T-узлами: у каждой рельсы свой отрезок, а не наложение
    /// хвостов (раньше все горизонтали шли до оси на одной высоте).
    fn rail_corridor(&mut self, mut items: Vec<Rail>, lane: f64, bottom: f64) {
        // break и continue в одном коридоре: если их горизонтали на одной
        // высоте, они сливаются в один отрезок и читаются как одна линия.
        // continue поднимаем на полшага — две ветви видны врозь.
        let mixed = items.iter().any(|r| r.cont) && items.iter().any(|r| !r.cont);
        if mixed {
            for r in &mut items {
                if r.cont {
                    // continue выше, break ниже: между горизонталями
                    // целая клетка, иначе на листе А4 они сливаются
                    if r.by > r.sy + 1e-9 {
                        r.by -= self.st.grid * 0.5;
                    } else {
                        r.by += self.st.grid * 0.5;
                    }
                } else {
                    r.by += self.st.grid * 0.5;
                }
            }
        }
        items.sort_by(|a, b| {
            b.by.partial_cmp(&a.by)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.sx.partial_cmp(&b.sx).unwrap_or(std::cmp::Ordering::Equal))
        });
        items.dedup_by(|a, b| a.sx == b.sx && a.sy == b.sy);
        let mut next = bottom;
        let mut i = 0;
        while i < items.len() {
            let by = items[i].by;
            let mut j = i;
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            while j < items.len() && (items[j].by - by).abs() < 1e-9 {
                let r = &items[j];
                // выход из плитки: вниз на полшага, иначе горизонталь
                // ложится на нижнюю грань плитки
                if by > r.sy + 1e-9 {
                    self.edge(&[(r.sx, r.sy), (r.sx, by)], false);
                }
                lo = lo.min(r.sx);
                hi = hi.max(r.sx);
                j += 1;
            }
            // Горизонтали с двух сторон от коридора: коридор может стоять
            // МЕЖДУ рельсами (середина if), и одна горизонталь от крайней
            // рельсы не дотянется до второй. Два отрезка сходятся в lane.
            if lo < lane - 1e-9 {
                self.edge(&[(lo, by), (lane, by)], false);
            }
            if hi > lane + 1e-9 {
                self.edge(&[(hi, by), (lane, by)], false);
            }
            if by < next - 1e-9 {
                self.edge(&[(lane, by), (lane, next)], false);
            }
            next = by;
            i = j;
        }
    }

    /// Следующий коридор стороны. Индекс = база вложенности + счётчик
    /// стороны: внешний цикл всегда на шаг дальше вложенного, соседи
    /// начинают счёт заново (их полосы не пересекаются по высоте).
    fn lane(&mut self, right: bool, left: f64, right_edge: f64) -> f64 {
        let local = if right {
            let l = self.lane_right;
            self.lane_right += 1;
            l
        } else {
            let l = self.lane_left;
            self.lane_left += 1;
            l
        };
        let idx = self.lane_base + local;
        if right {
            right_edge + self.st.grid * (idx as f64 + 1.0)
        } else {
            left - self.st.grid * (idx as f64 + 1.0)
        }
    }

    fn rail(&self, b: &BreakAt, cont: bool) -> Rail {
        let by = if b.from_tile { b.y + self.st.grid } else { b.y };
        Rail {
            sx: b.tx,
            sy: b.y,
            by,
            cont,
        }
    }

    /// Подпись рельсы снаружи коридора. Центрирование, а не start/end:
    /// границы листа считают полуширину подписи, и длинное слово не
    /// срезается краем viewBox.
    fn rail_label(&mut self, lane: f64, right: bool, y: f64, text: &str) {
        let half = text.chars().count() as f64 * self.st.char_w / 2.0;
        let x = if right {
            lane + self.st.label_exit_dx + half
        } else {
            lane - self.st.label_exit_dx - half
        };
        self.labels.push(Label {
            x,
            y: y - self.st.label_dy,
            text: text.into(),
            ha: "center".into(),
        });
    }

    /// Лист с нижней трапецией: continue входит в её бок (следующая
    /// итерация), break проходит ниже и вливается в продолжение после
    /// цикла.
    #[allow(clippy::too_many_arguments)]
    fn rails_to_end(
        &mut self,
        breaks: &[BreakAt],
        conts: &[BreakAt],
        tx: f64,
        lw: f64,
        head_y: Option<f64>,
        cy2: f64,
        yj: f64,
        left: f64,
        right: f64,
        blocked: &impl Fn(f64, f64, f64) -> bool,
    ) {
        // continue входит в БОК loop_end: середина вертикального участка
        // боковой кромки (ниже верхней кромки на h/4), а не в её верхний
        // угол — линия на кромке читалась как продолжение границы блока
        let lh = self.sizes["loop"].1;
        let cont_y = cy2 - lh / 4.0;
        for right_side in [false, true] {
            let bs: Vec<&BreakAt> = breaks
                .iter()
                .filter(|b| (b.tx > tx) == right_side)
                .collect();
            let cs: Vec<&BreakAt> = conts.iter().filter(|b| (b.tx > tx) == right_side).collect();
            if bs.is_empty() && cs.is_empty() {
                continue;
            }
            // Одинокая break-рельса из плитки вне трапеции: прямой спуск
            // по своей колонке короче коридора и не с чем пересекаться.
            if cs.is_empty() && bs.len() == 1 {
                let b = bs[0];
                let outside = b.tx < tx - lw / 2.0 || b.tx > tx + lw / 2.0;
                if b.from_tile && outside && !blocked(b.tx, b.y, yj) {
                    let r = self.rail(b, false);
                    self.edge(&[(r.sx, r.sy), (r.sx, yj), (tx, yj)], false);
                    continue;
                }
            }
            // Один break + один continue из плиток: общий коридор сливал
            // их в одну вертикаль, и на выходе она раздваивалась —
            // читается как «одна линия, потом две». Каноническое чтение:
            // continue возвращается к голове цикла (вверх по своей
            // стороне), break спускается вниз по своей колонке и уходит
            // под трапецию. Ветви не пересекаются и не сливаются.
            if let (Some(hy), true) = (head_y, bs.len() == 1 && cs.len() == 1) {
                let c = cs[0];
                let b = bs[0];
                let outside =
                    b.tx < tx - lw / 2.0 - 1.0 || b.tx > tx + lw / 2.0 + 1.0;
                if c.from_tile && b.from_tile && outside {
                    let lane = self.lane(right_side, left, right);
                    let by = c.y + self.st.grid * 0.5;
                    self.edge(&[(c.tx, c.y), (c.tx, by)], false);
                    self.edge(&[(c.tx, by), (lane, by)], false);
                    self.edge(&[(lane, by), (lane, hy)], false);
                    let hx = if right_side {
                        tx + lw / 2.0
                    } else {
                        tx - lw / 2.0
                    };
                    self.edge(&[(lane, hy), (hx, hy)], true);
                    // подпись вдоль вертикали, С ВНУТРЕННЕЙ стороны:
                    // снаружи она уходила за край листа (lane - half)
                    if !c.marked {
                        let half = "continue".chars().count() as f64 * self.st.char_w / 2.0;
                        let x = if right_side {
                            lane - self.st.label_exit_dx - half
                        } else {
                            lane + self.st.label_exit_dx + half
                        };
                        self.labels.push(Label {
                            x,
                            y: (by + hy) * 0.5,
                            text: "continue".into(),
                            ha: "center".into(),
                        });
                    }
                    self.edge(&[(b.tx, b.y), (b.tx, yj), (tx, yj)], false);
                    if !b.marked {
                        self.rail_label(b.tx, right_side, yj, "break");
                    }
                    continue;
                }
            }
            let lane = self.lane(right_side, left, right);
            let bottom = if bs.is_empty() { cont_y } else { yj };
            let items: Vec<Rail> = bs
                .iter()
                .map(|b| self.rail(b, false))
                .chain(cs.iter().map(|b| self.rail(b, true)))
                .collect();
            self.rail_corridor(items, lane, bottom);
            if !cs.is_empty() {
                let x = if right_side {
                    tx + lw / 2.0
                } else {
                    tx - lw / 2.0
                };
                self.edge(&[(lane, cont_y), (x, cont_y)], false);
            }
            if !bs.is_empty() {
                self.edge(&[(lane, yj), (tx, yj)], false);
            }
            // В одном коридоре два смысла — подписи у выходов. Подписи
            // снаружи коридора: между рельсой и фигурой места нет.
            // Выход, нарисованный маркером, подпись уже несёт внутри.
            if !bs.is_empty() && !cs.is_empty() {
                if cs.iter().any(|b| !b.marked) {
                    self.rail_label(lane, right_side, cont_y, "continue");
                }
                if bs.iter().any(|b| !b.marked) {
                    self.rail_label(lane, right_side, yj, "break");
                }
            }
        }
    }

    /// Голова/середина разрезанного цикла: низ трапеции на другом листе.
    /// Обе рельсы уходят на выход листа, но на разных высотах и с
    /// подписями: смысл виден, даже когда геометрия листа одна.
    fn rails_to_page(
        &mut self,
        breaks: &[BreakAt],
        conts: &[BreakAt],
        tx: f64,
        y_exit: f64,
        left: f64,
        right: f64,
    ) {
        for right_side in [false, true] {
            let bs: Vec<&BreakAt> = breaks
                .iter()
                .filter(|b| (b.tx > tx) == right_side)
                .collect();
            let cs: Vec<&BreakAt> = conts.iter().filter(|b| (b.tx > tx) == right_side).collect();
            if bs.is_empty() && cs.is_empty() {
                continue;
            }
            let lane = self.lane(right_side, left, right);
            // высота ровно на сетке совпала бы с by рельсы (низ последней
            // плитки + полшага): горизонталь рельсы и общий хвост легли бы
            // друг на друга. Полшага в сторону от узлов сетки — и общий
            // отрезок рельсы не пересекает: хвост ниже всех капель.
            let both = !bs.is_empty() && !cs.is_empty();
            let cont_y = if both {
                y_exit - self.st.grid * 0.5
            } else {
                y_exit + self.st.grid * 0.5
            };
            let brk_y = y_exit + self.st.grid * 0.5;
            let bottom = if bs.is_empty() { cont_y } else { brk_y };
            let items: Vec<Rail> = bs
                .iter()
                .map(|b| self.rail(b, false))
                .chain(cs.iter().map(|b| self.rail(b, true)))
                .collect();
            self.rail_corridor(items, lane, bottom);
            // На листе без нижней трапеции геометрия выхода одна (кружок
            // следующего листа), поэтому смысл несёт подпись — и когда
            // рельса одна, а не только при обоих смыслах в коридоре.
            if !cs.is_empty() {
                self.edge(&[(lane, cont_y), (tx, cont_y)], false);
                if cs.iter().any(|b| !b.marked) {
                    self.rail_label(lane, right_side, cont_y, "continue");
                }
            }
            if !bs.is_empty() {
                self.edge(&[(lane, brk_y), (tx, brk_y)], false);
                if bs.iter().any(|b| !b.marked) {
                    self.rail_label(lane, right_side, brk_y, "break");
                }
            }
        }
    }
}
