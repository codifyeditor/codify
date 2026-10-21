#![windows_subsystem = "windows"]

use std::{ffi::OsStr, mem, os::windows::ffi::OsStrExt, path::{Path, PathBuf}, process::Command, ptr, sync::OnceLock};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{CreateFontW, CreateSolidBrush, SetBkColor, SetTextColor, COLORREF, HBRUSH, HFONT},
    System::{Com::CoTaskMemFree, LibraryLoader::GetModuleHandleW},
    UI::{
        Controls::Dialogs::{GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OFN_OVERWRITEPROMPT},
        Shell::{BROWSEINFOW, SHBrowseForFolderW, SHGetPathFromIDListW},
        WindowsAndMessaging::{
            AppendMenuW, CreateMenu, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect,
            GetMessageW, GetWindowTextLengthW, GetWindowTextW, LoadCursorW, MessageBoxW, PostQuitMessage,
            RegisterClassW, SendMessageW, SetWindowTextW, ShowWindow, TranslateMessage, UpdateWindow,
            ACCEL, CreateAcceleratorTableW, TranslateAcceleratorW, FVIRTKEY, FCONTROL,
            WM_COMMAND, WM_CREATE, WM_DESTROY, WM_SIZE, WM_KEYDOWN, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX,
            WNDCLASSW, MSG, CW_USEDEFAULT, CS_HREDRAW, CS_VREDRAW, IDC_ARROW, MB_ICONERROR, MB_OK,
            MF_POPUP, MF_STRING, SW_SHOW, WS_CHILD, WS_VISIBLE, WS_OVERLAPPEDWINDOW, WS_VSCROLL,
            WS_HSCROLL, WS_BORDER, ES_MULTILINE, ES_AUTOVSCROLL, ES_AUTOHSCROLL, ES_WANTRETURN,
            LBS_NOTIFY, LBS_NOINTEGRALHEIGHT, LB_ERR, LB_ADDSTRING, LB_RESETCONTENT, LB_GETCURSEL,
            WM_SETFONT, EM_SETLIMITTEXT, GetDlgItem, GetKeyState, VK_CONTROL, VK_F5,
        },
    },
};

const IDC_FILES: i32 = 101;
const IDC_EDITOR: i32 = 102;
const IDC_OUTPUT: i32 = 103;
const IDC_STATUS: i32 = 104;
const IDM_OPEN: usize = 40001;
const IDM_OPEN_FOLDER: usize = 40002;
const IDM_SAVE: usize = 40003;
const IDM_SAVE_AS: usize = 40004;
const IDM_RUN: usize = 40005;
const IDM_EXTENSIONS: usize = 40006;
const IDM_EXIT: usize = 40007;
const COLOR_BG: COLORREF = 0x00251E1B; // COLORREF is 0x00BBGGRR
const COLOR_TEXT: COLORREF = 0x00E8E4DF;

#[derive(Default)]
struct AppState {
    workspace: Option<PathBuf>,
    current_file: Option<PathBuf>,
    files: Vec<PathBuf>,
}
static mut STATE: Option<AppState> = None;
static mut DARK_BRUSH: HBRUSH = 0 as HBRUSH;
static mut FONT: HFONT = 0 as HFONT;
static INSTANCE: OnceLock<isize> = OnceLock::new();

fn wide(s: &str) -> Vec<u16> { OsStr::new(s).encode_wide().chain(Some(0)).collect() }
fn path_wide(path: &Path) -> Vec<u16> { path.as_os_str().encode_wide().chain(Some(0)).collect() }
fn hwnd_id(id: i32) -> HWND { unsafe { GetDlgItem(MAIN_HWND, id) } }
static mut MAIN_HWND: HWND = ptr::null_mut();

