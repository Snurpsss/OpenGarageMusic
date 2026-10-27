//! The window of the player, a child of the program's, and the web view in it.

use std::borrow::Cow;
use std::num::NonZeroIsize;
use std::path::PathBuf;
use std::sync::mpsc;

use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
use windows::core::w;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow, GetGUIThreadInfo, GetMessageW, GetWindowThreadProcessId, IsChild, PostThreadMessageW, RegisterClassW, SetTimer, SetWindowPos, ShowWindow,
    TranslateMessage, GUITHREADINFO, HWND_TOP, MSG, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE, WINDOW_EX_STYLE, WM_APP, WM_TIMER, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
};
use wry::dpi::{PhysicalPosition, PhysicalSize};
use wry::{Rect, WebContext, WebViewBuilder, WebViewBuilderExtWindows};

const PAGE: &str = include_str!("player.html");

/// An order of the program.
enum Order {
    Bounds(i32, i32, i32, i32),
    Show(bool),
    Script(String),
    Quit,
}

fn order(line: &str) -> Option<Order> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if let Some(b) = value.get("bounds").and_then(|b| b.as_array()) {
        let n = |i: usize| b.get(i).and_then(serde_json::Value::as_f64).unwrap_or(0.0).round() as i32;
        return Some(Order::Bounds(n(0), n(1), n(2).max(1), n(3).max(1)));
    }
    if let Some(show) = value.get("show").and_then(serde_json::Value::as_bool) {
        return Some(Order::Show(show));
    }
    if let Some(js) = value.get("js").and_then(serde_json::Value::as_str) {
        return Some(Order::Script(js.to_owned()));
    }
    value.get("quit").map(|_| Order::Quit)
}

/// The window the web view is drawn in, as wry asks for it.
struct Host(HWND);

impl HasWindowHandle for Host {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let handle = NonZeroIsize::new(self.0 .0 as isize).ok_or(HandleError::Unavailable)?;
        // SAFETY: the window is ours and outlives the web view.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(handle))) })
    }
}

unsafe extern "system" fn procedure(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: the arguments are those Windows gave.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

/// Gives the keyboard back to the program's window when the web view took it:
/// YouTube's page takes it as it loads, or when it is clicked, and a field being
/// typed in, or the space bar, would lose it. The video is played with the
/// mouse; the keyboard stays the program's. It is read for the program's thread
/// and for the window in front (the web view's own process, when the keyboard
/// went there), and given back with this thread joined to them for the time of it.
///
/// SAFETY: plain calls to Windows about windows of this program.
unsafe fn guard_focus(window: HWND, parent: HWND) {
    unsafe {
        let focus_of = |thread: u32| {
            let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
            if GetGUIThreadInfo(thread, &mut info).is_ok() { info.hwndFocus } else { HWND::default() }
        };
        let program = GetWindowThreadProcessId(parent, None);
        let front = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let ours = |focus: HWND| !focus.0.is_null() && (focus == window || IsChild(window, focus).as_bool());
        if ours(focus_of(program)) || ours(focus_of(front)) {
            let us = GetCurrentThreadId();
            let joined: Vec<u32> = [program, front].into_iter().filter(|&t| t != 0 && t != us).filter(|&t| AttachThreadInput(us, t, true).as_bool()).collect();
            let _ = SetFocus(Some(parent));
            for thread in joined {
                let _ = AttachThreadInput(us, thread, false);
            }
        }
    }
}

fn bounds(width: i32, height: i32) -> Rect {
    Rect { position: PhysicalPosition::new(0, 0).into(), size: PhysicalSize::new(width.max(1) as u32, height.max(1) as u32).into() }
}

pub fn run(parent: isize, data: Option<PathBuf>) -> Result<(), String> {
    // SAFETY: plain calls to Windows on this thread; the windows are ours.
    unsafe {
        // Pixels as the program counts them.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let instance = HINSTANCE(module.0);
        let class = WNDCLASSW { lpfnWndProc: Some(procedure), hInstance: instance, lpszClassName: w!("OpenGarageMusicVideo"), ..Default::default() };
        RegisterClassW(&class);
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("OpenGarageMusicVideo"),
            w!(""),
            WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            0,
            0,
            320,
            180,
            Some(HWND(parent as *mut core::ffi::c_void)),
            None,
            Some(instance),
            None,
        )
        .map_err(|e| e.to_string())?;
        let host = Host(window);

        let mut context = WebContext::new(data);
        let webview = WebViewBuilder::new_with_web_context(&mut context)
            // It does not take the keyboard from the program's window: a field
            // being typed in would lose it.
            .with_focused(false)
            .with_https_scheme(true)
            .with_additional_browser_args("--autoplay-policy=no-user-gesture-required")
            .with_custom_protocol("og".to_owned(), |_id, _request| {
                wry::http::Response::builder().header("Content-Type", "text/html; charset=utf-8").body(Cow::Borrowed(PAGE.as_bytes())).unwrap_or_default()
            })
            .with_url("https://og.localhost/player.html")
            .with_ipc_handler(|request| println!("{}", request.body()))
            .with_bounds(bounds(320, 180))
            .build_as_child(&host)
            .map_err(|e| e.to_string())?;

        // The orders come on another thread, which wakes this one for each.
        let thread = GetCurrentThreadId();
        let (send, orders) = mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::stdin().lines() {
                let Ok(line) = line else { break };
                if let Some(order) = order(&line) {
                    if send.send(order).is_err() {
                        return;
                    }
                    let _ = PostThreadMessageW(thread, WM_APP, WPARAM(0), LPARAM(0));
                }
            }
            let _ = send.send(Order::Quit);
            let _ = PostThreadMessageW(thread, WM_APP, WPARAM(0), LPARAM(0));
        });
        // Ten times a second, the keyboard is watched (see `guard_focus`).
        SetTimer(None, 0, 100, None);
        let parent_window = HWND(parent as *mut core::ffi::c_void);
        println!("{}", serde_json::json!({ "started": true }));

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            if message.message == WM_TIMER && message.hwnd.0.is_null() {
                guard_focus(window, parent_window);
                continue;
            }
            if message.message == WM_APP && message.hwnd.0.is_null() {
                for order in orders.try_iter() {
                    match order {
                        Order::Bounds(x, y, width, height) => {
                            let _ = SetWindowPos(window, Some(HWND_TOP), x, y, width, height, SWP_NOACTIVATE);
                            let _ = webview.set_bounds(bounds(width, height));
                        }
                        Order::Show(true) => {
                            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
                        }
                        Order::Show(false) => {
                            let _ = ShowWindow(window, SW_HIDE);
                        }
                        Order::Script(js) => {
                            let _ = webview.evaluate_script(&js);
                        }
                        Order::Quit => return Ok(()),
                    }
                }
                continue;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}
