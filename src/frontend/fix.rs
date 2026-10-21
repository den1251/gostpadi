//! Мягкая нормализация форматирования C перед разбором (feature `autofix`).
//!
//! Трогаем только пробельные символы и директивы препроцессора — семантику
//! не чиним: пропущенный `;`, `||` или незакрытая кавычка остаются ошибкой.
//! Границы: число строк не меняется (кроме CR-только файлов, где CR и есть
//! перевод строки), чтобы координаты ошибок оставались координатами исходника.

/// Нормализация: CRLF/CR → LF, BOM снять, табы → 4 пробела, отступы и
/// хвостовые пробелы, `# include < stdio . h >` → `#include <stdio.h>`,
/// `if(` → `if (`. Строковые и символьные литералы не трогаем.
pub fn normalize(src: &str) -> String {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    // CRLF и одиночный CR (старый Mac): для C это всё перевод строки.
    let lf = src.replace("\r\n", "\n").replace('\r', "\n");
    let mut quote: Option<char> = None;
    let mut block = false; // внутри /* ... */
    let mut out = String::with_capacity(lf.len());
    for (i, line) in lf.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        match line.trim_start().strip_prefix('#') {
            Some(rest) if !block && quote.is_none() => out.push_str(&directive(rest)),
            _ => out.push_str(&normalize_line(line, &mut quote, &mut block)),
        }
    }
    out
}

/// Строка кода: табы → 4 пробела, `if(` → `if (`, `}else` → `} else`,
/// хвостовые пробелы. Внутри литералов и комментариев — байт в байт.
fn normalize_line(line: &str, quote: &mut Option<char>, block: &mut bool) -> String {
    const KEYWORDS: [&str; 4] = ["if", "while", "for", "switch"];
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if *block {
            if c == '*' && chars.get(i + 1) == Some(&'/') {
                *block = false;
                out.push_str("*/");
                i += 2;
                continue;
            }
            out.push(c);
            i += 1;
            continue;
        }
        if let Some(q) = *quote {
            out.push(c);
            if c == '\\' {
                if let Some(&n) = chars.get(i + 1) {
                    out.push(n);
                    i += 2;
                    continue;
                }
            } else if c == q {
                *quote = None;
            }
            i += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            *block = true;
            out.push_str("/*");
            i += 2;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            // хвост строки — комментарий: не нормализуем, но и не теряем
            out.extend(&chars[i..]);
            break;
        }
        if c == '"' || c == '\'' {
            *quote = Some(c);
            out.push(c);
            i += 1;
            continue;
        }
        if c == '\t' {
            out.push_str("    ");
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let prev = out.chars().last();
            if word == "else" && prev == Some('}') {
                out.push(' ');
            }
            out.push_str(&word);
            if (KEYWORDS.contains(&word.as_str()) && chars.get(i) == Some(&'('))
                || (word == "else" && chars.get(i) == Some(&'{'))
            {
                out.push(' ');
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    if quote.is_none() {
        let n = out.trim_end().len();
        out.truncate(n);
    }
    out
}

/// Директива без `#`: `#include < stdio . h >` → `#include <stdio.h>`,
/// прочие — `#word rest` с одним пробелом и без хвостовых пробелов
/// (пробел после `\` ломает континуацию — убираем).
fn directive(rest: &str) -> String {
    let rest = rest.trim();
    let word: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let body = rest[word.len()..].trim();
    if word.is_empty() {
        return if body.is_empty() {
            "#".to_string()
        } else {
            format!("# {body}")
        };
    }
    if word == "include" {
        let spec: String = body.chars().filter(|c| !c.is_whitespace()).collect();
        return format!("#include {spec}");
    }
    if body.is_empty() {
        format!("#{word}")
    } else {
        format!("#{word} {body}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_cr_bom_and_tabs() {
        let src = "\u{feff}int main(void)\r\n{\r\n\tint a = 1;\r\n\treturn 0;\r\n}\r";
        let out = normalize(src);
        assert_eq!(out, "int main(void)\n{\n    int a = 1;\n    return 0;\n}\n");
    }

    #[test]
    fn directive_spaces_glued() {
        assert_eq!(
            normalize("#  include   <  stdio . h  >  \n"),
            "#include <stdio.h>\n"
        );
        assert_eq!(normalize("# define MAX 10\n"), "#define MAX 10\n");
        assert_eq!(
            normalize("#define F(x) \\\n  ((x) + 1)\n"),
            "#define F(x) \\\n  ((x) + 1)\n"
        );
    }

    #[test]
    fn keyword_paren_and_else_glued() {
        assert_eq!(
            normalize("if(x){a=1;}else{b=2;}\n"),
            "if (x){a=1;} else {b=2;}\n"
        );
        assert_eq!(normalize("}else{ a = 0; }\n"), "} else { a = 0; }\n");
    }

    #[test]
    fn strings_and_comments_untouched() {
        let src = "printf(\"a\\tb\\t\");\t// if( x\tу\n/* if(\t */\n";
        let out = normalize(src);
        assert!(out.contains("\"a\\tb\\t\""), "литерал цел: {out:?}");
        assert!(out.contains("// if( x\tу"), "комментарий цел: {out:?}");
        assert!(
            out.contains("/* if(\t */"),
            "блочный комментарий цел: {out:?}"
        );
    }

    #[test]
    fn block_comment_across_lines() {
        let out = normalize("a = 1; /* if(\n\tb = 2; */\nif(b){}\n");
        assert_eq!(out, "a = 1; /* if(\n\tb = 2; */\nif (b){}\n");
    }

    #[test]
    fn trailing_spaces_gone() {
        assert_eq!(normalize("a = 1;   \n\n"), "a = 1;\n\n");
    }
}