unsafe fn set_status(text: &str) { let w = wide(text); SetWindowTextW(hwnd_id(IDC_STATUS), w.as_ptr()); }
unsafe fn set_output(text: &str) { let w = wide(text); SetWindowTextW(hwnd_id(IDC_OUTPUT), w.as_ptr()); }
unsafe fn editor_text() -> String {
    let edit = hwnd_id(IDC_EDITOR);
    let len = GetWindowTextLengthW(edit).max(0) as usize;
    let mut buf = vec![0u16; len + 1];
    GetWindowTextW(edit, buf.as_mut_ptr(), buf.len() as i32);
    String::from_utf16_lossy(&buf[..len])
}
unsafe fn show_error(message: &str) {
    let body = wide(message); let title = wide("Codify");
    MessageBoxW(MAIN_HWND, body.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
}

unsafe fn make_menu() -> isize {
    let menu = CreateMenu();
    let file = CreateMenu();
    let run = CreateMenu();
    let view = CreateMenu();
    let items = [
        (IDM_OPEN, "Open File\tCtrl+O"), (IDM_OPEN_FOLDER, "Open Folder..."),
        (IDM_SAVE, "Save\tCtrl+S"), (IDM_SAVE_AS, "Save As..."), (IDM_EXIT, "Exit"),
    ];
    for (id, label) in items { let w = wide(label); AppendMenuW(file, MF_STRING, id, w.as_ptr()); }
    let r = wide("Run Active File\tF5"); AppendMenuW(run, MF_STRING, IDM_RUN, r.as_ptr());
    let e = wide("Browse Extensions on Open VSX"); AppendMenuW(view, MF_STRING, IDM_EXTENSIONS, e.as_ptr());
    let f = wide("File"); let rr = wide("Run"); let v = wide("Extensions");
    AppendMenuW(menu, MF_POPUP, file as usize, f.as_ptr());
    AppendMenuW(menu, MF_POPUP, run as usize, rr.as_ptr());
    AppendMenuW(menu, MF_POPUP, view as usize, v.as_ptr());
    menu
}

unsafe fn create_control(class: &str, text: &str, style: u32, id: i32) -> HWND {
    let c = wide(class); let t = wide(text);
    CreateWindowExW(0, c.as_ptr(), t.as_ptr(), WS_CHILD | WS_VISIBLE | style,
        0, 0, 100, 100, MAIN_HWND, id as isize as _, *INSTANCE.get().unwrap() as _, ptr::null())
}

unsafe fn layout() {
    let mut rc = mem::zeroed(); GetClientRect(MAIN_HWND, &mut rc);
    let w = rc.right - rc.left; let h = rc.bottom - rc.top;
    let side = 215; let status_h = 24; let output_h = (h / 4).clamp(90, 190);
    let editor_h = (h - status_h - output_h).max(80);
    windows_sys::Win32::UI::WindowsAndMessaging::MoveWindow(hwnd_id(IDC_FILES), 0, 0, side, editor_h, 1);
    windows_sys::Win32::UI::WindowsAndMessaging::MoveWindow(hwnd_id(IDC_EDITOR), side, 0, (w-side).max(100), editor_h, 1);
    windows_sys::Win32::UI::WindowsAndMessaging::MoveWindow(hwnd_id(IDC_OUTPUT), 0, editor_h, w, output_h, 1);
    windows_sys::Win32::UI::WindowsAndMessaging::MoveWindow(hwnd_id(IDC_STATUS), 0, editor_h+output_h, w, status_h, 1);
}

unsafe fn refresh_files() {
    let Some(state) = STATE.as_mut() else { return };
    let Some(root) = state.workspace.clone() else { return };
    state.files.clear();
    let list = hwnd_id(IDC_FILES); SendMessageW(list, LB_RESETCONTENT, 0, 0);
    if let Ok(entries) = std::fs::read_dir(&root) {
        let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort_by_key(|p| (!p.is_dir(), p.file_name().unwrap_or_default().to_string_lossy().to_lowercase()));
        for path in paths.into_iter().take(5000) {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let display = if path.is_dir() { format!("▸ {name}") } else { name };
            let w = wide(&display); SendMessageW(list, LB_ADDSTRING, 0, w.as_ptr() as LPARAM);
            state.files.push(path);
        }
    }
}

unsafe fn open_file_path(path: PathBuf) {
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let w = wide(&contents); SetWindowTextW(hwnd_id(IDC_EDITOR), w.as_ptr());
            let parent = path.parent().map(Path::to_path_buf);
            if let Some(state) = STATE.as_mut() { state.current_file = Some(path.clone()); state.workspace = parent; }
            refresh_files();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("plain text");
            set_status(&format!("  {}    |    {}    |    Native lightweight mode", path.display(), ext));
            set_output("Codify ready. Select Run → Run Active File (F5) to execute using an installed runtime.");
        }
        Err(e) => show_error(&format!("Couldn't open file:\n{e}")),
    }
}

