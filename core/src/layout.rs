//! The 65% keyboard profile.
//!
//! Key codes are linux input event codes (KEY_*), exactly what both
//! injection backends consume.
//!
//! Widths are in 1/20 key units (20 = 1.0u). A gap key is inert:
//! drawn when labeled, never injected. `Key::inert(None, _)` is a
//! spacing gap.

use crate::key::{key, Key, Layout, Row};

// ---- 65%: number row through modifier row, nav keys on the right
// ---- edge, arrows bottom-right. All rows sum to 320 (16.0u).
const P65_ROW0: [Key; 15] = [
     Key::wide("esc", key::KEY_ESC, 20),
    Key::sym("1", key::KEY_1, "!"),
    Key::sym("2", key::KEY_2, "@"),
    Key::sym("3", key::KEY_3, "#"),
    Key::sym("4", key::KEY_4, "$"),
    Key::sym("5", key::KEY_5, "%"),
    Key::sym("6", key::KEY_6, "^"),
    Key::sym("7", key::KEY_7, "&"),
    Key::sym("8", key::KEY_8, "*"),
    Key::sym("9", key::KEY_9, "("),
    Key::sym("0", key::KEY_0, ")"),
    Key::sym("-", key::KEY_MINUS, "_"),
    Key::sym("=", key::KEY_EQUAL, "+"),
     Key::wide_repeat("\u{232B}", key::KEY_BACKSPACE, 40), // ⌫
     Key::wide("home", key::KEY_HOME, 20),
];

const P65_ROW1: [Key; 15] = [
     Key::wide("\u{21E5}", key::KEY_TAB, 30), // ⇥
    Key::letter("Q", key::KEY_Q),
    Key::letter("W", key::KEY_W),
    Key::letter("E", key::KEY_E),
    Key::letter("R", key::KEY_R),
    Key::letter("T", key::KEY_T),
    Key::letter("Y", key::KEY_Y),
    Key::letter("U", key::KEY_U),
    Key::letter("I", key::KEY_I),
    Key::letter("O", key::KEY_O),
    Key::letter("P", key::KEY_P),
    Key::sym("[", key::KEY_LEFTBRACE, "{"),
    Key::sym("]", key::KEY_RIGHTBRACE, "}"),
    Key::sym_wide("\\", key::KEY_BACKSLASH, "|", 30),
     Key::wide("pgup", key::KEY_PAGEUP, 20),
];

const P65_ROW2: [Key; 14] = [
     Key::caps("caps", key::KEY_CAPSLOCK, 35),
    Key::letter("A", key::KEY_A),
    Key::letter("S", key::KEY_S),
    Key::letter("D", key::KEY_D),
    Key::letter("F", key::KEY_F),
    Key::letter("G", key::KEY_G),
    Key::letter("H", key::KEY_H),
    Key::letter("J", key::KEY_J),
    Key::letter("K", key::KEY_K),
    Key::letter("L", key::KEY_L),
    Key::sym(";", key::KEY_SEMICOLON, ":"),
    Key::sym("'", key::KEY_APOSTROPHE, "\""),
     Key::wide("\u{21B5}", key::KEY_ENTER, 45), // ↵
     Key::wide("pgdn", key::KEY_PAGEDOWN, 20),
];

const P65_ROW3: [Key; 14] = [
     Key::latch("\u{21E7}", key::KEY_LEFTSHIFT, 45), // ⇧
    Key::letter("Z", key::KEY_Z),
    Key::letter("X", key::KEY_X),
    Key::letter("C", key::KEY_C),
    Key::letter("V", key::KEY_V),
    Key::letter("B", key::KEY_B),
    Key::letter("N", key::KEY_N),
    Key::letter("M", key::KEY_M),
    Key::sym(",", key::KEY_COMMA, "<"),
    Key::sym(".", key::KEY_DOT, ">"),
    Key::sym("/", key::KEY_SLASH, "?"),
     Key::latch("\u{21E7}", key::KEY_RIGHTSHIFT, 35), // ⇧
    Key::wide_repeat("\u{2191}", key::KEY_UP, 20),             // ↑
    Key::wide("end", key::KEY_END, 20),
];

