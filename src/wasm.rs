//! WASM-мост для сайта: C-ABI поверх пайплайна, ноль зависимостей.
//! Протокол: указатель+длина входа, флаги, на выходе буфер с JSON
//! {"ok":true,"sheets":[...]} либо {"ok":false,"error":{...}}.
//! Память: буфер выдаёт Rust, освобождение — gostpadi_free.

use crate::pipeline::{self, Options};

/// Чтение u32 little-endian из буфера с проверкой границ.
fn rd_u32(b: &[u8], off: &mut usize) -> Option<u32> {
    let v = b.get(*off..*off + 4)?;
    *off += 4;
    Some(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
}

/// Чтение байтового блока длиной из буфера.
fn rd_block<'a>(b: &'a [u8], off: &mut usize, len: usize) -> Option<&'a [u8]> {
    let v = b.get(*off..*off + len)?;
    *off += len;
    Some(v)
}

/// Разбор пачки: u32 count, затем для каждого входа u32 name_len,
/// имя, u8 is_c, u32 data_len, текст. None — битый пакет.
fn unpack(b: &[u8]) -> Option<Vec<(String, String, bool)>> {
    let mut off = 0usize;
    let count = rd_u32(b, &mut off)? as usize;
    if count > 4096 {
        return None;
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let nl = rd_u32(b, &mut off)? as usize;
        let name = String::from_utf8(rd_block(b, &mut off, nl)?.to_vec()).ok()?;
        let is_c = *rd_block(b, &mut off, 1)?.first()? != 0;
        let dl = rd_u32(b, &mut off)? as usize;
        let text = String::from_utf8(rd_block(b, &mut off, dl)?.to_vec()).ok()?;
        out.push((name, text, is_c));
    }
    Some(out)
}

/// Экранирование без кавычек: для сборки мелких объектов через format!.
/// Кавычки навешивает вызывающий — как у `json_str`.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = std::fmt::write(&mut out, format_args!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// JSON-строка в буфер: кавычки, слэш и управляющие символы.
fn json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = std::fmt::write(out, format_args!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Текст схемы -> ответ в формате JSON (листы SVG либо ошибка разбора).
fn render_result(text: &str, is_c: bool, ru: bool, lw: f64, font: f64) -> String {
    let opts = Options {
        labels: if ru { "ru" } else { "en" }.into(),
        font: (font > 0.0).then_some(font),
        lw: (lw > 0.0).then_some(lw),
        no_split: false,
    };
    match pipeline::render_text(text, is_c, &opts) {
        Ok(pages) => {
            let mut out = String::with_capacity(pages.iter().map(|p| p.len()).sum::<usize>() + 32);
            out.push_str("{\"ok\":true,\"sheets\":[");
            for (i, p) in pages.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                json_str(&mut out, p);
            }
            out.push_str("]}");
            out
        }
        Err(e) => {
            let mut out = String::with_capacity(128);
            out.push_str("{\"ok\":false,\"error\":{\"msg\":");
            json_str(&mut out, &e.msg);
            out.push_str(",\"line\":");
            match e.line {
                Some(l) => out.push_str(&l.to_string()),
                None => out.push_str("null"),
            }
            out.push_str(",\"col\":");
            match e.col {
                Some(c) => out.push_str(&c.to_string()),
                None => out.push_str("null"),
            }
            out.push_str(",\"src\":");
            match &e.src {
                Some(s) => json_str(&mut out, s),
                None => out.push_str("null"),
            }
            out.push_str("}}");
            out
        }
    }
}