unsafe fn choose_open_file() {
    let mut file = vec![0u16; 32768];
    let filter = wide("All files\0*.*\0Source files\0*.rs;*.py;*.js;*.ts;*.c;*.cpp;*.h;*.java;*.go;*.rb;*.php;*.html;*.css;*.json;*.ce;*.ceasy\0\0");
    let mut ofn: OPENFILENAMEW = mem::zeroed();
    ofn.lStructSize = mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = MAIN_HWND; ofn.lpstrFilter = filter.as_ptr(); ofn.lpstrFile = file.as_mut_ptr();
    ofn.nMaxFile = file.len() as u32; ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST;
    if GetOpenFileNameW(&mut ofn) != 0 { open_file_path(PathBuf::from(String::from_utf16_lossy(&file[..file.iter().position(|&c| c == 0).unwrap_or(file.len())]))); }
}

unsafe fn choose_folder() {
    let title = wide("Choose a project folder");
    let mut bi: BROWSEINFOW = mem::zeroed();
    bi.hwndOwner = MAIN_HWND; bi.lpszTitle = title.as_ptr(); bi.ulFlags = 0x0001 | 0x0040;
    let pidl = SHBrowseForFolderW(&mut bi);
    if pidl.is_null() { return; }
    let mut buffer = vec![0u16; 32768];
    let ok = SHGetPathFromIDListW(pidl, buffer.as_mut_ptr()); CoTaskMemFree(pidl as _);
    if ok != 0 {
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        let folder = PathBuf::from(String::from_utf16_lossy(&buffer[..len]));
        if let Some(state) = STATE.as_mut() { state.workspace = Some(folder.clone()); state.current_file = None; }
        SetWindowTextW(hwnd_id(IDC_EDITOR), wide("// Welcome to Codify\r\n// Lightweight native editor\r\n// Open a file from the Explorer panel or use File > Open File.\r\n").as_ptr());
        refresh_files(); set_status(&format!("  {}    |    workspace", folder.display()));
    }
}

unsafe fn save_file(save_as: bool) -> bool {
    let current = STATE.as_ref().and_then(|s| s.current_file.clone());
    let path = if !save_as { current } else { None };
    let path = match path {
        Some(p) => p,
        None => {
            let mut file = vec![0u16; 32768];
            if let Some(p) = STATE.as_ref().and_then(|s| s.current_file.as_ref()) {
                let wide_path = path_wide(p); let n = wide_path.len().min(file.len()); file[..n].copy_from_slice(&wide_path[..n]);
            }
            let filter = wide("All files\0*.*\0\0"); let mut ofn: OPENFILENAMEW = mem::zeroed();
            ofn.lStructSize = mem::size_of::<OPENFILENAMEW>() as u32; ofn.hwndOwner = MAIN_HWND;
            ofn.lpstrFilter = filter.as_ptr(); ofn.lpstrFile = file.as_mut_ptr(); ofn.nMaxFile = file.len() as u32;
            ofn.Flags = OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT;
            if GetSaveFileNameW(&mut ofn) == 0 { return false; }
            let len = file.iter().position(|&c| c == 0).unwrap_or(file.len()); PathBuf::from(String::from_utf16_lossy(&file[..len]))
        }
    };
    match std::fs::write(&path, editor_text()) {
        Ok(()) => {
            if let Some(state) = STATE.as_mut() { state.current_file = Some(path.clone()); state.workspace = path.parent().map(Path::to_path_buf); }
            refresh_files(); set_status(&format!("  {}    |    saved", path.display())); true
        }
        Err(e) => { show_error(&format!("Couldn't save file:\n{e}")); false }
    }
}

