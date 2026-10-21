use super::ctx::{BreakAt, ColEnd, Ctx};
use super::measure::{no_sentinel, HEAD, MID, TAIL};
use super::types::{Anchor, Shape};
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
        let cy1 = top + lh / 2.0;
        if begins {
            self.add("loop_begin", tx, cy1, no_sentinel(&nd.loop_label()));
        }
        self.loop_depth += 1;
        let num = self.loop_depth;
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
                    && (s.cx + s.w / 2.0 > x + 1.0)
                    && (s.cx - s.w / 2.0 < x - 1.0)
            })
        };
        let breaks: Vec<BreakAt> = self.breaks.drain(mark..).collect();
        let conts: Vec<BreakAt> = self.continues.drain(mark_c..).collect();
        // Рельсы break/continue: вниз мимо loop_end, T-стык на продолжении.
        //
        // Прямой спуск из плитки, стоящей вне трапеции, — самый короткий
        // путь без ступеньки: колонка b.tx уже своя ось. Но только если
        // на вертикали ниже нет чужих фигур и нет другой рельсы той же
        // стороны — её первая горизонталь шла бы через эту колонку.
        // Остальные уходят в коридор своей стороны: правая колонка —
        // вправо, иначе её горизонталь режет ствол цикла. Колонка без
        // плиток (только break/continue) выходит сразу в коридор:
        // выходить из неё неоткуда, вертикаль на её оси пустая.
        if ends {
            // Общая высота слияния рельс: continue уходил на середину
            // выхода loop_end, break — на merge ниже. Разные концы давали
            // частичное перекрытие интервалов, и вертикаль одной рельсы
            // неизбежно резала горизонталь другой (labfix-14: пять рельс
            // в левом поле, 8 X). Одна высота убирает и разницу, и X.
            let yj = (cy2 + lh / 2.0 + merge) * 0.5;
            // Прямой спуск из плитки, стоящей вне трапеции, — самый
            // короткий путь и без ступеньки; такие рельсы занимают свою
            // колонку и в коридоре слотов не участвуют.
            let mut rails: Vec<&BreakAt> = Vec::new();
            let all: Vec<&BreakAt> = breaks.iter().chain(conts.iter()).collect();
            for b in &breaks {
                let free = (b.tx < tx - lw / 2.0 || b.tx > tx + lw / 2.0)
                    && !(b.from_tile && blocked(b.tx, b.y, yj));
                // Прямой спуск идёт по своей колонке внутри тела; рельса
                // той же стороны идёт в коридор снаружи и её первая
                // горизонталь режет эту колонку (oap-lab2: break прямо из
                // плитки и break из пустой ветки на одной стороне).
                let side = b.tx > tx;
                let same_side = all
                    .iter()
                    .any(|r| !std::ptr::eq(*r, b) && (r.tx > tx) == side);
                if b.from_tile && free && !same_side {
                    self.edge(&[(b.tx, b.y), (b.tx, yj), (tx, yj)], false);
                } else {
                    rails.push(b);
                }
            }
            rails.extend(conts.iter());
            self.rail_set(&mut rails, yj, tx, body_left, body_right);
        } else {
            // голова/середина: выход листа — кружок под телом. Рельсы
            // сводятся на полшага ниже тела: на в-гэпе стоит верх
            // следующего блока, и горизонталь ложилась на его кромку
            // (labfix-09: рельса break по верхней грани io).
            let y_exit = yend + self.st.grid;
            let mut rails: Vec<&BreakAt> = breaks.iter().chain(conts.iter()).collect();
            self.rail_set(&mut rails, y_exit, tx, body_left, body_right);
        }
        self.loop_depth -= 1;
        (merge, ColEnd::Flow)
    }

    /// Рельсы уходят в коридор своей стороны (правая колонка — вправо:
    /// иначе её первая горизонталь режет ствол цикла). Верхняя рельса
    /// ставится дальше от оси: её вертикаль не пересекает горизонтали
    /// нижних, а те оканчиваются на её высоте — T-стык.
    fn rail_set(&mut self, rails: &mut [&BreakAt], y: f64, tx: f64, left: f64, right: f64) {
        rails.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal));
        for b in rails.iter() {
            let slot = self.break_slot;
            self.break_slot += 1;
            let rx = if b.tx > tx {
                right + self.st.grid * (slot as f64 + 1.0)
            } else {
                left - self.st.grid * (slot as f64 + 1.0)
            };
            self.rail_to(b, rx, y, tx);
        }
    }

    /// Рельса break/continue: из плитки вниз на полшага сетки (иначе
    /// горизонталь ложится на нижнюю грань плитки), влево по коридору и
    /// горизонталь на y к оси. Полшага, а не в-гэп: в-гэп от низа тела
    /// приходится ровно на верхнюю грань loop_end (labfix-05).
    fn rail_to(&mut self, b: &BreakAt, rx: f64, y: f64, tx: f64) {
        let mut pts = vec![(b.tx, b.y)];
        if b.from_tile && b.y + self.st.grid <= y {
            pts.push((b.tx, b.y + self.st.grid));
        }
        let by = pts[pts.len() - 1].1;
        pts.push((rx, by));
        pts.push((rx, y));
        pts.push((tx, y));
        self.edge(&pts, false);
    }
}
