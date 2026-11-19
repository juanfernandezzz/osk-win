//! Startup update check: asks GitHub for the latest release and, when it is
//! newer than this build, installs it. Manual checks ask before installation.

use std::fs::{File, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU8, Ordering};

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MB_YESNO,
    SW_SHOWNORMAL,
};

const REPO: &str = "juanfernandezzz/osk-win";
const INSTALLER: &str = "osk-win-setup.exe";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");
const IDLE: u8 = 0;
const BACKGROUND: u8 = 1;
const REQUESTED: u8 = 2;
const INSTALLING: u8 = 3;
const INSTALLING_REQUESTED: u8 = 4;
static CHECKING: AtomicU8 = AtomicU8::new(IDLE);

struct CheckGuard<'a>(Option<&'a AtomicU8>);

impl<'a> CheckGuard<'a> {
    fn acquire(flag: &'a AtomicU8, manual: bool) -> Option<Self> {
        let request = if manual { REQUESTED } else { BACKGROUND };
        (flag.fetch_max(request, Ordering::AcqRel) == IDLE).then(|| Self(Some(flag)))
    }

    // A click either promotes this check or starts a new one after its
    // silent completion. Never clear a newer worker's state on drop.
    fn should_inform(&mut self) -> bool {
        let flag = self.0.expect("check still active");
        let state = flag.load(Ordering::Acquire);
        if matches!(state, BACKGROUND | INSTALLING)
            && flag
                .compare_exchange(state, IDLE, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            self.0 = None;
            false
        } else {
            true
        }
    }

    fn manual(&self) -> bool {
        self.0.expect("active check").load(Ordering::Acquire) == REQUESTED
    }
}

impl Drop for CheckGuard<'_> {
    fn drop(&mut self) {
        if let Some(flag) = self.0 {
            flag.store(IDLE, Ordering::Release);
        }
    }
}

fn inform(text: &str) {
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            &HSTRING::from("osk-win"),
            MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND | MB_TOPMOST,
        );
    }
}

#[cfg(not(test))]
pub(crate) fn check_manually() {
    if let Some(guard) = CheckGuard::acquire(&CHECKING, true) {
        std::thread::spawn(move || run_check(guard));
    }
}

#[cfg(test)]
thread_local! {
    pub(crate) static MANUAL_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn check_manually() {
    MANUAL_CHECKS.with(|count| count.set(count.get() + 1));
}

fn run_check(mut guard: CheckGuard<'_>) {
    if let Err(e) = check(&mut guard) {
        warn!("osk: update check failed: {e}");
        if guard.should_inform() {
            inform(&format!("No se pudo completar la actualización.\n\n{e}"));
        }
    }
}

/// Check in the background, so a slow or missing network never delays the
/// keyboard; a failed check only logs. It waits first: starting curl while
/// the window is still being drawn made the keyboard appear 5x slower.
pub fn check_at_startup() {
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        if let Some(guard) = CheckGuard::acquire(&CHECKING, false) {
            run_check(guard);
        }
    });
}

/// `curl.exe` (standard on Windows 10/11) with `args`; its stdout.
fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new(system_tool("curl.exe")?)
        .args(["-sL", "--fail"])
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(format!("curl {}", out.status))
    }
}

fn system_tool(name: &str) -> Result<PathBuf, String> {
    std::env::var_os("SystemRoot")
        .map(|dir| PathBuf::from(dir).join("System32").join(name))
        .ok_or_else(|| "Falta SystemRoot".to_owned())
}

/// Whether version `a` ("0.3.0") is newer than `b`.
fn is_newer(a: &str, b: &str) -> bool {
    let nums = |s: &str| {
        s.split('.')
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    nums(a) > nums(b)
}

#[derive(Debug, PartialEq)]
struct Update {
    version: String,
    url: String,
    sha256: String,
}

fn available_update(release: &serde_json::Value, current: &str) -> Result<Option<Update>, String> {
    let release_tag = release["tag_name"].as_str().ok_or("no tag_name")?;
    let tag = release_tag.trim_start_matches('v');
    if !is_newer(tag, current) {
        return Ok(None);
    }
    let asset = release["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"] == INSTALLER)
        .ok_or("no installer in the release")?;
    let url = format!("https://github.com/{REPO}/releases/download/{release_tag}/{INSTALLER}");
    if asset["browser_download_url"].as_str() != Some(&url) {
        return Err("URL de instalador inesperada".into());
    }
    let hash = asset["digest"]
        .as_str()
        .and_then(|d| d.strip_prefix("sha256:"))
        .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("Falta el SHA-256 del instalador")?;
    Ok(Some(Update {
        version: tag.to_owned(),
        url,
        sha256: hash.to_owned(),
    }))
}

