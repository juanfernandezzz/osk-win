//! The shared keyboard controller: everything a click does.
//!
//! Both UIs route key clicks through this module, which owns the
//! modifier engine. The UIs keep only rendering, key repeat, and the
//! event loop, so a behavior fix lands once and every surface gets it.

use crate::events::{tap_events, OskEvent, TAP_MAX_EVENTS};
use crate::key::{Click, Key, KeyKind};
use crate::mods::{mod_index, ModKeyState, ModifierEngine, MomentaryClick, MOD_SHIFT};
use crate::typekey::{CharMap, KeySink};

/// What [`Controller::click_key`] decided to do, for UI feedback
/// (flash, pressed look, key repeat).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clicked {
    /// A gap key: nothing happened.
    Inert,
    /// A momentary modifier: one press+release injected.
    Momentary,
    /// A latching modifier ran the 3-state machine: slot index and
    /// new state.
    Latch(usize, ModKeyState),
    /// Caps Lock toggled; `true` when now on.
    Caps(bool),
    /// An ordinary key was tapped.
    Tapped,
}

/// The controller state shared by every UI.
pub struct Controller {
    mods: ModifierEngine,
    charmap: Option<CharMap>,
}

impl Default for Controller {
    fn default() -> Self {
        Self::new()
    }
}

impl Controller {
    pub fn new() -> Self {
        Controller {
            mods: ModifierEngine::new(),
            charmap: CharMap::new(),
        }
    }

    // ---- read accessors for rendering ----

    pub fn mods(&self) -> &ModifierEngine {
        &self.mods
    }

    pub fn charmap(&self) -> Option<&CharMap> {
        self.charmap.as_ref()
    }

    // ---- the click path ----

    /// Send a deferred standalone tap (Win waiting out the
    /// double-click window) once it is due. Hosts call this when
    /// [`Controller::tick_due`] passes. Returns whether a tap was sent.
    pub fn tick(&mut self, now: u64, sink: &mut dyn KeySink) -> bool {
        match self.mods.take_due_tap(now, false) {
            Some(code) => {
                sink.press(code);
                sink.release(code);
                true
            }
            None => false,
        }
    }

    /// When [`Controller::tick`] has work, in ms; None = nothing
    /// pending, so the host needs no timer.
    pub fn tick_due(&self) -> Option<u64> {
        self.mods.deferred_due()
    }

    /// Send a pending deferred tap right away (before another key).
    fn flush_deferred(&mut self, sink: &mut dyn KeySink) {
        if let Some(code) = self.mods.take_due_tap(0, true) {
            sink.press(code);
            sink.release(code);
        }
    }

    /// Route one key click by [`Click`](crate::key::Click): momentary
    /// press+release, the 3-state latch machine, Caps Lock, or an
    /// ordinary tap. `now` is the click time in ms.
    pub fn click_key(&mut self, k: &Key, now: u64, sink: &mut dyn KeySink) -> Clicked {
        let (code, click) = match k.kind {
            KeyKind::Inject { code, click } => (code.get(), click),
            KeyKind::Gap => return Clicked::Inert,
        };
        // A deferred Win tap goes out before any other key, so the order
        // stays what was clicked. A click on that same key decides the
        // double-click instead.
        let same_slot = click == Click::Momentary && self.mods.deferred_slot() == mod_index(code);
        if !same_slot {
            self.flush_deferred(sink);
        }
        match click {
            Click::Momentary => {
                // A single click is one standalone press+release, sent at
                // once: Alt must be a real tap so Unity-style mouse-mode
                // toggles fire, and a lone Super opens the shell search.
                // A double-click locks the slot like Ctrl (the modifier
                // is then held around every tap); a click unlocks it.
                match self.mods.momentary_click(code, now) {
                    MomentaryClick::Tap => {
                        sink.press(code);
                        sink.release(code);
                        Clicked::Momentary
                    }
                    // Win waits out the double-click window; the host
                    // sends it from tick() when due.
                    MomentaryClick::Deferred => Clicked::Momentary,
                    MomentaryClick::Locked | MomentaryClick::Unlocked => {
                        let slot = mod_index(code).unwrap_or(MOD_SHIFT);
                        Clicked::Latch(slot, self.mods.state(slot))
                    }
                }
            }
            Click::Latch => {
                // 3-state machine: OFF -> LATCHED -> LOCKED -> OFF
                // (double-click locks). A latch key is always a
                // modifier; the engine no-ops on any other code.
                self.mods.click(code, now);
                let slot = mod_index(code).unwrap_or(MOD_SHIFT);
                Clicked::Latch(slot, self.mods.state(slot))
            }
            Click::Caps => {
                self.mods.toggle_caps();
                Clicked::Caps(self.mods.caps_lock())
            }
            Click::Tap => {
                self.tap_key(k, sink);
                Clicked::Tapped
            }
        }
    }