const P65_ROW4: [Key; 10] = [
     Key::latch("ctrl", key::KEY_LEFTCTRL, 25),
     Key::momentary("win", key::KEY_LEFTMETA, 25),
     Key::momentary("alt", key::KEY_LEFTALT, 25),
    Key::space(125),
     Key::momentary("alt", key::KEY_RIGHTALT, 25),
     Key::latch("ctrl", key::KEY_RIGHTCTRL, 25),
    // 0.5u gap before the arrow cluster
    Key::inert(None, 10),
    Key::wide_repeat("\u{2190}", key::KEY_LEFT, 20), // ←
    Key::wide_repeat("\u{2193}", key::KEY_DOWN, 20), // ↓
    Key::wide_repeat("\u{2192}", key::KEY_RIGHT, 20), // →
];

const ROWS_65: [Row; 5] = [
    Row { keys: &P65_ROW0 },
    Row { keys: &P65_ROW1 },
    Row { keys: &P65_ROW2 },
    Row { keys: &P65_ROW3 },
    Row { keys: &P65_ROW4 },
];

pub const LAYOUT_65: Layout = Layout {
    name: "65%",
    rows: &ROWS_65,
};

// ---- 65% + Numpad: 20.5u (410 units) wide, 65% layout with numeric keypad on the right ----
const NUMPAD_ROW0: [Key; 20] = [
    Key::wide("esc", key::KEY_ESC, 20),
    Key::sym("1", key::KEY_1, "!"),
    Key::sym("2", key::KEY_2, "@"),
    Key::sym("3", key::KEY_3, "#"),
    Key::sym("4", key::KEY_4, "$"),
    Key::sym("5", key::KEY_5, "%"),
    Key::sym("6", key::KEY_6, "^"),
    Key::sym("7", key::KEY_7, "&"),
    Key::sym("8", key::KEY_8, "*"),
    Key::sym("9", key::KEY_9, "("),
    Key::sym("0", key::KEY_0, ")"),
    Key::sym("-", key::KEY_MINUS, "_"),
    Key::sym("=", key::KEY_EQUAL, "+"),
    Key::wide_repeat("\u{232B}", key::KEY_BACKSPACE, 40),
    Key::wide("home", key::KEY_HOME, 20),
    // 0.5u gap before the numpad
    Key::inert(None, 10),
    Key::wide("del", key::KEY_DELETE, 20),
    Key::sym_wide("/", key::KEY_KPSLASH, "/", 20),
    Key::sym_wide("*", key::KEY_KPASTERISK, "*", 20),
    Key::wide_repeat("\u{232B}", key::KEY_BACKSPACE, 20),
];

const NUMPAD_ROW1: [Key; 20] = [
    Key::wide("\u{21E5}", key::KEY_TAB, 30),
    Key::letter("Q", key::KEY_Q),
    Key::letter("W", key::KEY_W),
    Key::letter("E", key::KEY_E),
    Key::letter("R", key::KEY_R),
    Key::letter("T", key::KEY_T),
    Key::letter("Y", key::KEY_Y),
    Key::letter("U", key::KEY_U),
    Key::letter("I", key::KEY_I),
    Key::letter("O", key::KEY_O),
    Key::letter("P", key::KEY_P),
    Key::sym("[", key::KEY_LEFTBRACE, "{"),
    Key::sym("]", key::KEY_RIGHTBRACE, "}"),
    Key::sym_wide("\\", key::KEY_BACKSLASH, "|", 30),
    Key::wide("pgup", key::KEY_PAGEUP, 20),
    // 0.5u gap before the numpad
    Key::inert(None, 10),
    Key::sym("7", key::KEY_7, "&"),
    Key::sym("8", key::KEY_8, "*"),
    Key::sym("9", key::KEY_9, "("),
    Key::sym_wide("-", key::KEY_KPMINUS, "-", 20),
];

