//! CLI: выходы, коды возврата, сообщения об ошибках.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gostpadi"))
        .args(args)
        .output()
        .unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = env::temp_dir().join(format!("gostpadi-cli-{}-{name}", std::process::id()));
    fs::create_dir_all(&d).unwrap();
    d
}

const OK_GVN: &str = "input scanf(\"%d\", &a)\nc = a * 2\noutput printf(\"c\", c)\n";

#[test]
fn check_ok_exits_zero() {
    let d = tmp("check-ok");
    let f = d.join("ok.gvn");
    fs::write(&f, OK_GVN).unwrap();
    let out = run(&["--check", f.to_str().unwrap()]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("ok: 5 blocks"));
}

#[test]
fn check_bad_exits_one_with_line() {
    let d = tmp("check-bad");
    let f = d.join("bad.gvn");
    // строка 2 с неожиданным отступом
    fs::write(&f, "input scanf(\"%d\", &a)\n    c = 1\n").unwrap();
    let out = run(&["--check", f.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains(&format!("{}:2:", f.display())),
        "stderr: {err}"
    );
    assert!(err.contains("indentation"), "stderr: {err}");
}

#[test]
fn template_prints_gvn_stub() {
    let out = run(&["--template"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("input scanf"));
}

/// Шаблон обязан быть рабочей схемой: --template -> файл -> --check = ok.
/// Ловит съеденные `\`-переносом отступы веток (if без веток).
#[test]
fn template_roundtrips_through_check() {
    let d = tmp("template-roundtrip");
    let f = d.join("tpl.gvn");
    let out = run(&["--template"]);
    fs::write(&f, &out.stdout).unwrap();
    let chk = run(&["--check", f.to_str().unwrap()]);
    assert!(
        chk.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&chk.stderr)
    );
    assert!(String::from_utf8_lossy(&chk.stdout).contains("ok: 6 blocks"));
}

#[test]
fn no_args_exits_two() {
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unknown_flag_exits_two() {
    let out = run(&["--wat"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn version_flag() {
    let out = run(&["-V"]);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "gostpadi 2.0.0"
    );
}

#[test]
fn help_flag_short() {
    let out = run(&["--help"]);
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("ИСПОЛЬЗОВАНИЕ"));
    assert!(help.contains("--labels=ru|en"));
}

#[test]
fn render_to_file_creates_svg() {
    let d = tmp("render-file");
    let src = d.join("in.gvn");
    fs::write(&src, OK_GVN).unwrap();
    let out_path = d.join("out.svg");
    let out = run(&[src.to_str().unwrap(), "-o", out_path.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let svg = fs::read_to_string(&out_path).unwrap();
    assert!(svg.contains("<svg"));
}

#[test]
fn render_examples_board2_if_to_tmp() {
    let d = tmp("board2");
    let out_path = d.join("out.svg");
    let out = run(&["examples/board2_if.gvn", "-o", out_path.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let svg = fs::read_to_string(&out_path).unwrap();
    assert!(svg.contains("<svg"));
}

#[test]
fn batch_into_folder_with_dup_stems() {
    let d = tmp("batch");
    fs::create_dir_all(d.join("1")).unwrap();
    fs::create_dir_all(d.join("2")).unwrap();
    let a = d.join("1/main.c");
    let b = d.join("2/main.c");
    fs::write(&a, "int main(){printf(\"hi\");return 0;}").unwrap();
    fs::write(&b, "int main(){printf(\"bye\");return 0;}").unwrap();
    let dir = d.join("out");
    fs::create_dir_all(&dir).unwrap();
    let out = run(&[
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "-o",
        dir.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.join("1-main.svg").exists());
    assert!(dir.join("2-main.svg").exists());
}

/// -o без косой черты и не существующая папка — basename-префикс:
/// результат рядом с КАЖДЫМ входом (как Python), не директория.
/// Голое имя относительно входов — гоняем с current_dir во временной папке.
#[test]
fn batch_bare_o_writes_next_to_each_input() {
    let d = tmp("bare-o");
    fs::create_dir_all(d.join("a")).unwrap();
    fs::create_dir_all(d.join("b")).unwrap();
    fs::write(d.join("a/x.gvn"), OK_GVN).unwrap();
    fs::write(d.join("b/y.gvn"), OK_GVN).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_gostpadi"))
        .args(["a/x.gvn", "b/y.gvn", "-o", "out.svg"])
        .current_dir(&d)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !d.join("out.svg").exists(),
        "-o не должен был стать файлом в cwd"
    );
    assert!(d.join("a/out.svg").exists());
    assert!(d.join("b/out.svg").exists());
}

/// Пачка: сбойный вход пропускается с сообщением, остальные рендерятся,
/// код возврата 1 (порт render_many).
#[test]
fn batch_skips_bad_input_exit_one() {
    let d = tmp("batch-skip");
    let ok = d.join("ok.gvn");
    let bad = d.join("bad.gvn");
    fs::write(&ok, OK_GVN).unwrap();
    fs::write(&bad, "    c = 1\n").unwrap();
    let out = run(&[ok.to_str().unwrap(), bad.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("bad.gvn"), "stderr: {err}");
    assert!(d.join("ok.svg").exists(), "ok.svg не создан");
}

/// `| head`: обрыв трубы не паника (SIGPIPE игнорируется std, BrokenPipe
/// обрабатывается тихим exit 0).
#[test]
fn broken_pipe_exits_zero() {
    let d = tmp("sigpipe");
    let mut paths: Vec<String> = Vec::new();
    for i in 0..300 {
        let f = d.join(format!("f{i}.gvn"));
        fs::write(&f, OK_GVN).unwrap();
        paths.push(f.to_str().unwrap().to_string());
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_gostpadi"))
        .args(&paths)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // закрываем трубу до того, как ребёнок допишет 300 строк
    drop(child.stdout.take());
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(0), "паника на BrokenPipe даёт 101");
}

fn render_svg(d: &std::path::Path, name: &str, extra: &[&str]) -> String {
    let src = d.join(format!("{name}.gvn"));
    fs::write(&src, OK_GVN).unwrap();
    let out_path = d.join(format!("{name}.svg"));
    let mut args = vec![src.to_str().unwrap(), "-o", out_path.to_str().unwrap()];
    args.extend_from_slice(extra);
    let out = run(&args);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs::read_to_string(&out_path).unwrap()
}

fn rect_widths(svg: &str) -> Vec<f64> {
    let mut out: Vec<f64> = svg
        .split("<rect")
        // skip(2): мусор до первого <rect и белая подложка страницы
        .skip(2)
        .filter_map(|p| {
            p.split("width=\"")
                .nth(1)
                .and_then(|s| s.split('"').next())
                .and_then(|v| v.parse::<f64>().ok())
        })
        .collect();
    out.sort_by(|a, b| b.partial_cmp(a).unwrap());
    out
}

/// --font=16: в SVG font-size="16", фигуры крупнее, чем при дефолте 12.
#[test]
fn font_flag_grows_svg() {
    let d = tmp("font16");
    let svg16 = render_svg(&d, "f16", &["--font=16"]);
    assert!(
        svg16.contains("font-size=\"16\""),
        "нет font-size=\"16\": {:?}",
        svg16.matches("font-size").take(3).collect::<Vec<_>>()
    );
    let svg12 = render_svg(&d, "f12", &[]);
    assert!(svg12.contains("font-size=\"12\""));
    assert!(
        rect_widths(&svg16)[0] > rect_widths(&svg12)[0],
        "rect при font=16 крупнее: {} vs {}",
        rect_widths(&svg16)[0],
        rect_widths(&svg12)[0]
    );
}

/// --lw=2.5: толщина пера 2.5 и на линиях, и на усиках стрелок.
#[test]
fn lw_flag_sets_stroke_width() {
    let d = tmp("lw25");
    let svg = render_svg(&d, "lw", &["--lw=2.5"]);
    assert!(svg.contains("stroke-width=\"2.5\""), "{svg}");
    // усики — path с тем же пером
    assert!(svg.contains("<path"), "нет усиков");
    let paths_lw: Vec<&str> = svg
        .split("<path")
        .skip(1)
        .filter_map(|p| {
            p.split("stroke-width=\"")
                .nth(1)
                .and_then(|s| s.split('"').next())
        })
        .collect();
    assert!(paths_lw.iter().all(|&w| w == "2.5"), "{paths_lw:?}");
}

/// --font=0 и --lw=-1 — usage-ошибка с сообщением, exit 2.
#[test]
fn invalid_font_lw_exits_two() {
    let d = tmp("bad-flags");
    let src = d.join("in.gvn");
    fs::write(&src, OK_GVN).unwrap();
    for flag in ["--font=0", "--font=-3", "--lw=-1", "--font=abc"] {
        let out = run(&[src.to_str().unwrap(), flag]);
        assert_eq!(out.status.code(), Some(2), "{flag}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!err.is_empty(), "{flag}: нет сообщения");
    }
}

/// Функции файла рисуются все: main — схема входа, helper'ы — свои
/// файлы с суффиксом. Функция из одних return теряться не должна —
/// но и пустой схемой «Start → End» не рисуется (T7): в helper'е
/// должна быть хотя бы одна рисуемая операция.
#[test]
fn all_functions_of_file_get_own_scheme() {
    let d = tmp("functions");
    let f = d.join("two.c");
    fs::write(
        &f,
        "int add(int a, int b) { int s = a + b; return s; }\nint main() { printf(\"%d\", add(2, 3)); return 0; }",
    )
    .unwrap();
    let out = run(&[f.to_str().unwrap(), "-o", d.join("o/").to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(d.join("o/two.svg").exists(), "main — схема входа");
    assert!(d.join("o/two-add.svg").exists(), "helper — свой файл");
}

/// Схема функции подписана её сигнатурой: коробка «предопределённый
/// процесс» (прямоугольник с двойными вертикальными линиями) над
/// «началом», стрелка из коробки в овал. main — программа, не функция:
/// подписи нет (репорт: на доске блок Min(x,y) → начало → … → конец).
#[test]
fn function_signature_is_drawn_above_schema() {
    let d = tmp("signature");
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/corpus/oap-lab3.c");
    let out = run(&[src, "-o", d.join("o/").to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let read = |name: &str| fs::read_to_string(d.join("o").join(name)).unwrap();

    let print = read("oap-lab3-print_mas.svg");
    assert!(
        print.contains("print_mas(const int a[], int n)"),
        "нет сигнатуры над схемой print_mas"
    );
    assert_eq!(
        print.matches("<line ").count(),
        2,
        "коробка подписи без двойных вертикальных линий"
    );

    let sum = read("oap-lab3-sum_neg.svg");
    assert!(
        sum.contains("sum_neg(int a[], int n)"),
        "нет сигнатуры над схемой sum_neg"
    );

    let main = read("oap-lab3.svg");
    assert!(
        !main.contains("main(void)") && !main.contains("<line "),
        "main подписан как функция"
    );

    let chk = run(&["--check", src]);
    assert!(
        chk.status.success(),
        "--check корпуса: {}",
        String::from_utf8_lossy(&chk.stderr)
    );
}

/// Файл без main, но с функцией — не отказ: единственная функция
/// файла и есть схема.
#[test]
fn file_without_main_still_draws() {
    let d = tmp("no-main");
    let f = d.join("helper.c");
    fs::write(
        &f,
        "int helper(int x) { int y = x * 2; printf(\"%d\", y); return y; }",
    )
    .unwrap();
    let out = run(&[f.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        d.join("helper-helper.svg").exists(),
        "схема единственной функции"
    );
}

/// Пустой main (одни return) — не схема «Start → End», а честный отказ
/// с кодом 2: рисовать нечего (репорт 11.10.2026).
#[test]
fn empty_main_is_refused_not_start_end() {
    let d = tmp("empty-main");
    let f = d.join("e.c");
    fs::write(&f, "int main(void) { return 0; }").unwrap();
    let out = run(&[f.to_str().unwrap()]);
    assert!(!out.status.success(), "пустой main — отказ");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("нет рисуемых операторов"), "stderr: {}", err);
    assert!(!d.join("e.svg").exists(), "схема не пишется");
}

/// Один helper без рисуемых операторов (только return) — отказ по
/// всему файлу: пустышка пропускается, но если она единственная —
/// рисовать нечего.
#[test]
fn file_of_only_empty_helper_is_refused() {
    let d = tmp("helper-empty");
    let f = d.join("h.c");
    fs::write(&f, "int twice(int x) { return x * 2; }").unwrap();
    let out = run(&[f.to_str().unwrap()]);
    assert!(!out.status.success(), "пустышка — отказ");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("нет рисуемых операторов"), "stderr: {}", err);
}

/// --trim: SVG по содержимому — миллиметры, без `<g transform>` (без
/// ужатия) и без листа А4 вокруг.
#[test]
fn trim_gives_mm_svg_without_scaling() {
    let d = tmp("trim");
    let f = d.join("t.gvn");
    fs::write(&f, OK_GVN).unwrap();
    let out = run(&["--trim", f.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let svg = fs::read_to_string(d.join("t.svg")).unwrap();
    assert!(svg.contains("mm\""), "размер в миллиметрах");
    assert!(!svg.contains("scale("), "без вписывания: {svg}");
}

/// --drawio: редактируемый mxGraph XML, а не картинка.
#[test]
fn drawio_gives_editable_mxfile() {
    let d = tmp("drawio");
    let f = d.join("t.gvn");
    fs::write(&f, OK_GVN).unwrap();
    let out = run(&["--drawio", f.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let xml = fs::read_to_string(d.join("t.drawio")).unwrap();
    assert!(xml.contains("<mxfile host=\"gostpadi\">"), "не mxfile");
    assert!(xml.contains("vertex=\"1\""), "блоки не редактируемые");
}

/// do-while: полноценный цикл, а не отказ. Подпись «do while» видна в
/// SVG (проверка идёт после тела — подпись обязательна).
#[test]
fn dowhile_renders_with_label() {
    let d = tmp("dowhile");
    let f = d.join("d.c");
    fs::write(
        &f,
        "int main(void) { int i = 0; do { printf(\"%d\", i); i = i + 1; } while (i < 5); return 0; }",
    )
    .unwrap();
    let out = run(&["--check", f.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    run(&[f.to_str().unwrap()]);
    let svg = fs::read_to_string(d.join("d.svg")).unwrap();
    assert!(svg.contains("do while"), "подпись do while в схеме: {svg}");
}

/// Пытка: 5 функций (static, void, argv, массивы), макросы (объектный,
/// функциональный, многострочный с континуацией), else-if каскад с
/// return в каждой ветке, do-while, break/continue в while+switch.
/// Каждая функция — своя схема, все проходят --check.
#[test]
fn torture_file_draws_all_functions() {
    let d = tmp("torture");
    let f = d.join("t.c");
    fs::write(
        &f,
        "\
#include <stdio.h>
#define PI 3.1415926535
#define SQR(x) ((x) * (x))
#define MULTI \\\n    printf(\"one\"); \\\n    printf(\"two\")

static int twice(int x) { int r = x * 2; return r; }

void greet(const char *n) { printf(\"Hi %s\\n\", n); }

double area(double r) { double a = PI * SQR(r); return a; }

int classify(int x) {
    if (x < 0) return -1;
    else if (x == 0) return 0;
    else if (x < 100) return 1;
    else return 2;
}

int main(int argc, char **argv) {
    int choice = 0;
    while (1) {
        printf(\"menu\\n\");
        if (scanf(\"%d\", &choice) != 1) continue;
        if (choice == 1) { greet(argv[0]); }
        else if (choice == 2) { printf(\"%d\\n\", twice(21)); }
        else if (choice == 3) { printf(\"%lf\\n\", area(2.0)); }
        else { printf(\"bye\\n\"); }
        if (choice == 4) break;
    }
    int v[3] = {1, 2, 3};
    int k = 0;
    do {
        if (k == 1) { k++; continue; }
        k++;
    } while (k < 3);
    MULTI;
    printf(\"%d %d\\n\", classify(5), v[0]);
    return 0;
}",
    )
    .unwrap();
    let out = run(&[f.to_str().unwrap(), "-o", d.join("o/").to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    for name in ["t", "t-twice", "t-greet", "t-area", "t-classify"] {
        assert!(d.join(format!("o/{name}.svg")).exists(), "нет {name}.svg");
    }
    let out = run(&["--check", f.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "--check пытки: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Выключенная ветка #if 0 на схеме не появляется, #define с
/// продолжением строки не роняет парсер.
#[test]
fn if0_and_continuations_render_clean() {
    let d = tmp("if0");
    let f = d.join("p.c");
    fs::write(
        &f,
        "int main(void) {\n#if 0\n    printf(\"dead\");\n#endif\n#define MSG \\\n    \"hello\"\n    printf(MSG);\n    return 0;\n}",
    )
    .unwrap();
    let out = run(&["--check", f.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    run(&[f.to_str().unwrap()]);
    let svg = fs::read_to_string(d.join("p.svg")).unwrap();
    assert!(!svg.contains("dead"), "выключенный код на схеме: {svg}");
    assert!(svg.contains("printf(MSG)"), "живой код на схеме: {svg}");
}

/// Мягкий фикс по умолчанию спасает CR-только файл (старый Mac), а
/// `--no-fix` возвращает строгий разбор и падает.
#[cfg(feature = "autofix")]
#[test]
fn no_fix_flag_switches_to_strict() {
    let d = tmp("no-fix");
    let f = d.join("cr.c");
    fs::write(
        &f,
        "int main(void)\r{\r// сумма\rint a = 1;\rprintf(\"%d\", a);\rreturn 0;\r}\r",
    )
    .unwrap();
    let ok = run(&["--check", f.to_str().unwrap()]);
    assert!(
        ok.status.success(),
        "фикс не спас вход: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let strict = run(&["--check", "--no-fix", f.to_str().unwrap()]);
    assert_eq!(strict.status.code(), Some(1), "строгий режим должен падать");
    assert!(run(&["--help"]).status.success());
    let help = String::from_utf8_lossy(&run(&["--help"]).stdout).to_string();
    assert!(help.contains("--no-fix"), "флаг не описан в --help");
}


#[test]
fn output_file_keeps_function_schemes() {
    // -o файл не должен склеивать схемы функций в один файл
    let dir = std::env::temp_dir().join(format!("gostpadi-outfile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("lab.svg");
    let st = std::process::Command::new(env!("CARGO_BIN_EXE_gostpadi"))
        .args(["corpus/oap-lab4.c", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .collect();
    assert!(names.iter().any(|n| n.starts_with("lab-show_mas")), "{names:?}");
    assert!(names.iter().any(|n| n.starts_with("lab-max_diag")), "{names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn small_scale_warns() {
    let st = std::process::Command::new(env!("CARGO_BIN_EXE_gostpadi"))
        .args(["corpus/oap-lab2.c", "--no-split", "-o", "/tmp/gostpadi-warn.svg"])
        .output()
        .unwrap();
    assert!(st.status.success());
    let err = String::from_utf8_lossy(&st.stderr);
    assert!(err.contains("предупреждение: масштаб"), "{err}");
}
