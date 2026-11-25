//! The keys of USER mode: functions in the key assignment registers and
//! programs whose global label has a key, drawn on the DM41X keyboard.

use crate::focal::{self, Names};
use crate::state::State;
use crate::{DIM, GOLD, SEL_BG};
use crust::style;
use std::path::Path;

/// A key with a function or program on it.
#[derive(Debug, PartialEq)]
pub struct Assigned {
    /// The keycode the calculator shows: row and column, minus when shifted.
    pub code: i32,
    pub name: String,
    pub program: bool,
}

/// The keycode for a key byte: column less one, then row less one, plus 8
/// when shifted. Row 4 counts ENTER as two columns, so CHS is 2 and ← is 4.
pub fn user_code(k: u8) -> i32 {
    let row = (k & 7) as i32 + 1;
    let n = (k >> 4) as i32;
    let col = if row == 4 { n.max(1) } else { n + 1 };
    let code = row * 10 + col;
    if k & 8 != 0 {
        -code
    } else {
        code
    }
}

/// Every assigned key, in keyboard order. The registers from 0x0C0 up hold
/// two functions each while their first byte is F0; a function on a key
/// wins over a program on the same key, as on the calculator.
pub fn assigned(st: &State, names: &Names) -> Vec<Assigned> {
    let mut out: Vec<Assigned> = vec![];
    for a in 0x0C0..0x200 {
        let v = st.reg(a);
        if v >> 48 != 0xF0 {
            break;
        }
        for slot in [v >> 24 & 0xFF_FFFF, v & 0xFF_FFFF] {
            let (f1, f2, k) = ((slot >> 16) as u8, (slot >> 8) as u8, slot as u8);
            if f1 | f2 != 0 {
                out.push(Assigned { code: user_code(k), name: focal::key_function(f1, f2, names), program: false });
            }
        }
    }
    // a global label keeps its key, plus one, in the byte before its name
    let code = st.code();
    for s in focal::decode(&code, names) {
        let k = code.get(s.at + 3).copied().unwrap_or(0);
        if let (Some(name), true) = (s.text.strip_prefix("LBL \""), k > 0) {
            let c = user_code(k - 1);
            if !out.iter().any(|a| a.code == c) {
                out.push(Assigned { code: c, name: name.trim_end_matches('"').to_string(), program: true });
            }
        }
    }
    out.sort_by_key(|a| (a.code.abs(), a.code < 0));
    out
}

/// The DM41X keyboard, row by row: each key's keycode (0 for the keys the
/// HP-41 lacks), its face and its gold shifted function.
const BOARD: [&[(i32, &str, &str)]; 8] = [
    &[(11, "Σ+", "Σ-"), (12, "1/x", "yˣ"), (13, "√x", "x²"), (14, "LOG", "10ˣ"), (15, "LN", "eˣ"), (0, "ON", "CLK")],
    &[(21, "x≷y", "CLΣ"), (22, "R↓", "%"), (23, "SIN", "SIN⁻¹"), (24, "COS", "COS⁻¹"), (25, "TAN", "TAN⁻¹"), (0, "USR", "SETUP")],
    &[(31, "shift", ""), (32, "XEQ", "ASN"), (33, "STO", "LBL"), (34, "RCL", "GTO"), (35, "SST", "BST"), (0, "α", "")],
    &[(41, "ENTER↑", "CATALOG"), (42, "CHS", "ISG"), (43, "EEX", "RTN"), (0, "DSP", "PRN"), (44, "←", "CLx/A")],
    &[(51, "−", "x=y?"), (52, "7", "SF"), (53, "8", "CF"), (54, "9", "FS?"), (0, "▲", "")],
    &[(61, "+", "x≤y?"), (62, "4", "BEEP"), (63, "5", "P→R"), (64, "6", "R→P"), (0, "▼", "")],
    &[(71, "×", "x>y?"), (72, "1", "FIX"), (73, "2", "SCI"), (74, "3", "ENG"), (0, "PRG", "SIZE")],
    &[(81, "÷", "x=0?"), (82, "0", "π"), (83, ".", "LASTx"), (0, "CST", "CONF"), (84, "R/S", "VIEW")],
];

