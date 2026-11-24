//! FOCAL, the HP-41's program bytes: decode them to a listing, and encode a
//! listing back to bytes the calculator loads from a .raw file.

use std::collections::HashMap;

/// XROM names by (module, function), from the X-Functions table and the
/// module files on the calculator's disk.
pub type Names = HashMap<(u8, u8), String>;

/// One program line: its text, whether it ends a program, and where its
/// bytes start in the code it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub text: String,
    pub end: bool,
    pub at: usize,
}

/// The HP-41 character set as shown here. Codes 32 to 125 are ASCII; 126 is
/// sigma and 127 the append sign. The rest follow the table in dm41xm, so
/// every code has a character of its own and a listing encodes back exactly.
const LOW: &str = "█αβγδεζηθικλμ∠νξοπρστυφχψωϊϋ←≠→↔";
const HIGH: &str = "€∞‚ƒ„…†‡ˆ‰Š‹Œ∑Ž≤≥‘’“”•–—˜™š›œ∫žŸ∩¡¢£¤¥¦§¨©ª«¬∪®¯°±²³´µ¶·¸¹º»\
                    ¼½¾¿ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÝÞßàáâãäåæçèéêëìíîïðñòóôõö÷øùúûüýþÿ";

pub fn hp_char(b: u8) -> char {
    match b {
        0..=31 => LOW.chars().nth(b as usize).unwrap_or('?'),
        32..=125 => b as char,
        126 => 'Σ',
        127 => '⊢',
        _ => HIGH.chars().nth(b as usize - 128).unwrap_or('?'),
    }
}

fn from_hp_char(c: char) -> Option<u8> {
    match c {
        ' '..='}' => Some(c as u8),
        'Σ' => Some(126),
        '⊢' => Some(127),
        _ => LOW
            .chars()
            .position(|x| x == c)
            .map(|i| i as u8)
            .or_else(|| HIGH.chars().position(|x| x == c).map(|i| i as u8 + 128)),
    }
}

pub fn hp_text(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| hp_char(b)).collect()
}

/// Bytes 0x40 to 0x8F: the functions that take one byte.
const ONE: [&str; 80] = [
    "+", "-", "*", "/", "X<Y?", "X>Y?", "X<=Y?", "Σ+", "Σ-", "HMS+", "HMS-", "MOD", "%", "%CH", "P-R", "R-P", //
    "LN", "X^2", "SQRT", "Y^X", "CHS", "E^X", "LOG", "10^X", "E^X-1", "SIN", "COS", "TAN", "ASIN", "ACOS", "ATAN",
    "DEC", //
    "1/X", "ABS", "FACT", "X≠0?", "X>0?", "LN1+X", "X<0?", "X=0?", "INT", "FRC", "D-R", "R-D", "HMS", "HR", "RND",
    "OCT", //
    "CLΣ", "X<>Y", "PI", "CLST", "R^", "RDN", "LASTX", "CLX", "X=Y?", "X≠Y?", "SIGN", "X<=0?", "MEAN", "SDEV",
    "AVIEW", "CLD", //
    "DEG", "RAD", "GRAD", "ENTER^", "STOP", "RTN", "BEEP", "CLA", "ASHF", "PSE", "CLRG", "AOFF", "AON", "OFF",
    "PROMPT", "ADV",
];

/// Bytes 0x90 to 0x9F: a function and a register, flag or digit after it.
const ROW9: [&str; 16] = [
    "RCL", "STO", "ST+", "ST-", "ST*", "ST/", "ISG", "DSE", "VIEW", "ΣREG", "ASTO", "ARCL", "FIX", "SCI", "ENG", "TONE",
];

/// Bytes 0xA8 to 0xAD: the flag tests.
const FLAGS: [&str; 6] = ["SF", "CF", "FS?C", "FC?C", "FS?", "FC?"];