const NUMPAD_ROW2: [Key; 19] = [
    Key::caps("caps", key::KEY_CAPSLOCK, 35),
    Key::letter("A", key::KEY_A),
    Key::letter("S", key::KEY_S),
    Key::letter("D", key::KEY_D),
    Key::letter("F", key::KEY_F),
    Key::letter("G", key::KEY_G),
    Key::letter("H", key::KEY_H),
    Key::letter("J", key::KEY_J),
    Key::letter("K", key::KEY_K),
    Key::letter("L", key::KEY_L),
    Key::sym(";", key::KEY_SEMICOLON, ":"),
    Key::sym("'", key::KEY_APOSTROPHE, "\""),
    Key::wide("\u{21B5}", key::KEY_ENTER, 45),
    Key::wide("pgdn", key::KEY_PAGEDOWN, 20),
    // 0.5u gap before the numpad
    Key::inert(None, 10),
    Key::sym("4", key::KEY_4, "$"),
    Key::sym("5", key::KEY_5, "%"),
    Key::sym("6", key::KEY_6, "^"),
    Key::sym_wide("+", key::KEY_KPPLUS, "+", 20),
];

const NUMPAD_ROW3: [Key; 19] = [
    Key::latch("\u{21E7}", key::KEY_LEFTSHIFT, 45),
    Key::letter("Z", key::KEY_Z),
    Key::letter("X", key::KEY_X),
    Key::letter("C", key::KEY_C),
    Key::letter("V", key::KEY_V),
    Key::letter("B", key::KEY_B),
    Key::letter("N", key::KEY_N),
    Key::letter("M", key::KEY_M),
    Key::sym(",", key::KEY_COMMA, "<"),
    Key::sym(".", key::KEY_DOT, ">"),
    Key::sym("/", key::KEY_SLASH, "?"),
    Key::latch("\u{21E7}", key::KEY_RIGHTSHIFT, 35),
    Key::wide_repeat("\u{2191}", key::KEY_UP, 20),
    Key::wide("end", key::KEY_END, 20),
    // 0.5u gap before the numpad
    Key::inert(None, 10),
    Key::sym("1", key::KEY_1, "!"),
    Key::sym("2", key::KEY_2, "@"),
    Key::sym("3", key::KEY_3, "#"),
    Key::sym_wide("=", key::KEY_KPEQUAL, "=", 20),
];

const NUMPAD_ROW4: [Key; 14] = [
    Key::latch("ctrl", key::KEY_LEFTCTRL, 25),
    Key::momentary("win", key::KEY_LEFTMETA, 25),
    Key::momentary("alt", key::KEY_LEFTALT, 25),
    Key::space(125),
    Key::momentary("alt", key::KEY_RIGHTALT, 25),
    Key::latch("ctrl", key::KEY_RIGHTCTRL, 25),
    // 0.5u gap before the arrow cluster
    Key::inert(None, 10),
    Key::wide_repeat("\u{2190}", key::KEY_LEFT, 20),
    Key::wide_repeat("\u{2193}", key::KEY_DOWN, 20),
    Key::wide_repeat("\u{2192}", key::KEY_RIGHT, 20),
    // 0.5u gap before the numpad
    Key::inert(None, 10),
    Key::sym_wide("0", key::KEY_0, ")", 40),
    Key::sym_wide(".", key::KEY_DOT, ">", 20),
    Key::wide("\u{21B5}", key::KEY_KPENTER, 20),
];

const ROWS_NUMPAD: [Row; 5] = [
    Row { keys: &NUMPAD_ROW0 },
    Row { keys: &NUMPAD_ROW1 },
    Row { keys: &NUMPAD_ROW2 },
    Row { keys: &NUMPAD_ROW3 },
    Row { keys: &NUMPAD_ROW4 },
];

