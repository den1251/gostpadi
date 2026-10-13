use super::ctx::{BreakAt, ColEnd, Ctx};
use super::measure::{no_sentinel, HEAD, MID, TAIL};
use super::types::Anchor;
use crate::ir::Node;

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

impl Ctx<'_> {
    /// Цикл на основной линии: entry-стрелка сверху, две трапеции
    /// (loop_begin сверху, loop_end снизу), тело между ними.
    /// Порт гост-паттерна 3.4: обратная связь не рисуется — совпадение
    /// номеров на трапециях связывает begin/end неявно.
    pub(super) fn loop_top(
        &mut self,
        nd: &Node,
        prev: Option<(f64, f64)>,
        cursor: f64,
    ) -> (Option<(f64, f64)>, f64) {
        if let Some(p) = prev {
            self.edge(&[p, (0.0, cursor + self.st.vgap)], true);
        }
        let (merge, _) = self.sub_loop(nd, 0.0, cursor + self.st.vgap);
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
        let cy1 = top + lh / 2.0;
        if begins {
            self.add("loop_begin", tx, cy1, no_sentinel(&nd.loop_label()));
        }
        self.loop_depth += 1;
        let num = self.loop_depth;
        // коридор: шире половины трапеции и любого вложенного содержимого,
        // чтобы рельсы break/continue не задевали фигуры
        let top0 = if begins { cy1 + lh / 2.0 + self.st.vgap } else { top };
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
        let cy2 = yend + self.st.vgap + lh / 2.0;
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
        let breaks: Vec<BreakAt> = self.breaks.drain(mark..).collect();
        let conts: Vec<BreakAt> = self.continues.drain(mark_c..).collect();
        // Рельсы break: вниз мимо loop_end, T-стык на продолжении.
        //
        // Колонка плитки (b.tx) уже своя ось: если она не накрыта трапецией
        // слияния, рельса падает из центра плитки прямо на merge и вправо
        // на ось — один изгиб. Коридор тогда не нужен, и лишней ступеньки
        // нет. Трапеция шириной lw по центру tx, так что достаточно
        // выйти за её половину.
        //
        // Плитка на месте — отъезд вниз на в-gap нужен ещё и по виду:
        // горизонталь с высоты низа плитки сливается с её границей и
        // читается как уход из угла, а не из центра. Колонка без плиток
        // (только break/continue) выходит сразу влево: выходить неоткуда,
        // и вертикаль на её оси запрещена.
        if ends {
            for b in &breaks {
                let slot = self.break_slot;
                self.break_slot += 1;
                let clear = b.tx < tx - lw / 2.0 || b.tx > tx + lw / 2.0;
                if b.from_tile && clear {
                    self.edge(&[(b.tx, b.y), (b.tx, merge), (tx, merge)], false);
                    continue;
                }
                let rx = body_left - self.st.grid * (slot as f64 + 1.0);
                self.rail_to(b, rx, merge, tx);
            }
            // рельсы continue: тот же коридор, T-стык на выходе loop_end —
            // следующая итерация
            let yj = (cy2 + lh / 2.0 + merge) * 0.5;
            for c in &conts {
                let slot = self.break_slot;
                self.break_slot += 1;
                let rx = body_left - self.st.grid * (slot as f64 + 1.0);
                self.edge(&[(c.tx, c.y), (rx, c.y), (rx, yj), (tx, yj)], false);
            }
        } else {
            // голова/середина: выход листа — кружок под телом (его рисует
            // следующий узел), обе рельсы сводятся на верх этого кружка:
            // вертикаль входа и сам кружок там уже есть
            let y_exit = yend + self.st.vgap;
            for b in breaks.iter().chain(conts.iter()) {
                let slot = self.break_slot;
                self.break_slot += 1;
                let rx = body_left - self.st.grid * (slot as f64 + 1.0);
                self.rail_to(b, rx, y_exit, tx);
            }
        }
        self.loop_depth -= 1;
        (merge, ColEnd::Flow)
    }

    /// Рельса break/continue: из плитки вниз на в-гэп (иначе горизонталь
    /// сливается с её границей), влево по коридору и горизонталь на y к оси.
    fn rail_to(&mut self, b: &BreakAt, rx: f64, y: f64, tx: f64) {
        let mut pts = vec![(b.tx, b.y)];
        if b.from_tile && b.y + self.st.vgap < y {
            pts.push((b.tx, b.y + self.st.vgap));
        }
        let by = pts[pts.len() - 1].1;
        pts.push((rx, by));
        pts.push((rx, y));
        pts.push((tx, y));
        self.edge(&pts, false);
    }
}
