use rusty_companion::AppState;
use std::{
    ptr::{null, null_mut},
    sync::{Arc, OnceLock},
};
use windows_sys::Win32::{
    Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};
static STATE: OnceLock<Arc<AppState>> = OnceLock::new();
const APP_ICON_ID: usize = 101;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn error(text: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(text).as_ptr(),
            wide("RustyPrompter").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
pub fn run(state: Arc<AppState>) {
    STATE.set(state).ok();
    unsafe {
        let instance = GetModuleHandleW(null());
        let class = wide("RustyPrompterCompanion");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hIcon: LoadIconW(instance, APP_ICON_ID as *const u16),
            hbrBackground: (COLOR_WINDOW + 1) as _,
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("RustyPrompter · PC companion").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            370,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            error("Could not create the companion window.");
            return;
        }
        CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide("RustyPrompter\nWrite here. Read on your tablet.").as_ptr(),
            WS_CHILD | WS_VISIBLE | 1,
            25,
            22,
            490,
            52,
            hwnd,
            10 as _,
            instance,
            null(),
        );
        CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide("").as_ptr(),
            WS_CHILD | WS_VISIBLE | 1,
            20,
            93,
            500,
            105,
            hwnd,
            11 as _,
            instance,
            null(),
        );
        CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("Open script studio").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            165,
            212,
            220,
            43,
            hwnd,
            100 as _,
            instance,
            null(),
        );
        CreateWindowExW(0, wide("STATIC").as_ptr(), wide("Keep this window open while syncing.\nAllow Private networks if Windows Firewall asks.").as_ptr(), WS_CHILD | WS_VISIBLE | 1, 20, 273, 500, 45, hwnd, 12 as _, instance, null());
        SendMessageW(hwnd, WM_TIMER, 1, 0);
        SetTimer(hwnd, 1, 1500, None);
        ShowWindow(hwnd, SW_SHOW);
        let mut message = std::mem::zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        UnregisterClassW(class.as_ptr(), instance);
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match message {
        WM_COMMAND if wp & 0xffff == 100 => {
            if let Some(state) = STATE.get() {
                let _ = webbrowser::open(&state.launch_url());
            }
            0
        }
        WM_TIMER => {
            if let Some(state) = STATE.get() {
                let code = state.pairing.lock().unwrap().code.clone();
                let linked = if state.tablets.lock().unwrap().is_empty() {
                    "Waiting for tablet"
                } else {
                    "Tablet connected"
                };
                let text = format!(
                    "{}\nPairing code: {code}\n{linked}",
                    state.addresses().join("\n")
                );
                SetWindowTextW(GetDlgItem(hwnd, 11), wide(&text).as_ptr());
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wp, lp),
    }
}