/// Other ways to write a name, as rpnx and older listings do.
const ALIASES: [(&str, &str); 10] = [
    ("ENTER", "ENTER^"),
    ("X#0?", "X≠0?"),
    ("X!=0?", "X≠0?"),
    ("X#Y?", "X≠Y?"),
    ("X!=Y?", "X≠Y?"),
    ("X≤Y?", "X<=Y?"),
    ("X≤0?", "X<=0?"),
    ("X↑2", "X^2"),
    ("Y↑X", "Y^X"),
    ("R↑", "R^"),
];

/// Register and label names after the number range: A-J, the stack and
/// status registers, and the local labels a-e.
const POST: [&str; 26] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "T", "Z", "Y", "X", "L", "M", "N", "O", "P", "Q", "⊢", "a", "b",
    "c", "d", "e",
];

/// The X-Functions of the 41CX (module 25). The numbers are checked against
/// the XROM lines in the HP-41 ROM sources and the DM41X's own module file,
/// which adds X<I>Y at the end.
const XF: [&str; 64] = [
    "-EXT FCN", "ALENG", "ANUM", "APPCHR", "APPREC", "ARCLREC", "AROT", "ATOX", "CLFL", "CLKEYS", "CRFLAS", "CRFLD",
    "DELCHR", "DELREC", "EMDIR", "FLSIZE", "GETAS", "GETKEY", "GETP", "GETR", "GETREC", "GETRX", "GETSUB", "GETX",
    "INSCHR", "INSREC", "PASN", "PCLPS", "POSA", "POSFL", "PSIZE", "PURFL", "RCLFLAG", "RCLPT", "RCLPTA", "REGMOVE",
    "REGSWAP", "SAVEAS", "SAVEP", "SAVER", "SAVERX", "SAVEX", "SEEKPT", "SEEKPTA", "SIZE?", "STOFLAG", "X<>F",
    "XTOA", "-CX EXT FCN", "ASROOM", "CLRGX", "ED", "EMDIRX", "EMROOM", "GETKEYX", "RESZFL", "ΣREG?", "X=NN?",
    "X≠NN?", "X<NN?", "X<=NN?", "X>NN?", "X>=NN?", "X<I>Y",
];

/// The Time functions of the 41CX (module 26), in the same order: the
/// HP-41 names them alphabetically, the CX additions after a second header.
const TIME: [&str; 36] = [
    "-TIME", "ADATE", "ALMCAT", "ALMNOW", "ATIME", "ATIME24", "CLK12", "CLK24", "CLKT", "CLKTD", "CLOCK", "CORRECT",
    "DATE", "DATE+", "DDAYS", "DMY", "DOW", "MDY", "RCLAF", "RCLSW", "RUNSW", "SETAF", "SETDATE", "SETIME", "SETSW",
    "STOPSW", "SW", "T+X", "TIME", "XYZALM", "-CX TIME", "CLALMA", "CLALMX", "CLRALMS", "RCLALM", "SWPT",
];

pub fn builtin_names() -> Names {
    let table = |m: u8, t: &[&str]| -> Vec<((u8, u8), String)> {
        t.iter().enumerate().filter(|(_, n)| !n.starts_with('-')).map(|(f, n)| ((m, f as u8), n.to_string())).collect()
    };
    table(25, &XF).into_iter().chain(table(26, &TIME)).collect()
}

/// A register, flag or label after a function. `wide` writes 0 to 9 with two
/// digits, as the calculator does for registers but not for FIX 4.
fn postfix(b: u8, wide: bool) -> String {
    let v = b & 0x7F;
    let s = match v {
        0..=0x65 if wide => format!("{v:02}"),
        0..=0x65 => v.to_string(),
        _ => POST[(v - 0x66) as usize].to_string(),
    };
    if b & 0x80 != 0 {
        format!("IND {s}")
    } else {
        s
    }
}

