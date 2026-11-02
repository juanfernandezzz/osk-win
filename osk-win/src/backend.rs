//! Key-injection backend for Windows: `SendInput` with virtual-key codes.
//!
//! `press`/`release` take a Linux `KEY_*` code (the osk core's key currency)
//! and translate it to a Windows VK via [`crate::vk`]. SendInput needs no
//! elevated privileges for keyboard input, matching the Linux backends.

use std::fs::File;
use std::io::Write;
use std::os::windows::io::FromRawHandle;
use std::sync::Mutex;

use oskwin_core::typekey::KeySink;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_CONNECTED, HANDLE};
use windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_OUTBOUND};
use windows::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, VIRTUAL_KEY,
};

use crate::vk;

fn is_extended_vk(vk: u16) -> bool {
    matches!(
        vk,
        0x21..=0x28 // Page Up, Page Down, End, Home, Left, Up, Right, Down
        | 0x2D | 0x2E // Insert, Delete
        | 0x5B | 0x5C // Left Windows, Right Windows
        | 0x6F // Numpad Divide
        | 0xA3 // Right Ctrl
        | 0xA5 // Right Alt
    )
}

/// `dwExtraInfo` of every event the osk injects, so its keyboard hook
/// can tell them from a real keyboard's.
pub const OSK_MARK: usize = 0x4F53_4B;

/// The pipe to osk-win-input.exe once it connects; until then keys go out
/// from this process.
static HELPER: Mutex<Option<File>> = Mutex::new(None);

/// The pipe osk-win-input.exe reads, named after the osk's process id.
pub fn pipe_name(pid: u32) -> String {
    format!(r"\\.\pipe\osk-win-input-{pid}")
}

/// Windows drops keys a normal program sends to an app run as
/// administrator (UIPI). osk-win-input.exe, installed next to the osk and
/// signed with UIAccess, sends them instead. If it is missing or Windows
/// will not start it (its certificate is not trusted), keys go out from
/// here as before.
pub fn start_input_helper() {
    let Some(helper) = std::env::current_exe().ok().map(|e| e.with_file_name("osk-win-input.exe")) else {
        return;
    };
    if !helper.exists() {
        return;
    }
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let pid = std::process::id();
    let name = wide(&pipe_name(pid));
    let pipe = unsafe {
        CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            PIPE_ACCESS_OUTBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_WAIT,
            1,
            4096,
            0,
            0,
            None,
        )
    };
    if pipe.is_invalid() {
        return;
    }
    let (file, args) = (wide(&helper.to_string_lossy()), wide(&pid.to_string()));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_FLAG_NO_UI,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(args.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    if unsafe { ShellExecuteExW(&mut info) }.is_err() {
        let _ = unsafe { CloseHandle(pipe) };
        return;
    }
    let raw = pipe.0 as isize;
    std::thread::spawn(move || {
        let pipe = HANDLE(raw as *mut _);
        match unsafe { ConnectNamedPipe(pipe, None) } {
            Err(e) if e.code() != ERROR_PIPE_CONNECTED.to_hresult() => {
                let _ = unsafe { CloseHandle(pipe) };
            }
            _ => *HELPER.lock().unwrap_or_else(|p| p.into_inner()) = Some(unsafe { File::from_raw_handle(pipe.0) }),
        }
    });
}

/// Injection backend.
pub trait Backend {
    fn name(&self) -> &'static str;
    /// Initialize. `Err` when the backend is unavailable.
    fn init(&mut self) -> Result<(), String>;
    fn press(&mut self, code: u16);
    fn release(&mut self, code: u16);
    /// Non-blocking drain of the backend's own events. A no-op for SendInput.
    fn pump(&mut self);
    fn destroy(&mut self);
}

impl KeySink for dyn Backend {
    fn press(&mut self, code: u16) {
        Backend::press(self, code);
    }
    fn release(&mut self, code: u16) {
        Backend::release(self, code);
    }
}

/// Sized adapter so a `&mut dyn Backend` can be passed to
/// `oskwin_core::typekey::type_string`, whose `KeySink` bound is `Sized`.
pub struct BackendSink<'a> {
    pub b: &'a mut dyn Backend,
}

impl KeySink for BackendSink<'_> {
    fn press(&mut self, code: u16) {
        self.b.press(code);
    }
    fn release(&mut self, code: u16) {
        self.b.release(code);
    }
}

/// Injects keystrokes via the Windows `SendInput` API.
pub struct SendInputBackend;

impl SendInputBackend {
    pub fn new() -> Self {
        Self
    }

    /// One key event: by scancode when `scan` is nonzero (Windows'
    /// active layout picks the character), else by virtual key.
    fn send(vk: u16, scan: u16, up: bool) {
        let mut flags = if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
        if scan != 0 {
            flags |= KEYEVENTF_SCANCODE;
        } else if is_extended_vk(vk) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        let mut input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(if scan != 0 { 0 } else { vk }),
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: OSK_MARK,
                },
            },
        };
        let sent =
            unsafe { SendInput(std::slice::from_mut(&mut input), std::mem::size_of::<INPUT>() as i32) };
        if sent != 1 {
            eprintln!("osk: SendInput sent {sent}/1");
        }
    }
}

impl SendInputBackend {
    /// Linux codes 1..=88 equal the PC set-1 scancodes of the main
    /// block, so those go out by position and type what the user's
    /// Windows layout (Latin American, etc.) puts there; the rest
    /// (extended keys: arrows, Win, right Ctrl/Alt) go by virtual key.
    pub fn key(code: u16, up: bool) {
        let mut helper = HELPER.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(pipe) = helper.as_mut() {
            if pipe.write_all(&[code as u8, (code >> 8) as u8, u8::from(up)]).is_ok() {
                return;
            }
            // The helper is gone: send from here.
            *helper = None;
        }
        drop(helper);
        if (1..=88).contains(&code) {
            Self::send(0, code, up);
        } else if let Some(vk) = vk::vk_for(code) {
            Self::send(vk, 0, up);
        }
    }
}

impl Default for SendInputBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for SendInputBackend {
    fn name(&self) -> &'static str {
        "sendinput"
    }
    fn init(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn press(&mut self, code: u16) {
        Self::key(code, false);
    }
    fn release(&mut self, code: u16) {
        Self::key(code, true);
    }
    fn pump(&mut self) {}
    fn destroy(&mut self) {}
}