/// Вход: (ptr, len) в UTF-8; is_c/ru — флаги; lw/font — 0 = дефолт.
/// Возврат: указатель на буфер с JSON, длина записана в *out_len.
/// Паника движка (кривой ввод недопустим по контракту парсеров) —
/// wasm-trap: страница покажет «движок упал» и перезапустит модуль.
#[no_mangle]
pub extern "C" fn gostpadi_render(
    ptr: *const u8,
    len: usize,
    is_c: u8,
    ru: u8,
    lw: f64,
    font: f64,
    out_len: *mut usize,
) -> *mut u8 {
    if ptr.is_null() || out_len.is_null() {
        return std::ptr::null_mut();
    }
    let bytes = if len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let text = String::from_utf8_lossy(bytes);
    let json = render_result(&text, is_c != 0, ru != 0, lw, font);
    let boxed = json.into_bytes().into_boxed_slice();
    let p = boxed.as_ptr() as *mut u8;
    let n = boxed.len();
    std::mem::forget(boxed);
    unsafe { *out_len = n };
    p
}

/// Буфер для данных со стороны JS (вход, out_len): пишет страница.
#[no_mangle]
pub extern "C" fn gostpadi_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    let mut v = Vec::<u8>::with_capacity(len);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Пачка файлов: единые размеры блоков и общий масштаб на все входы —
/// как `gostpadi 1/*.gvn -o out/` в CLI. Вход — упакованный unpack();
/// выход — JSON {"ok":true,"files":[{name,sheets}]}, сбойные входы
/// перечислены в "errors", остальные рисуются.
#[no_mangle]
pub extern "C" fn gostpadi_render_batch(
    ptr: *const u8,
    len: usize,
    ru: u8,
    lw: f64,
    font: f64,
    out_len: *mut usize,
) -> *mut u8 {
    if ptr.is_null() || out_len.is_null() {
        return std::ptr::null_mut();
    }
    let bytes = if len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let inputs = match unpack(bytes) {
        Some(v) if !v.is_empty() => v,
        _ => return std::ptr::null_mut(),
    };
    let opts = Options {
        labels: if ru != 0 { "ru" } else { "en" }.into(),
        font: (font > 0.0).then_some(font),
        lw: (lw > 0.0).then_some(lw),
        no_split: false,
    };
    let st = opts.style();
    // сбойные входы не останавливают остальные (порт CLI-пачки)
    let mut ok: Vec<(String, Vec<crate::ir::Node>)> = Vec::new();
    let mut errors: Vec<(String, crate::error::ParseError)> = Vec::new();
    for (name, text, is_c) in &inputs {
        match pipeline::parse_batch(
            std::slice::from_ref(&(name.clone(), text.clone(), *is_c)),
            &opts,
        ) {
            Ok(mut s) => ok.append(&mut s),
            Err((_, e)) => errors.push((name.clone(), e)),
        }
    }
    // вход без рисуемых операторов (пустой main, файл из одних
    // return/объявлений) — отказ фронтенда, а не ошибка разбора:
    // сайт показывает его в «empty» с человеческим текстом. Раньше
    // весь отказ шёл в «errors» и приезжал как «в коде нет ни одной
    // функции с телом» + err-подсветка — после T7 (11.10.2026) пустых
    // схем не бывает, и текст врёт. Разделяем по подстроке: пустой
    // вход -> empty, всё остальное -> errors.
    let rendered = pipeline::render_batch(ok, &st);
    let mut out = String::with_capacity(4096);
    let mut files: Vec<String> = Vec::new();
    for (name, pages) in rendered.iter() {
        let mut f = String::from("{\"name\":");
        f.push('"');
        f.push_str(&json_escape(name));
        f.push_str("\",\"sheets\":[");
        for (k, p) in pages.iter().enumerate() {
            if k > 0 {
                f.push(',');
            }
            f.push('"');
            f.push_str(&json_escape(p));
            f.push('"');
        }
        f.push_str("]}");
        files.push(f);
    }
    let mut errs: Vec<String> = Vec::new();
    let mut empty: Vec<String> = Vec::new();
    for (name, e) in &errors {
        let item = format!(
            "{{\"name\":\"{}\",\"msg\":\"{}\",\"line\":{},\"col\":{},\"src\":{}}}",
            json_escape(name),
            json_escape(&e.msg),
            e.line.map_or("null".into(), |l| l.to_string()),
            e.col.map_or("null".into(), |c| c.to_string()),
            match &e.src {
                Some(s) => format!("\"{}\"", json_escape(s)),
                None => "null".to_string(),
            }
        );
        if e.msg.contains("нет рисуемых операторов") {
            empty.push(item);
        } else {
            errs.push(item);
        }
    }
    out.push_str("{\"ok\":true,\"files\":[");
    out.push_str(&files.join(","));
    out.push_str("],\"empty\":[");
    out.push_str(&empty.join(","));
    out.push_str("],\"errors\":[");
    out.push_str(&errs.join(","));
    out.push_str("]}");
    let boxed = out.into_bytes().into_boxed_slice();
    let p = boxed.as_ptr() as *mut u8;
    let n = boxed.len();
    std::mem::forget(boxed);
    unsafe { *out_len = n };
    p
}

