#![windows_subsystem = "windows"]

//! osk-win-input: sends the keys osk-win asks for over its pipe. It carries
//! UIAccess (see build.rs), so Windows lets them reach apps run as
//! administrator; it obeys only the osk-win.exe installed next to it.

use std::fs::OpenOptions;
use std::io::Read;
use std::os::windows::io::AsRawHandle;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

#[allow(dead_code)]
#[path = "../backend.rs"]
mod backend;
#[allow(dead_code)]
#[path = "../vk.rs"]
mod vk;

/// The full path of process `pid`'s executable.
fn exe_of(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len) };
    let _ = unsafe { CloseHandle(process) };
    ok.ok().map(|()| String::from_utf16_lossy(&buf[..len as usize]))
}

fn main() {
    let Some(pid) = std::env::args().nth(1).and_then(|a| a.parse::<u32>().ok()) else {
        return;
    };
    let Ok(mut pipe) = OpenOptions::new().read(true).open(backend::pipe_name(pid)) else {
        return;
    };
    // Anyone could start this program; only the osk-win.exe installed
    // next to it (Program Files: writable by administrators only) may
    // have it type.
    let mut server = 0;
    let _ = unsafe { GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut server) };
    let osk = std::env::current_exe().map(|e| e.with_file_name("osk-win.exe"));
    let trusted = server == pid
        && matches!((exe_of(server), osk), (Some(a), Ok(b)) if a.eq_ignore_ascii_case(&b.to_string_lossy()));
    if !trusted {
        return;
    }
    let mut rec = [0u8; 3];
    while pipe.read_exact(&mut rec).is_ok() {
        backend::SendInputBackend::key(u16::from_le_bytes([rec[0], rec[1]]), rec[2] != 0);
    }
}