fn command_for(path: &Path) -> Option<Command> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    let p = path.to_string_lossy().to_string();
    let mut c = match ext.as_str() {
        "py" => { let mut c = Command::new("python"); c.arg(&p); c },
        "js" | "mjs" | "cjs" => { let mut c = Command::new("node"); c.arg(&p); c },
        "ts" | "tsx" => { let mut c = Command::new("npx"); c.args(["tsx", &p]); c },
        "rs" => { let out = path.with_extension("codify-run.exe"); let script = format!("rustc \"{}\" -o \"{}\" && \"{}\"", p, out.display(), out.display()); let mut c = Command::new("cmd"); c.args(["/C", script.as_str()]); c },
        "c" => { let out = path.with_extension("codify-run.exe"); let script = format!("gcc \"{}\" -o \"{}\" && \"{}\"", p, out.display(), out.display()); let mut c = Command::new("cmd"); c.args(["/C", script.as_str()]); c },
        "cpp" | "cc" | "cxx" => { let out = path.with_extension("codify-run.exe"); let script = format!("g++ \"{}\" -o \"{}\" && \"{}\"", p, out.display(), out.display()); let mut c = Command::new("cmd"); c.args(["/C", script.as_str()]); c },
        "java" => { let mut c = Command::new("java"); c.arg(&p); c },
        "go" => { let mut c = Command::new("go"); c.args(["run", &p]); c },
        "rb" => { let mut c = Command::new("ruby"); c.arg(&p); c },
        "php" => { let mut c = Command::new("php"); c.arg(&p); c },
        "ps1" => { let mut c = Command::new("powershell"); c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &p]); c },
        "bat" | "cmd" => { let mut c = Command::new("cmd"); c.args(["/C", &p]); c },
        "sh" => { let mut c = Command::new("bash"); c.arg(&p); c },
        "ce" | "ceasy" => { let mut c = Command::new("ceasy"); c.arg(&p); c },
        _ => return None,
    };
    if let Some(parent) = path.parent() { c.current_dir(parent); }
    Some(c)
}