    /// Tap an injectable key: build the chord, inject it, consume
    /// latched modifiers. Non-tap keys are ignored.
    pub fn tap_key(&mut self, k: &Key, sink: &mut dyn KeySink) {
        let Some(tap) = k.tap() else {
            return;
        };
        let casing = self.mods.casing();
        let sticky = self.mods.sticky();
        let mut ev = [OskEvent {
            code: 0,
            press: false,
        }; TAP_MAX_EVENTS];
        let Ok(n) = tap_events(tap, casing, &sticky, &mut ev) else {
            return;
        };
        for e in &ev[..n] {
            if e.press {
                sink.press(e.code);
            } else {
                sink.release(e.code);
            }
        }
        self.mods.consume_latched();
    }
}

// ---- tests: the behavior contract through the controller ----

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::key;
    use crate::mods::MOD_ALT;

    /// Every injected event sequence, press/release in order.
    #[derive(Default)]
    struct Rec {
        ev: Vec<OskEvent>,
    }

    impl Rec {
        fn codes(&self, press: Option<bool>) -> Vec<u16> {
            self.ev
                .iter()
                .filter(|e| press.is_none_or(|p| e.press == p))
                .map(|e| e.code)
                .collect()
        }
    }

    impl KeySink for Rec {
        fn press(&mut self, code: u16) {
            self.ev.push(OskEvent { code, press: true });
        }
        fn release(&mut self, code: u16) {
            self.ev.push(OskEvent { code, press: false });
        }
    }

    fn ctl() -> Controller {
        Controller::new()
    }

    const K_A_KEY: Key = Key::letter("a", key::KEY_A);

    #[test]
    fn plain_tap_injects() {
        let mut c = ctl();
        let mut s = Rec::default();
        c.tap_key(&K_A_KEY, &mut s);
        assert_eq!(s.codes(None), &[key::KEY_A, key::KEY_A]);
        assert_eq!(s.codes(Some(true)), &[key::KEY_A]);
        assert_eq!(s.codes(Some(false)), &[key::KEY_A]);
    }

    #[test]
    fn latched_shift_wraps_the_tap_and_is_consumed() {
        let mut c = ctl();
        let mut s = Rec::default();
        let shift = Key::latch("\u{21e7}", key::KEY_LEFTSHIFT, 45);
        assert_eq!(
            c.click_key(&shift, 100, &mut s),
            Clicked::Latch(crate::mods::MOD_SHIFT, ModKeyState::Latched)
        );
        c.tap_key(&K_A_KEY, &mut s);
        // shift press, a press/release, shift release
        assert_eq!(s.codes(None), &[
            key::KEY_LEFTSHIFT,
            key::KEY_A,
            key::KEY_A,
            key::KEY_LEFTSHIFT,
        ]);
        // consumed after exactly one tap
        assert_eq!(c.mods().state(crate::mods::MOD_SHIFT), ModKeyState::Off);
    }

    #[test]
    fn momentary_key_injects_one_press_and_release() {
        let mut c = ctl();
        let mut s = Rec::default();
        let alt = Key::momentary("alt", key::KEY_LEFTALT, 25);
        assert_eq!(c.click_key(&alt, 100, &mut s), Clicked::Momentary);
        assert_eq!(s.codes(None), &[key::KEY_LEFTALT, key::KEY_LEFTALT]);
        assert_eq!(c.mods().state(MOD_ALT), ModKeyState::Off);
    }

    #[test]
    fn slow_momentary_clicks_each_tap() {
        let mut c = ctl();
        let mut s = Rec::default();
        let alt = Key::momentary("alt", key::KEY_LEFTALT, 25);
        assert_eq!(c.click_key(&alt, 100, &mut s), Clicked::Momentary);
        assert_eq!(c.click_key(&alt, 900, &mut s), Clicked::Momentary);
        assert_eq!(s.codes(Some(true)), &[key::KEY_LEFTALT, key::KEY_LEFTALT]);
        assert_eq!(c.mods().state(MOD_ALT), ModKeyState::Off);
    }

    #[test]
    fn double_clicked_momentary_locks_holds_and_unlocks() {
        let mut c = ctl();
        let mut s = Rec::default();
        let alt = Key::momentary("alt", key::KEY_LEFTALT, 25);
        let k = Key::letter("C", key::KEY_C);
        // The first click taps at once; the quick second click locks
        // without sending a second tap.
        assert_eq!(c.click_key(&alt, 100, &mut s), Clicked::Momentary);
        assert_eq!(
            c.click_key(&alt, 300, &mut s),
            Clicked::Latch(MOD_ALT, ModKeyState::Locked)
        );
        assert_eq!(s.codes(Some(true)), &[key::KEY_LEFTALT]);
        // Locked: Alt is held around every tap, and stays locked.
        s.ev.clear();
        c.click_key(&k, 400, &mut s);
        c.click_key(&k, 500, &mut s);
        assert_eq!(
            s.codes(Some(true)),
            &[key::KEY_LEFTALT, key::KEY_C, key::KEY_LEFTALT, key::KEY_C]
        );
        assert_eq!(c.mods().state(MOD_ALT), ModKeyState::Locked);
        // A click unlocks without a tap; the next key is plain.
        s.ev.clear();
        assert_eq!(c.click_key(&alt, 2000, &mut s), Clicked::Latch(MOD_ALT, ModKeyState::Off));
        c.click_key(&k, 2100, &mut s);
        assert_eq!(s.codes(Some(true)), &[key::KEY_C]);
    }

    #[test]
    fn win_tap_waits_out_the_double_click_window() {
        let mut c = ctl();
        let mut s = Rec::default();
        let win = Key::momentary("win", key::KEY_LEFTMETA, 25);
        assert_eq!(c.click_key(&win, 100, &mut s), Clicked::Momentary);
        assert!(s.ev.is_empty(), "nothing sent before the window passes");
        let due = c.tick_due().expect("a deferred tap is pending");
        assert!(!c.tick(due - 1, &mut s), "not due yet");
        assert!(c.tick(due, &mut s), "due: the tap goes out");
        assert_eq!(s.codes(None), &[key::KEY_LEFTMETA, key::KEY_LEFTMETA]);
        assert_eq!(c.tick_due(), None, "nothing left pending");
    }

    #[test]
    fn double_clicked_win_locks_without_any_tap() {
        let mut c = ctl();
        let mut s = Rec::default();
        let win = Key::momentary("win", key::KEY_LEFTMETA, 25);
        c.click_key(&win, 100, &mut s);
        assert_eq!(
            c.click_key(&win, 300, &mut s),
            Clicked::Latch(crate::mods::MOD_META, ModKeyState::Locked)
        );
        assert!(!c.tick(10_000, &mut s), "the deferred tap was dropped");
        assert!(s.ev.is_empty(), "a double-click must not tap Super: {:?}", s.ev);
        let k = Key::letter("E", key::KEY_E);
        c.click_key(&k, 400, &mut s);
        assert_eq!(s.codes(Some(true)), &[key::KEY_LEFTMETA, key::KEY_E]);
    }

    #[test]
    fn a_key_after_a_win_click_keeps_the_order() {
        let mut c = ctl();
        let mut s = Rec::default();
        let win = Key::momentary("win", key::KEY_LEFTMETA, 25);
        let k = Key::letter("E", key::KEY_E);
        c.click_key(&win, 100, &mut s);
        c.click_key(&k, 150, &mut s);
        assert_eq!(s.codes(Some(true)), &[key::KEY_LEFTMETA, key::KEY_E]);
        assert_eq!(c.tick_due(), None, "flushed, not sent twice");
    }

    #[test]
    fn latch_key_toggles_state() {
        let mut c = ctl();
        let mut s = Rec::default();
        let shift = Key::latch("\u{21e7}", key::KEY_LEFTSHIFT, 45);
        // Outside the double-click window: OFF -> LATCHED -> OFF -> ...
        assert_eq!(
            c.click_key(&shift, 100, &mut s),
            Clicked::Latch(0, ModKeyState::Latched)
        );
        assert_eq!(c.click_key(&shift, 600, &mut s), Clicked::Latch(0, ModKeyState::Off));
        assert_eq!(
            c.click_key(&shift, 700, &mut s),
            Clicked::Latch(crate::mods::MOD_SHIFT, ModKeyState::Latched)
        );
        // Two quick clicks lock the slot.
        assert_eq!(
            c.click_key(&shift, 700 + crate::mods::DOUBLE_CLICK_MS - 1, &mut s),
            Clicked::Latch(0, ModKeyState::Locked)
        );
    }

    #[test]
    fn caps_toggles_and_gaps_are_inert() {
        let mut c = ctl();
        let mut s = Rec::default();
        let caps = Key::caps("caps", key::KEY_CAPSLOCK, 45);
        assert_eq!(c.click_key(&caps, 0, &mut s), Clicked::Caps(true));
        assert_eq!(c.click_key(&caps, 10, &mut s), Clicked::Caps(false));
        let gap = Key::inert(None, 10);
        assert_eq!(c.click_key(&gap, 0, &mut s), Clicked::Inert);
        assert!(s.codes(None).is_empty());
    }

}