pub const LAYOUT_NUMPAD: Layout = Layout {
    name: "65%+Num",
    rows: &ROWS_NUMPAD,
};

// ---- Latino: the Windows osk.exe key layout (Windows 10, Latin American
// ---- 080a), measured from its window: main block, then the Inicio/Re Pág
// ---- block; its Nav/Subir/Opciones... option buttons are left out.
// ---- Keys inject by scancode, so Windows' own layout types the
// ---- characters; these labels only mirror it. All rows sum to 371.
const LAT_ROW0: [Key; 18] = [
    Key::wide("Esc", key::KEY_ESC, 20),
    Key::sym("|", key::KEY_GRAVE, "\u{B0}").altgr("\u{AC}"), // | ° ¬
    Key::sym("1", key::KEY_1, "!"),
    Key::sym("2", key::KEY_2, "\""),
    Key::sym("3", key::KEY_3, "#"),
    Key::sym("4", key::KEY_4, "$"),
    Key::sym("5", key::KEY_5, "%"),
    Key::sym("6", key::KEY_6, "&"),
    Key::sym("7", key::KEY_7, "/"),
    Key::sym("8", key::KEY_8, "("),
    Key::sym("9", key::KEY_9, ")"),
    Key::sym("0", key::KEY_0, "="),
    Key::sym("'", key::KEY_MINUS, "?").altgr("\\"),
    Key::sym("\u{BF}", key::KEY_EQUAL, "\u{A1}"), // ¿ ¡
    Key::wide_repeat("\u{232B}", key::KEY_BACKSPACE, 35), // ⌫
    Key::inert(None, 2),
    Key::wide("Inicio", key::KEY_HOME, 27),
    Key::wide("Re P\u{E1}g", key::KEY_PAGEUP, 27),
];

const LAT_ROW1: [Key; 17] = [
    Key::wide("Tab", key::KEY_TAB, 28),
    Key::letter("Q", key::KEY_Q).altgr("@"),
    Key::letter("W", key::KEY_W),
    Key::letter("E", key::KEY_E),
    Key::letter("R", key::KEY_R),
    Key::letter("T", key::KEY_T),
    Key::letter("Y", key::KEY_Y),
    Key::letter("U", key::KEY_U),
    Key::letter("I", key::KEY_I),
    Key::letter("O", key::KEY_O),
    Key::letter("P", key::KEY_P),
    Key::sym("\u{B4}", key::KEY_LEFTBRACE, "\u{A8}"), // ´ ¨ (dead keys)
    Key::sym("+", key::KEY_RIGHTBRACE, "*").altgr("~"),
    Key::wide("Entrar", key::KEY_ENTER, 47),
    Key::inert(None, 2),
    Key::wide("Fin", key::KEY_END, 27),
    Key::wide("Av P\u{E1}g", key::KEY_PAGEDOWN, 27),
];

const LAT_ROW2: [Key; 17] = [
    Key::caps("Bloq May", key::KEY_CAPSLOCK, 34),
    Key::letter("A", key::KEY_A),
    Key::letter("S", key::KEY_S),
    Key::letter("D", key::KEY_D),
    Key::letter("F", key::KEY_F),
    Key::letter("G", key::KEY_G),
    Key::letter("H", key::KEY_H),
    Key::letter("J", key::KEY_J),
    Key::letter("K", key::KEY_K),
    Key::letter("L", key::KEY_L),
    Key::letter("\u{D1}", key::KEY_SEMICOLON), // Ñ
    Key::sym("{", key::KEY_APOSTROPHE, "[").altgr("^"),
    Key::sym("}", key::KEY_BACKSLASH, "]").altgr("`"),
    // The lower half of osk.exe's two-row Enter.
    Key::wide("", key::KEY_ENTER, 41),
    Key::inert(None, 2),
    Key::wide("Insertar", key::KEY_INSERT, 27),
    Key::wide("Pausa", key::KEY_PAUSE, 27),
];