fn check(guard: &mut CheckGuard<'_>) -> Result<(), String> {
    let api = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let release: serde_json::Value =
        serde_json::from_slice(&curl(&["-m", "15", &api])?).map_err(|e| e.to_string())?;
    let current = VERSION;
    let Some(update) = available_update(&release, current)? else {
        if guard.should_inform() {
            inform(&format!(
                "osk-win {current} está actualizado. No hay una versión más reciente disponible."
            ));
        }
        return Ok(());
    };
    apply(
        guard,
        || confirm(&update.version, current),
        || download(&update),
        |(path, _lock)| install(&path),
    )
}

fn apply<T>(
    guard: &CheckGuard<'_>,
    mut confirm: impl FnMut() -> bool,
    download: impl FnOnce() -> Result<T, String>,
    install: impl FnOnce(T) -> Result<(), String>,
) -> Result<(), String> {
    let asked = guard.manual();
    if asked && !confirm() {
        return Ok(());
    }
    let setup = download()?;
    // A manual click during the background download still gets a decision.
    let flag = guard.0.expect("active check");
    if flag
        .compare_exchange(BACKGROUND, INSTALLING, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        flag.store(INSTALLING_REQUESTED, Ordering::Release);
        if !asked && !confirm() {
            return Ok(());
        }
    }
    install(setup)
}

fn confirm(tag: &str, current: &str) -> bool {
    let text = format!(
        "Hay una versión nueva de osk-win ({tag}); tienes la {current}.\n\n\
         ¿Descargarla e instalarla ahora? El teclado se cierra durante la instalación."
    );
    let answer = unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(text),
            &HSTRING::from("osk-win"),
            MB_YESNO | MB_ICONINFORMATION | MB_TOPMOST | MB_SETFOREGROUND,
        )
    };
    answer == IDYES
}

fn download(update: &Update) -> Result<(PathBuf, File), String> {
    let dir = PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("Falta LOCALAPPDATA")?)
        .join("osk-win")
        .join("updates");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let setup = dir.join(INSTALLER);
    curl(&["-m", "600", "-o", &setup.to_string_lossy(), &update.url])?;
    let lock = verify_installer(&setup, &update.sha256)?;
    Ok((setup, lock))
}

fn verify_installer(setup: &Path, expected_sha256: &str) -> Result<File, String> {
    let lock = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(setup)
        .map_err(|e| e.to_string())?;
    let hash = Command::new(system_tool("certutil.exe")?)
        .arg("-hashfile")
        .arg(setup)
        .arg("SHA256")
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if !hash.status.success()
        || !String::from_utf8_lossy(&hash.stdout)
            .lines()
            .any(|line| line.trim().eq_ignore_ascii_case(expected_sha256))
    {
        return Err("El SHA-256 del instalador descargado no coincide".into());
    }
    Ok(lock)
}