fn parse_postfix(s: &str) -> Option<u8> {
    let s = s.trim();
    if let Some(r) = s.strip_prefix("IND ") {
        return parse_postfix(r).filter(|v| v & 0x80 == 0).map(|v| v | 0x80);
    }
    let s = s.strip_prefix("ST ").unwrap_or(s).trim();
    if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() {
        return s.parse::<u8>().ok().filter(|&v| v <= 0x65);
    }
    POST.iter().position(|&p| p == s).map(|i| i as u8 + 0x66)
}

/// A number line from its digit bytes (0x10 to 0x1C).
fn number(run: &[u8]) -> String {
    let (mut mant, mut exp, mut neg, mut eneg, mut in_exp) = (String::new(), String::new(), false, false, false);
    for &b in run {
        match b {
            0x10..=0x19 => (if in_exp { &mut exp } else { &mut mant }).push((b'0' + b - 0x10) as char),
            0x1A => mant.push('.'),
            0x1B => in_exp = true,
            _ if in_exp => eneg = !eneg,
            _ => neg = !neg,
        }
    }
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    s.push_str(&mant);
    if in_exp {
        if !mant.is_empty() {
            s.push(' ');
        }
        s.push('E');
        if eneg {
            s.push('-');
        }
        s.push_str(&exp);
    }
    s
}

fn quoted(bytes: &[u8]) -> String {
    format!("\"{}\"", hp_text(bytes))
}

/// The program lines in `code`. Null bytes are skipped; they only part two
/// numbers that follow each other.
pub fn decode(code: &[u8], names: &Names) -> Vec<Step> {
    let mut out = Vec::new();
    let mut i = 0;
    let at = |k: usize| code.get(k).copied().unwrap_or(0);
    while i < code.len() {
        let b = code[i];
        let mut end = false;
        let (len, text) = match b {
            0x00 => {
                i += 1;
                continue;
            }
            0x01..=0x0F => (1, format!("LBL {:02}", b - 1)),
            0x10..=0x1C => {
                let n = code[i..].iter().take_while(|&&x| (0x10..=0x1C).contains(&x)).count();
                (n, number(&code[i..i + n]))
            }
            0x1D..=0x1F => {
                let n = (at(i + 1) & 0x0F) as usize;
                let word = ["GTO", "XEQ", "W"][(b - 0x1D) as usize];
                (2 + n, format!("{word} {}", quoted(&code[(i + 2).min(code.len())..(i + 2 + n).min(code.len())])))
            }
            0x20..=0x2F => (1, format!("RCL {:02}", b - 0x20)),
            0x30..=0x3F => (1, format!("STO {:02}", b - 0x30)),
            0x40..=0x8F => (1, ONE[(b - 0x40) as usize].to_string()),
            0x90..=0x9F => (2, format!("{} {}", ROW9[(b - 0x90) as usize], postfix(at(i + 1), b < 0x9C))),
            0xA0..=0xA7 => {
                let (m, f) = (((b & 7) << 2) | (at(i + 1) >> 6), at(i + 1) & 0x3F);
                (2, names.get(&(m, f)).cloned().unwrap_or_else(|| format!("XROM {m:02},{f:02}")))
            }
            0xA8..=0xAD => (2, format!("{} {}", FLAGS[(b - 0xA8) as usize], postfix(at(i + 1), true))),
            0xAE => {
                let p = at(i + 1);
                let word = if p & 0x80 != 0 { "XEQ" } else { "GTO" };
                (2, format!("{word} IND {}", postfix(p & 0x7F, true)))
            }
            0xAF | 0xB0 => (1, "SPARE".to_string()),
            0xB1..=0xBF => (2, format!("GTO {:02}", (b & 0x0F) - 1)),
            0xC0..=0xCD if at(i + 2) >= 0xF0 => {
                let n = (at(i + 2) & 0x0F) as usize;
                let name = &code[(i + 4).min(code.len())..(i + 3 + n).min(code.len())];
                (3 + n, format!("LBL {}", quoted(name)))
            }
            0xC0..=0xCD => {
                end = true;
                (3, if at(i + 2) & 0x20 != 0 { ".END." } else { "END" }.to_string())
            }
            0xCE => (2, format!("X<> {}", postfix(at(i + 1), true))),
            0xCF => (2, format!("LBL {}", postfix(at(i + 1) & 0x7F, true))),
            0xD0..=0xDF => (3, format!("GTO {}", postfix(at(i + 2) & 0x7F, true))),
            0xE0..=0xEF => (3, format!("XEQ {}", postfix(at(i + 2) & 0x7F, true))),
            0xF0..=0xFF => {
                let n = (b & 0x0F) as usize;
                (1 + n, quoted(&code[(i + 1).min(code.len())..(i + 1 + n).min(code.len())]))
            }
        };
        out.push(Step { text, end, at: i });
        i += len;
    }
    out
}