/// One line of a key: its text cut to the key's width, then a gap.
fn cell(text: &str, w: usize, colour: u8, bold: bool, picked: bool) -> String {
    let t: String = text.chars().take(w.saturating_sub(1)).collect();
    let t = format!("{t:<0$}", w.saturating_sub(1));
    let t = if bold { style::bold(&t) } else { t };
    let t = if picked { style::fb(&t, 255, SEL_BG) } else { style::fg(&t, colour) };
    t + " "
}

/// The keyboard with the assignments on it, `width` columns wide: two lines
/// a key, the shifted one in gold above the key itself. `picked` is the
/// keycode to show in reverse.
pub fn board(keys: &[Assigned], picked: Option<i32>, width: usize) -> Vec<String> {
    let unit = (width.saturating_sub(1) / 6).clamp(4, 14);
    let mut out = vec![];
    for (r, row) in BOARD.iter().enumerate() {
        let (mut gold, mut face) = (String::from(" "), String::from(" "));
        for &(code, label, shifted) in row.iter() {
            let w = match (r, code) {
                (3, 41) => 2 * unit,
                (3, _) | (0..=2, _) => unit,
                _ => unit * 6 / 5,
            };
            let on = |c: i32| keys.iter().find(|a| code != 0 && a.code == c);
            let colour = |a: &Assigned, c: u8| if a.program { 114 } else { c };
            gold += &match on(-code) {
                Some(a) => cell(&a.name, w, colour(a, GOLD), true, picked == Some(-code)),
                None => cell(shifted, w, if code == 0 { 239 } else { 94 }, false, false),
            };
            face += &match on(code) {
                Some(a) => cell(&a.name, w, colour(a, 255), true, picked == Some(code)),
                None => cell(label, w, if code == 0 { 239 } else { DIM }, false, false),
            };
        }
        out.extend([gold, face, String::new()]);
    }
    out
}

/// The CST menus in a KEYS folder: each file's name with its keys A to P and
/// the three shifted ones (SHIFT ▲, SHIFT ▼, SHIFT α), as `A.ASTO` lines.
pub fn cst(dir: &Path) -> Vec<(String, Vec<(String, String)>)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<(String, Vec<(String, String)>)> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("cst")))
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path()).ok()?;
            let keys = text
                .lines()
                .filter_map(|l| l.split_once('.'))
                .map(|(k, f)| (k.trim().to_string(), f.trim().to_string()))
                .collect();
            Some((e.file_name().to_string_lossy().into_owned(), keys))
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_bytes_name_the_keys_the_calculator_shows() {
        assert_eq!(user_code(0x00), 11);
        assert_eq!(user_code(0x40), 15);
        assert_eq!(user_code(0x0A), -31);
        assert_eq!(user_code(0x4A), -35);
        assert_eq!(user_code(0x21), 23);
        assert_eq!(user_code(0x0B), -41);
        assert_eq!(user_code(0x4B), -44);
        assert_eq!(user_code(0x2F), -83);
        assert_eq!(user_code(0x37), 84);
    }

    #[test]
    fn registers_and_labels_give_the_assigned_keys() {
        let mut text = String::from("DM41\n");
        // SIN on 15, HMS+ on shifted R/S, then the end of the registers
        text.push_str("c0  f004594004493f  99010000000000  00000000000000  00000000000000\n");
        let mut st = State::parse(&text).unwrap();
        // c: R00 at 0x1C0, .END. in 0x1B0; one program LBL "AB" on key 23 (byte 0x21 + 1)
        st.regs[0x0D] = 0x1C0 << 12 | 0x1B0;
        st.regs[0x1BF] = 0xC000F322414200;
        st.regs[0x1BE] = 0xC0000D00000000;
        st.regs[0x1B0] = 0x00000000C0022F;
        let keys = assigned(&st, &focal::builtin_names());
        let got: Vec<(i32, &str, bool)> = keys.iter().map(|a| (a.code, a.name.as_str(), a.program)).collect();
        assert_eq!(got, [(15, "SIN", false), (23, "AB", true), (-84, "HMS+", false)]);
    }

    #[test]
    fn cst_files_give_their_keys() {
        let dir = std::env::temp_dir().join(format!("dm41-cst-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("normal.cst"), "A.ASTO\nB.\n1.\n").unwrap();
        let got = cst(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].1[0], ("A".to_string(), "ASTO".to_string()));
        assert_eq!(got[0].1[1], ("B".to_string(), String::new()));
    }
}
