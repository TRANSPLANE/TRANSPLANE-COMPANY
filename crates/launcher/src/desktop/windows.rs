use super::Session;
use anyhow::{Result, bail};
use std::{
    ptr,
    sync::atomic::{AtomicBool, AtomicIsize, Ordering},
};
use windows_sys::Win32::{
    Foundation::{GlobalFree, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{COLOR_WINDOW, DEFAULT_GUI_FONT, GetStockObject, UpdateWindow},
    System::{
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        LibraryLoader::GetModuleHandleW,
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
    },
    UI::{Shell::ShellExecuteW, WindowsAndMessaging::*},
};

const OPEN: usize = 101;
const COPY: usize = 102;
const STOP: usize = 103;
static WINDOW: AtomicIsize = AtomicIsize::new(0);
static CLOSING: AtomicBool = AtomicBool::new(false);
static ICON: &[u8] = include_bytes!("../../../../docs/media/company-icon.ico");

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

struct WindowState {
    session: Session,
}

pub fn run(session: Session) -> Result<()> {
    let mut state = Box::new(WindowState { session });
    let class_name = wide("TransplaneCompanyServiceWindow");
    let title = wide("TRANSPLANE-COMPANY");
    // All windows, the message loop and the clipboard remain on this dedicated UI thread.
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let icon = brand_icon();
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hIcon: if icon.is_null() {
                LoadIconW(ptr::null_mut(), IDI_APPLICATION)
            } else {
                icon
            },
            hbrBackground: (COLOR_WINDOW + 1) as _,
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&class) == 0 {
            bail!(
                "Cannot create the service window ({})",
                std::io::Error::last_os_error()
            );
        }
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            620,
            330,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            (&mut *state as *mut WindowState).cast(),
        );
        if hwnd.is_null() {
            // Preserve the one-time credential if the native window cannot be constructed.
            if let Some(token) = state.session.owner_token.as_deref() {
                let message = format!("Save your owner token:\n\n{token}\n\n{}", state.session.url);
                MessageBoxW(
                    ptr::null_mut(),
                    wide(&message).as_ptr(),
                    title.as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            bail!(
                "Cannot create the service window ({})",
                std::io::Error::last_os_error()
            );
        }
        populate(hwnd, instance, &state);
        WINDOW.store(hwnd as isize, Ordering::SeqCst);
        if CLOSING.load(Ordering::SeqCst) {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        if let Err(error) = open_browser(&state.session.url) {
            show_error(&format!("{error:#}"), &state.session.lang);
        }
        let mut message = std::mem::zeroed::<MSG>();
        loop {
            let status = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
            if status == 0 {
                break;
            }
            if status < 0 {
                bail!("Cannot read window messages");
            }
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        UnregisterClassW(class_name.as_ptr(), instance);
        if !icon.is_null() {
            DestroyIcon(icon);
        }
    }
    Ok(())
}

unsafe fn brand_icon() -> HICON {
    // An ICO directory entry contains the offset and size of its image resource.
    let Some(directory) = ICON.get(6..22) else {
        return ptr::null_mut();
    };
    let size = u32::from_le_bytes(directory[8..12].try_into().unwrap()) as usize;
    let offset = u32::from_le_bytes(directory[12..16].try_into().unwrap()) as usize;
    let Some(end) = offset.checked_add(size) else {
        return ptr::null_mut();
    };
    let Some(resource) = ICON.get(offset..end) else {
        return ptr::null_mut();
    };
    unsafe {
        CreateIconFromResourceEx(
            resource.as_ptr(),
            size as u32,
            1,
            0x00030000,
            32,
            32,
            LR_DEFAULTCOLOR,
        )
    }
}

unsafe fn populate(hwnd: HWND, instance: HINSTANCE, state: &WindowState) {
    let zh = state.session.lang == "zh-CN";
    let labels = if zh {
        [
            "服务已启动",
            "首次登录凭据：请保存",
            "使用已保存的主人凭据登录",
            "打开经营台",
            "复制凭据",
            "停止服务",
            "关闭此窗口将停止服务。",
        ]
    } else {
        [
            "Service is running",
            "First sign-in token: save it now",
            "Sign in with your saved owner token",
            "Open game",
            "Copy token",
            "Stop service",
            "Closing this window stops the service.",
        ]
    };
    unsafe {
        control(hwnd, instance, "STATIC", labels[0], 20, 18, 570, 24, 0, 0);
        control(
            hwnd,
            instance,
            "EDIT",
            &state.session.url,
            20,
            48,
            565,
            26,
            WS_BORDER | ES_READONLY as u32,
            0,
        );
        control(
            hwnd,
            instance,
            "STATIC",
            if state.session.owner_token.is_some() {
                labels[1]
            } else {
                labels[2]
            },
            20,
            88,
            565,
            22,
            0,
            0,
        );
        if let Some(token) = state.session.owner_token.as_deref() {
            control(
                hwnd,
                instance,
                "EDIT",
                token,
                20,
                116,
                565,
                26,
                WS_BORDER | ES_READONLY as u32,
                0,
            );
            control(
                hwnd, instance, "BUTTON", labels[4], 212, 170, 174, 32, WS_TABSTOP, COPY,
            );
        }
        control(
            hwnd,
            instance,
            "BUTTON",
            labels[3],
            20,
            170,
            174,
            32,
            WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            OPEN,
        );
        control(
            hwnd, instance, "BUTTON", labels[5], 406, 170, 178, 32, WS_TABSTOP, STOP,
        );
        control(
            hwnd,
            instance,
            "STATIC",
            &state.session.data_dir.to_string_lossy(),
            20,
            217,
            565,
            22,
            0,
            0,
        );
        control(hwnd, instance, "STATIC", labels[6], 20, 247, 565, 22, 0, 0);
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn control(
    parent: HWND,
    instance: HINSTANCE,
    class: &str,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    style: u32,
    id: usize,
) {
    unsafe {
        let child = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            width,
            height,
            parent,
            id as _,
            instance,
            ptr::null(),
        );
        SendMessageW(
            child,
            WM_SETFONT,
            GetStockObject(DEFAULT_GUI_FONT) as usize,
            1,
        );
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let creation = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, creation.lpCreateParams as _);
        }
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
        match message {
            WM_COMMAND if !state.is_null() => {
                let state = &*state;
                let action = wparam & 0xffff;
                let result = match action {
                    OPEN => open_browser(&state.session.url),
                    COPY => state
                        .session
                        .owner_token
                        .as_deref()
                        .map(|token| copy_token(hwnd, token))
                        .unwrap_or(Ok(())),
                    STOP => {
                        DestroyWindow(hwnd);
                        return 0;
                    }
                    _ => return DefWindowProcW(hwnd, message, wparam, lparam),
                };
                if let Err(error) = result {
                    show_error(&format!("{error:#}"), &state.session.lang);
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                WINDOW.store(0, Ordering::SeqCst);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

pub fn open_browser(url: &str) -> Result<()> {
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            wide("open").as_ptr(),
            wide(url).as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        bail!("Cannot open {url}. Open it in your browser.");
    }
    Ok(())
}

pub fn show_error(message: &str, lang: &str) {
    let title = if lang == "zh-CN" {
        "无法启动 TRANSPLANE-COMPANY"
    } else {
        "TRANSPLANE-COMPANY"
    };
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            wide(message).as_ptr(),
            wide(title).as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

pub fn close_window() {
    CLOSING.store(true, Ordering::SeqCst);
    let hwnd = WINDOW.load(Ordering::SeqCst) as HWND;
    if !hwnd.is_null() {
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }
}

fn copy_token(hwnd: HWND, token: &str) -> Result<()> {
    unsafe {
        if OpenClipboard(hwnd) == 0 {
            bail!("Cannot open the clipboard. Try again.");
        }
        let result = put_clipboard_text(token);
        CloseClipboard();
        result
    }
}

unsafe fn put_clipboard_text(text: &str) -> Result<()> {
    let value = wide(text);
    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, value.len() * std::mem::size_of::<u16>());
        if handle.is_null() {
            bail!("Cannot allocate clipboard memory");
        }
        let memory = GlobalLock(handle);
        if memory.is_null() {
            GlobalFree(handle);
            bail!("Cannot write to the clipboard");
        }
        ptr::copy_nonoverlapping(value.as_ptr(), memory as *mut u16, value.len());
        GlobalUnlock(handle);
        if EmptyClipboard() == 0 || SetClipboardData(13, handle).is_null() {
            GlobalFree(handle);
            bail!("Cannot write to the clipboard");
        }
        // Windows owns the successful CF_UNICODETEXT allocation from this point.
        Ok(())
    }
}