/// The listing as text, one numbered line per step, the way the calculator
/// prints it. rpnx loads this as it is.
pub fn listing(steps: &[Step]) -> String {
    let w = if steps.len() >= 100 { 3 } else { 2 };
    let mut s = String::new();
    for (n, st) in steps.iter().enumerate() {
        let mark = if st.text.starts_with("LBL \"") { "*" } else { " " };
        s.push_str(&format!("{:0w$}{mark}{}\n", n + 1, st.text));
    }
    s
}

/// The instruction on a listing line, without its step number.
fn body(line: &str) -> &str {
    let t = line.trim();
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    let rest = &t[digits..];
    if digits >= 2 && (rest.starts_with([' ', '*', '▸']) || rest.is_empty()) {
        rest.trim_start_matches(['*', '▸']).trim()
    } else {
        t
    }
}

fn text_bytes(s: &str, max: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for c in s.chars() {
        out.push(from_hp_char(c).ok_or_else(|| format!("the calculator has no character {c:?}"))?);
    }
    if out.len() > max {
        return Err(format!("text is {} characters, at most {max} fit", out.len()));
    }
    Ok(out)
}

/// The text between the quotes, or None when the argument is not quoted.
fn unquote(s: &str) -> Option<&str> {
    let s = s.trim();
    let inner = s.strip_prefix('"')?;
    Some(inner.strip_suffix('"').unwrap_or(inner))
}

fn number_bytes(s: &str) -> Option<Vec<u8>> {
    let t = s.replace(' ', "");
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'E' | '-')) {
        return None;
    }
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.as_str()),
    };
    let (mant, exp) = match rest.split_once('E') {
        Some((m, e)) => (m, Some(e)),
        None => (rest, None),
    };
    if mant.is_empty() && exp.is_none() || mant.matches('.').count() > 1 || mant.contains('-') {
        return None;
    }
    let mut out: Vec<u8> = mant.bytes().map(|c| if c == b'.' { 0x1A } else { c - b'0' + 0x10 }).collect();
    if neg {
        out.push(0x1C);
    }
    if let Some(e) = exp {
        let (eneg, digits) = match e.strip_prefix('-') {
            Some(d) => (true, d),
            None => (false, e),
        };
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) || digits.len() > 2 {
            return None;
        }
        out.push(0x1B);
        out.extend(digits.bytes().map(|c| c - b'0' + 0x10));
        if eneg {
            out.push(0x1C);
        }
    }
    Some(out)
}