const LAT_ROW3: [Key; 17] = [
    Key::latch("May\u{FA}s", key::KEY_LEFTSHIFT, 50),
    Key::letter("Z", key::KEY_Z),
    Key::letter("X", key::KEY_X),
    Key::letter("C", key::KEY_C),
    Key::letter("V", key::KEY_V),
    Key::letter("B", key::KEY_B),
    Key::letter("N", key::KEY_N),
    Key::letter("M", key::KEY_M),
    Key::sym(",", key::KEY_COMMA, ";"),
    Key::sym(".", key::KEY_DOT, ":"),
    Key::sym("-", key::KEY_SLASH, "_"),
    Key::wide_repeat("\u{2191}", key::KEY_UP, 20), // ↑
    Key::latch("May\u{FA}s", key::KEY_RIGHTSHIFT, 20),
    Key::wide_repeat("Supr", key::KEY_DELETE, 25),
    Key::inert(None, 2),
    Key::wide("Impr Pant", key::KEY_SYSRQ, 27),
    Key::wide("Bloq Despl", key::KEY_SCROLLLOCK, 27),
];

const LAT_ROW4: [Key; 12] = [
    Key::wide("Fn", key::KEY_FN, 20),
    Key::latch("Ctrl", key::KEY_LEFTCTRL, 20),
    // osk.exe draws the Windows logo and a menu icon; the bundled font has neither.
    Key::momentary("Win", key::KEY_LEFTMETA, 20),
    Key::momentary("Alt", key::KEY_LEFTALT, 20),
    Key::space(112),
    Key::momentary("AltGr", key::KEY_RIGHTALT, 20),
    Key::latch("Ctrl", key::KEY_RIGHTCTRL, 20),
    Key::wide_repeat("\u{2190}", key::KEY_LEFT, 20), // ←
    Key::wide_repeat("\u{2193}", key::KEY_DOWN, 20), // ↓
    Key::wide_repeat("\u{2192}", key::KEY_RIGHT, 20), // →
    Key::wide("Men\u{FA}", key::KEY_COMPOSE, 23),
    Key::inert(None, 56),
];

const ROWS_LATINO: [Row; 5] = [
    Row { keys: &LAT_ROW0 },
    Row { keys: &LAT_ROW1 },
    Row { keys: &LAT_ROW2 },
    Row { keys: &LAT_ROW3 },
    Row { keys: &LAT_ROW4 },
];

pub const LAYOUT_LATINO: Layout = Layout {
    name: "Latino",
    rows: &ROWS_LATINO,
};

pub static ALL_LAYOUTS: &[&Layout] = &[&LAYOUT_LATINO, &LAYOUT_65, &LAYOUT_NUMPAD];

/// Default profile: 65%.
pub const LAYOUT: Layout = LAYOUT_65;


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latino_mirrors_osk_exe() {
        let sums: Vec<u16> = LAYOUT_LATINO.rows.iter().map(|r| r.keys.iter().map(|k| k.width).sum()).collect();
        assert!(sums.iter().all(|&s| s == 371), "every row lines up: {sums:?}");
        let codes: Vec<u16> = LAYOUT_LATINO.rows.iter().flat_map(|r| r.keys).map(Key::code_u16).collect();
        for code in [
            key::KEY_DELETE, key::KEY_FN, key::KEY_COMPOSE, key::KEY_INSERT, key::KEY_PAUSE,
            key::KEY_SYSRQ, key::KEY_SCROLLLOCK, key::KEY_GRAVE, key::KEY_BACKSLASH,
        ] {
            assert!(codes.contains(&code), "osk.exe has key {code}");
        }
        // osk.exe has no < > key on this layout.
        assert!(!codes.contains(&key::KEY_102ND));
        // Enter spans rows 1 and 2, as osk.exe's two-row Enter.
        assert_eq!(codes.iter().filter(|&&c| c == key::KEY_ENTER).count(), 2);
    }
}