/// Освобождение буфера из gostpadi_render/gostpadi_alloc.
#[no_mangle]
pub extern "C" fn gostpadi_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        unsafe { drop(Vec::from_raw_parts(ptr, len, len)) };
    }
}

/// Тот же вход и тот же JSON, но другой выход на файл:
/// mode 1 — SVG по содержимому, в миллиметрах (для отчёта),
/// mode 2 — draw.io с редактируемыми блоками.
///
/// Отдельный вызов, а не поле в `gostpadi_render_batch`: файлы draw.io
/// и SVG по содержимому нужны по кнопке, а не при каждой отрисовке,
/// и в JSON на все три вида весили бы втрое.
#[no_mangle]
pub extern "C" fn gostpadi_export_batch(
    ptr: *const u8,
    len: usize,
    ru: u8,
    lw: f64,
    font: f64,
    mode: u8,
    out_len: *mut usize,
) -> *mut u8 {
    if ptr.is_null() || out_len.is_null() {
        return std::ptr::null_mut();
    }
    let bytes = if len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let Some(inputs) = unpack(bytes).filter(|v| !v.is_empty()) else {
        return std::ptr::null_mut();
    };
    let opts = Options {
        labels: if ru != 0 { "ru" } else { "en" }.into(),
        font: (font > 0.0).then_some(font),
        lw: (lw > 0.0).then_some(lw),
        no_split: false,
    };
    let st = opts.style();
    let schemes = match pipeline::parse_batch(&inputs, &opts) {
        Ok(s) => s,
        Err((name, e)) => {
            let mut out = String::from("{\"ok\":false,\"error\":{\"msg\":");
            json_str(&mut out, &e.msg);
            out.push_str(",\"name\":");
            json_str(&mut out, &name);
            out.push_str("}}");
            return leak(out, out_len);
        }
    };
    let produced = if mode == 2 {
        pipeline::render_drawio_batch(schemes, &st)
    } else {
        pipeline::render_tight_batch(schemes, &st)
    };
    let mut out = String::with_capacity(4096);
    out.push_str("{\"ok\":true,\"files\":[");
    for (i, (name, page)) in produced.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        json_str(&mut out, name);
        out.push_str(",\"sheets\":[");
        json_str(&mut out, page);
        out.push_str("]}");
    }
    out.push_str("]}");
    leak(out, out_len)
}

/// JSON-строка -> буфер наружу: содержимое забываем, длину пишем в out_len.
/// ABI-протокол упаковки входа (u32 count, [имя][флаг C][текст] на вход)
/// общий с `gostpadi_render_batch`; сама выдача покрыта тестами
/// `render_tight_batch`/`render_drawio_batch` в pipeline — этот модуль
/// компилируется только под wasm32, где `cargo test` не работает.
fn leak(out: String, out_len: *mut usize) -> *mut u8 {
    let boxed = out.into_bytes().into_boxed_slice();
    let p = boxed.as_ptr() as *mut u8;
    let n = boxed.len();
    std::mem::forget(boxed);
    unsafe { *out_len = n };
    p
}