/// One instruction to its bytes.
fn encode_one(t: &str, names: &Names) -> Result<Vec<u8>, String> {
    if let Some(text) = unquote(t) {
        let (lead, text) = match text.strip_prefix('⊢').or_else(|| text.strip_prefix('├')) {
            Some(r) => (vec![0x7F], r),
            None => (vec![], text),
        };
        let mut bytes = lead;
        bytes.extend(text_bytes(text, 15 - bytes.len())?);
        let mut out = vec![0xF0 | bytes.len() as u8];
        out.extend(bytes);
        return Ok(out);
    }
    if let Some(n) = number_bytes(t) {
        return Ok(n);
    }
    let (word, arg) = match t.split_once(char::is_whitespace) {
        Some((w, a)) => (w.to_uppercase(), a.trim()),
        None => (t.to_uppercase(), ""),
    };
    let word = ALIASES.iter().find(|(a, _)| *a == word).map_or(word.clone(), |(_, n)| n.to_string());
    let bad = || format!("cannot read {t:?}");
    let post = || parse_postfix(arg).ok_or_else(bad);
    if arg.is_empty() {
        if let Some(i) = ONE.iter().position(|&n| n == word) {
            return Ok(vec![0x40 + i as u8]);
        }
        if word == "END" || word == ".END." {
            return Ok(vec![0xC0, 0x00, 0x0D]);
        }
        if let Some((&(m, f), _)) = names.iter().find(|(_, n)| n.as_str() == word) {
            return Ok(vec![0xA0 | (m >> 2), ((m & 3) << 6) | f]);
        }
        return Err(format!("unknown function {t:?}"));
    }
    match word.as_str() {
        "LBL" => {
            if let Some(name) = unquote(arg) {
                let name = text_bytes(name, 14)?;
                let mut out = vec![0xC0, 0x00, 0xF1 + name.len() as u8, 0x00];
                out.extend(name);
                return Ok(out);
            }
            let p = parse_postfix(arg).filter(|v| v & 0x80 == 0).ok_or_else(bad)?;
            Ok(if p <= 14 { vec![p + 1] } else { vec![0xCF, p] })
        }
        "GTO" | "XEQ" | "W" => {
            let lead = match word.as_str() {
                "GTO" => 0x1D,
                "XEQ" => 0x1E,
                _ => 0x1F,
            };
            if let Some(name) = unquote(arg) {
                let name = text_bytes(name, 15)?;
                let mut out = vec![lead, 0xF0 | name.len() as u8];
                out.extend(name);
                return Ok(out);
            }
            let p = parse_postfix(arg).ok_or_else(bad)?;
            if p & 0x80 != 0 {
                return Ok(vec![0xAE, if word == "XEQ" { p } else { p & 0x7F }]);
            }
            match word.as_str() {
                "GTO" if p <= 14 => Ok(vec![0xB1 + p, 0x00]),
                "GTO" => Ok(vec![0xD0, 0x00, p]),
                "XEQ" => Ok(vec![0xE0, 0x00, p]),
                _ => Err(bad()),
            }
        }
        "RCL" | "STO" => {
            let p = post()?;
            let short = if word == "RCL" { 0x20 } else { 0x30 };
            Ok(if p <= 15 { vec![short + p] } else { vec![if word == "RCL" { 0x90 } else { 0x91 }, p] })
        }
        "X<>" => Ok(vec![0xCE, post()?]),
        "XROM" => {
            let (m, f) = arg.split_once(',').ok_or_else(bad)?;
            let (m, f): (u8, u8) = (m.trim().parse().map_err(|_| bad())?, f.trim().parse().map_err(|_| bad())?);
            if m > 31 || f > 63 {
                return Err(bad());
            }
            Ok(vec![0xA0 | (m >> 2), ((m & 3) << 6) | f])
        }
        w => {
            if let Some(i) = ROW9.iter().position(|&n| n == w) {
                return Ok(vec![0x90 + i as u8, post()?]);
            }
            if let Some(i) = FLAGS.iter().position(|&n| n == w) {
                return Ok(vec![0xA8 + i as u8, post()?]);
            }
            Err(format!("unknown function {t:?}"))
        }
    }
}