unsafe fn run_active() {
    if STATE.as_ref().and_then(|s| s.current_file.as_ref()).is_none() && !save_file(true) { return; }
    if !save_file(false) { return; }
    let Some(path) = STATE.as_ref().and_then(|s| s.current_file.clone()) else { return; };
    let Some(mut cmd) = command_for(&path) else {
        set_output("No run command is configured for this file type. Install the language runtime/compiler and add a runner in src/main.rs."); return;
    };
    set_output(&format!("Running {} ...", path.file_name().unwrap_or_default().to_string_lossy()));
    match cmd.output() {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            text.push_str(&format!("\r\n[process exited with {}]", out.status));
            if text.trim().is_empty() { text = "Process finished with no output.".into(); }
            set_output(&text);
        }
        Err(e) => set_output(&format!("Couldn't start the runtime: {e}\r\nInstall it and ensure its command is available on PATH.")),
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            MAIN_HWND = hwnd; STATE = Some(AppState::default());
            DARK_BRUSH = CreateSolidBrush(COLOR_BG); FONT = CreateFontW(17, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, wide("Consolas").as_ptr());
            let _ = create_control("LISTBOX", "", LBS_NOTIFY | LBS_NOINTEGRALHEIGHT | WS_VSCROLL | WS_BORDER, IDC_FILES);
            let editor = create_control("EDIT", "// Welcome to Codify\r\n// A lightweight native code editor\r\n// File > Open Folder to browse a project.\r\n", ES_MULTILINE | ES_AUTOVSCROLL | ES_AUTOHSCROLL | ES_WANTRETURN | WS_VSCROLL | WS_HSCROLL | WS_BORDER, IDC_EDITOR);
            let output = create_control("EDIT", "Output will appear here when you run a file.", ES_MULTILINE | ES_AUTOVSCROLL | ES_AUTOHSCROLL | WS_VSCROLL | WS_HSCROLL | WS_BORDER, IDC_OUTPUT);
            let status = create_control("STATIC", "  Ready    |    Rust native UI    |    low-memory design target", 0, IDC_STATUS);
            for id in [IDC_FILES, IDC_EDITOR, IDC_OUTPUT, IDC_STATUS] { SendMessageW(hwnd_id(id), WM_SETFONT, FONT as usize, 1); }
            SendMessageW(editor, EM_SETLIMITTEXT, 0x7ffffffe, 0); SendMessageW(output, EM_SETLIMITTEXT, 0x7ffffffe, 0);
            let _ = status; layout(); 0
        }
        WM_SIZE => { layout(); 0 }
        WM_KEYDOWN => {
            let key = wparam as u32;
            let ctrl = GetKeyState(VK_CONTROL as i32) < 0;
            if key == VK_F5 { run_active(); return 0; }
            if ctrl && key == b'O' as u32 { choose_open_file(); return 0; }
            if ctrl && key == b'S' as u32 { save_file(false); return 0; }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_COMMAND => {
            let id = (wparam & 0xffff) as usize;
            match id {
                IDM_OPEN => choose_open_file(), IDM_OPEN_FOLDER => choose_folder(),
                IDM_SAVE => { save_file(false); }, IDM_SAVE_AS => { save_file(true); },
                IDM_RUN => run_active(), IDM_EXTENSIONS => {
                    let url = wide("https://open-vsx.org/");
                    let open = wide("open"); let target = wide("https://open-vsx.org/");
                    windows_sys::Win32::UI::Shell::ShellExecuteW(hwnd, open.as_ptr(), target.as_ptr(), ptr::null(), ptr::null(), 1);
                    let _ = url;
                },
                IDM_EXIT => windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd),
                x if x == IDC_FILES as usize => {
                    let list = hwnd_id(IDC_FILES); let selected = SendMessageW(list, LB_GETCURSEL, 0, 0);
                    if selected != LB_ERR as isize {
                        if let Some(path) = STATE.as_ref().and_then(|s| s.files.get(selected as usize)).cloned() {
                            if path.is_dir() { if let Some(state) = STATE.as_mut() { state.workspace = Some(path); } refresh_files(); }
                            else { open_file_path(path); }
                        }
                    }
                }
                _ => {}
            }
            0
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let dc = wparam as windows_sys::Win32::Graphics::Gdi::HDC;
            SetTextColor(dc, COLOR_TEXT); SetBkColor(dc, COLOR_BG); DARK_BRUSH as LRESULT
        }
        WM_DESTROY => { PostQuitMessage(0); 0 }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    unsafe {
        let instance = GetModuleHandleW(ptr::null()); let _ = INSTANCE.set(instance as isize);
        let class_name = wide("CodifyNativeWindow");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW, lpfnWndProc: Some(window_proc), cbClsExtra: 0, cbWndExtra: 0,
            hInstance: instance, hIcon: ptr::null_mut(), hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: CreateSolidBrush(COLOR_BG), lpszMenuName: ptr::null(), lpszClassName: class_name.as_ptr(),
        };
        if RegisterClassW(&wc) == 0 { return; }
        let title = wide("Codify — Lightweight Code Editor");
        let hwnd = CreateWindowExW(0, class_name.as_ptr(), title.as_ptr(), WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT, CW_USEDEFAULT, 1200, 780, ptr::null_mut(), make_menu(), instance, ptr::null());
        if hwnd.is_null() { return; }
        ShowWindow(hwnd, SW_SHOW); UpdateWindow(hwnd);
        let accelerators = [
            ACCEL { fVirt: (FVIRTKEY | FCONTROL) as u8, key: b'O' as u16, cmd: IDM_OPEN as u16 },
            ACCEL { fVirt: (FVIRTKEY | FCONTROL) as u8, key: b'S' as u16, cmd: IDM_SAVE as u16 },
            ACCEL { fVirt: FVIRTKEY as u8, key: VK_F5 as u16, cmd: IDM_RUN as u16 },
        ];
        let accel_table = CreateAcceleratorTableW(accelerators.as_ptr(), accelerators.len() as i32);
        let mut msg: MSG = mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            if TranslateAcceleratorW(hwnd, accel_table, &msg) == 0 { TranslateMessage(&msg); DispatchMessageW(&msg); }
        }
    }
}