fn install(path: &Path) -> Result<(), String> {
    let file = HSTRING::from(path.as_os_str());
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("open"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: w!("/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /OSKUPDATE=1"),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info).map_err(|e| e.to_string())?;
        let mut code = 0;
        let result = if WaitForSingleObject(info.hProcess, INFINITE) == WAIT_OBJECT_0 {
            GetExitCodeProcess(info.hProcess, &mut code).map_err(|e| e.to_string())
        } else {
            Err(windows::core::Error::from_thread().to_string())
        };
        let _ = CloseHandle(info.hProcess);
        result?;
        if code != 0 {
            return Err(format!("El instalador terminó con código {code}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_and_older_releases_need_no_installer() {
        for tag in ["v0.3.0", "0.3.0", "v0.2.9"] {
            assert_eq!(
                available_update(&serde_json::json!({"tag_name":tag}), "0.3.0"),
                Ok(None)
            );
        }
    }

    #[test]
    fn newer_release_selects_the_existing_windows_installer() {
        let url = format!("https://github.com/{REPO}/releases/download/v0.4.0/{INSTALLER}");
        let release = serde_json::json!({"tag_name":"v0.4.0", "assets":[
            {"name":"source.zip", "browser_download_url":"unrelated"},
            {"name":INSTALLER, "browser_download_url":url, "digest":format!("sha256:{}", "ab".repeat(32))}
        ]});
        assert_eq!(
            available_update(&release, "0.3.0"),
            Ok(Some(Update {
                version: "0.4.0".into(),
                url,
                sha256: "ab".repeat(32)
            }))
        );
        let mut missing_hash = release.clone();
        missing_hash["assets"][1]["digest"] = serde_json::Value::Null;
        assert!(available_update(&missing_hash, "0.3.0").is_err());
    }

    #[test]
    fn startup_installs_and_manual_checks_respect_the_answer() {
        use std::cell::Cell;
        for (manual, yes, late_click, download_ok) in [
            (false, false, false, true),
            (true, true, false, true),
            (true, false, false, true),
            (false, false, true, true),
            (false, true, true, true),
            (false, false, false, false),
        ] {
            let state = AtomicU8::new(IDLE);
            let guard = CheckGuard::acquire(&state, manual).unwrap();
            let (asked, downloaded, installed) = (Cell::new(0), Cell::new(false), Cell::new(false));
            let result = apply(
                &guard,
                || {
                    asked.set(asked.get() + 1);
                    yes
                },
                || {
                    downloaded.set(true);
                    if late_click {
                        assert!(CheckGuard::acquire(&state, true).is_none());
                    }
                    if download_ok {
                        Ok(42)
                    } else {
                        Err("download".into())
                    }
                },
                |value| {
                    assert_eq!(value, 42);
                    installed.set(true);
                    Ok(())
                },
            );
            assert_eq!(asked.get(), u8::from(manual || late_click));
            assert_eq!(downloaded.get(), !manual || yes);
            assert_eq!(
                installed.get(),
                download_ok && (!(manual || late_click) || yes)
            );
            assert_eq!(result.is_ok(), download_ok);
        }
    }

    #[test]
    fn incomplete_release_is_an_error_not_up_to_date() {
        for release in [
            serde_json::json!({}),
            serde_json::json!({"tag_name":"v0.4.0"}),
        ] {
            assert!(available_update(&release, "0.3.0").is_err());
        }
    }

    #[test]
    fn downloaded_bytes_are_verified_and_locked_through_launch() {
        let file = std::env::temp_dir().join(format!("osk-digest-test-{}.bin", std::process::id()));
        let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        std::fs::write(&file, b"abc").unwrap();
        let lock = verify_installer(&file, hash).unwrap();
        assert!(std::fs::write(&file, b"bad").is_err());
        drop(lock);
        std::fs::write(&file, b"bad").unwrap();
        assert!(verify_installer(&file, hash).is_err());
        std::fs::remove_file(file).unwrap();
    }

    #[test]
    fn only_one_update_check_runs_and_completion_allows_retry() {
        let checking = AtomicU8::new(IDLE);
        let mut first = CheckGuard::acquire(&checking, false).unwrap();
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| assert!(CheckGuard::acquire(&checking, true).is_none()));
            }
        });
        assert!(
            first.should_inform(),
            "manual click gets the startup check's result"
        );
        drop(first);
        let mut quiet = CheckGuard::acquire(&checking, false).unwrap();
        assert!(!quiet.should_inform());
        let mut next = CheckGuard::acquire(&checking, true).unwrap();
        drop(quiet);
        assert!(CheckGuard::acquire(&checking, false).is_none());
        assert!(next.should_inform());
    }

    #[test]
    fn versions_compare_by_number_not_text() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("1.0.0", "0.99.9"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("garbage", "0.2.0"));
    }
}
