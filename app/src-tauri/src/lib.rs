use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static LOCKED: AtomicBool = AtomicBool::new(false);

pub fn log(line: &str) {
    let dir = std::env::var("TEMP").unwrap_or_else(|_| "/tmp".into());
    let path = std::path::Path::new(&dir).join("s0-probe.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "{} {}", ts, line);
    }
}

#[tauri::command]
fn native_print(window: tauri::WebviewWindow) -> Result<(), String> {
    window.print().map_err(|e| e.to_string())
}

#[tauri::command]
fn report(text: String) {
    log(&text);
}

#[tauri::command]
fn set_locked(v: bool) {
    LOCKED.store(v, Ordering::SeqCst);
}

#[cfg(all(windows, feature = "winlock"))]
mod winlock {
    use super::log;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::RemoteDesktop::WTSRegisterSessionNotification;
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};

    const WM_POWERBROADCAST: u32 = 0x0218;
    const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
    const PBT_APMSUSPEND: usize = 4;
    const PBT_APMRESUMEAUTOMATIC: usize = 0x12;
    const WTS_SESSION_LOCK: usize = 7;
    const WTS_SESSION_UNLOCK: usize = 8;

    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
        if msg == WM_POWERBROADCAST {
            match wparam.0 {
                PBT_APMSUSPEND => log("WM_POWERBROADCAST suspend"),
                PBT_APMRESUMEAUTOMATIC => log("WM_POWERBROADCAST resume"),
                _ => {}
            }
        } else if msg == WM_WTSSESSION_CHANGE {
            match wparam.0 {
                WTS_SESSION_LOCK => log("WTS_SESSION_LOCK"),
                WTS_SESSION_UNLOCK => log("WTS_SESSION_UNLOCK"),
                _ => {}
            }
        }
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }

    pub fn install(raw: isize) {
        let hwnd = HWND(raw as _);
        unsafe {
            let ok = SetWindowSubclass(hwnd, Some(proc), 1, 0);
            log(&format!("subclass installed {}", ok.as_bool()));
            let reg = WTSRegisterSessionNotification(hwnd, 0);
            log(&format!("wts registered {}", reg.is_ok()));
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    log(&format!("started args={:?}", std::env::args().collect::<Vec<_>>()));
    tauri::Builder::default()
        .register_uri_scheme_protocol("tacfile", |_ctx, _req| {
            tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", "text/plain")
                .header("Access-Control-Allow-Origin", "*")
                .body(b"tacfile ok".to_vec())
                .unwrap()
        })
        .setup(|_app| {
            #[cfg(all(windows, feature = "winlock"))]
            {
                use tauri::Manager;
                if let Some(w) = _app.get_webview_window("main") {
                    if let Ok(h) = w.hwnd() {
                        winlock::install(h.0 as isize);
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![native_print, report, set_locked])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
