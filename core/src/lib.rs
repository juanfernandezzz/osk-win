//! Pure logic for the osk on-screen keyboard: layouts, the modifier
//! engine, tap events, and char-to-key mapping.
//!
//! No Wayland, no `unsafe`; everything here is testable headless, and
//! [`selftest::run`] is the behavior contract, ported from the C
//! reference in `legacy/src/selftest.c`.
//!
//! Module map:
//! - [`controller`]: the click path both UIs share (taps and
//!   modifier dispatch).
//! - [`key`]: KEY_* event codes and the Key/Row/Layout types.
//! - [`layout`]: the 65% keyboard profile.
//! - [`metrics`]: the one spacing scale both front ends draw with.
//! - [`mods`]: the 3-state modifier engine (OFF / LATCHED / LOCKED).
//! - [`events`]: tap event sequences and sticky-modifier handling.
//! - [`interaction`]: the pointer/touch interaction state machine.
//! - [`typekey`]: char-to-key mapping and string typing.
//! - [`selftest`]: the headless self-test suite.

#![forbid(unsafe_code)]

pub mod controller;
pub mod events;
pub mod interaction;
pub mod key;
pub mod layout;
pub mod metrics;
pub mod mods;
pub mod selftest;
pub mod typekey;

pub use events::{OskEvent, StickyMods, TAP_MAX_EVENTS};
pub use key::{Click, Key, KeyKind, Layout, Row, TapKey, nz};
pub use layout::LAYOUT;
pub use mods::{Casing, ModKeyState, ModifierEngine};