/// A listing to the bytes of a .raw file. Step numbers are optional. The
/// last END becomes the end mark that .raw files carry.
pub fn encode(text: &str, names: &Names) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    let mut last_number = false;
    for (n, line) in text.lines().enumerate() {
        let t = body(line);
        if t.is_empty() || t.starts_with('#') || t.starts_with(';') {
            continue;
        }
        let bytes = encode_one(t, names).map_err(|e| format!("line {}: {e}", n + 1))?;
        let is_number = (0x10..=0x1C).contains(&bytes[0]);
        if is_number && last_number {
            out.push(0x00);
        }
        last_number = is_number;
        out.extend(bytes);
    }
    let ends_with_end = out.len() >= 3 && out[out.len() - 3..] == [0xC0, 0x00, 0x0D];
    if ends_with_end {
        out.truncate(out.len() - 3);
    }
    out.extend([0xC0, 0x02, 0x2F]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(listing: &str) -> String {
        let names = builtin_names();
        let bytes = encode(listing, &names).unwrap();
        decode(&bytes, &names).iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn every_character_code_has_a_character_of_its_own() {
        let mut seen = std::collections::HashSet::new();
        for b in 0..=255u8 {
            let c = hp_char(b);
            assert!(seen.insert(c), "{b} repeats {c}");
            assert_eq!(from_hp_char(c), Some(b));
        }
    }

    #[test]
    fn a_program_reads_back_as_written() {
        let src = "LBL \"AREA\"\n\"R?\"\nPROMPT\nX^2\nPI\n*\n\"⊢ M2\"\nRCL 05\nSTO 16\nSTO IND X\nRCL M\n\
                   FIX 4\nSF 21\nFS?C 05\nISG 00\nGTO 01\nGTO 22\nGTO A\nXEQ \"SUB\"\nXEQ 15\nXEQ IND 07\n\
                   GTO IND Z\nLBL 01\nLBL 22\nLBL a\nX<> L\nXROM 30,12\nCRFLAS\n1.5\n-2\n1 E-3\nE3\nEND";
        let mut want: Vec<&str> = src.lines().collect();
        *want.last_mut().unwrap() = ".END.";
        assert_eq!(round(src), want.join("\n"));
    }

    #[test]
    fn two_numbers_in_a_row_stay_two() {
        let bytes = encode("1\n2\nEND", &builtin_names()).unwrap();
        assert_eq!(bytes, vec![0x11, 0x00, 0x12, 0xC0, 0x02, 0x2F]);
    }

    #[test]
    fn short_forms_are_used_where_the_calculator_uses_them() {
        let n = builtin_names();
        assert_eq!(encode_one("RCL 05", &n).unwrap(), vec![0x25]);
        assert_eq!(encode_one("RCL 16", &n).unwrap(), vec![0x90, 16]);
        assert_eq!(encode_one("LBL 14", &n).unwrap(), vec![0x0F]);
        assert_eq!(encode_one("LBL 15", &n).unwrap(), vec![0xCF, 15]);
        assert_eq!(encode_one("GTO 03", &n).unwrap(), vec![0xB4, 0x00]);
        assert_eq!(encode_one("LBL \"AB\"", &n).unwrap(), vec![0xC0, 0x00, 0xF3, 0x00, b'A', b'B']);
        assert_eq!(encode_one("CRFLAS", &n).unwrap(), vec![0xA6, 0x4A]);
    }

    #[test]
    fn step_numbers_and_label_marks_are_ignored() {
        assert_eq!(round("01*LBL \"X\"\n02 X<>Y\n03 END"), "LBL \"X\"\nX<>Y\n.END.");
    }

    #[test]
    fn a_bad_line_is_named_by_its_number() {
        let e = encode("LBL \"X\"\nFOO\nEND", &builtin_names()).unwrap_err();
        assert!(e.starts_with("line 2:"), "{e}");
    }

    #[test]
    fn rpnx_names_are_accepted() {
        assert_eq!(round("ENTER\nX#0?\nEND"), "ENTER^\nX≠0?\n.END.");
    }
}
