//! Regression tests covering defect areas found during interrogation.

use oskwin_core::typekey::{type_text, CharMap, KeySink};

struct TestSink(Vec<(u16, bool)>);
impl KeySink for TestSink {
    fn press(&mut self, code: u16) {
        self.0.push((code, true));
    }
    fn release(&mut self, code: u16) {
        self.0.push((code, false));
    }
}

#[test]
fn type_text_crlf_and_tabs() {
    let map = match CharMap::new() {
        Some(m) => m,
        None => return, // xkbcommon not available in headless env
    };

    let mut sink = TestSink(Vec::new());
    let res = type_text(&mut sink, &map, "hello\r\nworld\ttest", || {});
    assert!(res.is_ok(), "type_text must accept CRLF and tab characters: {:?}", res);

    // Verify key codes: enter (28), tab (15)
    use oskwin_core::key::key;
    let has_enter = sink.0.iter().any(|(code, _)| *code == key::KEY_ENTER);
    let has_tab = sink.0.iter().any(|(code, _)| *code == key::KEY_TAB);
    assert!(has_enter, "CRLF must inject KEY_ENTER");
    assert!(has_tab, "Tab must inject KEY_TAB");
}

#[test]
fn multiline_typing_emits_enter_between_lines() {
    let map = match CharMap::new() {
        Some(m) => m,
        None => return,
    };
    let mut sink = TestSink(Vec::new());
    let res = type_text(&mut sink, &map, "a\nb", || {});
    assert!(res.is_ok());

    use oskwin_core::key::key;
    let expected = vec![
        (key::KEY_A, true),
        (key::KEY_A, false),
        (key::KEY_ENTER, true),
        (key::KEY_ENTER, false),
        (key::KEY_B, true),
        (key::KEY_B, false),
    ];
    assert_eq!(sink.0, expected);
}

#[test]
fn numpad_rows_sum_to_20_5u() {
    use oskwin_core::layout::LAYOUT_NUMPAD;
    for (i, row) in LAYOUT_NUMPAD.rows.iter().enumerate() {
        let sum: u16 = row.keys.iter().map(|k| k.width).sum();
        assert_eq!(sum, 410, "65%+Num row {} must sum to 410 (20.5u)", i);
    }
}

