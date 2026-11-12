#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

// Logical sizes; must match ui/index.html.
const NOTCH: (f64, f64) = (220.0, 36.0);
const PANEL_W: f64 = 420.0;
const HOTKEY: &str = "ctrl+alt+t";

fn state_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("state.json"))
}

#[tauri::command]
fn load(app: AppHandle) -> Result<String, String> {
    Ok(fs::read_to_string(state_file(&app)?).unwrap_or_default())
}

#[tauri::command]
fn save(app: AppHandle, data: String) -> Result<(), String> {
    // Write-then-rename so a crash mid-write never truncates the list.
    let path = state_file(&app)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

static EXPANDED: AtomicBool = AtomicBool::new(false);
static UP: AtomicBool = AtomicBool::new(false);

/// Resize between notch and panel around the horizontal centre. A notch in the
/// lower half of its monitor grows upward (bottom edge stays put), otherwise
/// downward. The direction is picked while collapsed and kept until the next
/// collapse, so the notch returns to the edge it came from.
/// Returns the new top-left (physical px) and whether it grows upward.
#[tauri::command]
fn set_expanded(win: WebviewWindow, expanded: bool, height: f64) -> Result<(i32, i32, bool), String> {
    let e = |e: tauri::Error| e.to_string();
    let scale = win.scale_factor().map_err(e)?;
    let pos = win.outer_position().map_err(e)?;
    let old = win.outer_size().map_err(e)?;
    let (old_w, old_h) = (old.width as i32, old.height as i32);
    // (left, top, width, height) of the monitor the window is on.
    let mon = win.current_monitor().map_err(e)?.map(|m| {
        (m.position().x, m.position().y, m.size().width as i32, m.size().height as i32)
    });

    if !EXPANDED.load(Relaxed) {
        let up = mon.is_some_and(|(_, top, _, mh)| pos.y + old_h / 2 > top + mh / 2);
        UP.store(up, Relaxed);
    }
    let up = UP.load(Relaxed);

    let (w, h) = if expanded { (PANEL_W, height) } else { NOTCH };
    let (w, h) = ((w * scale).round() as i32, (h * scale).round() as i32);
    let mut x = pos.x + (old_w - w) / 2;
    let mut y = if up { pos.y + old_h - h } else { pos.y };
    if let Some((left, top, mw, mh)) = mon {
        x = x.clamp(left, (left + mw - w).max(left));
        y = y.clamp(top, (top + mh - h).max(top));
    }
    win.set_size(PhysicalSize::new(w, h)).map_err(e)?;
    win.set_position(PhysicalPosition::new(x, y)).map_err(e)?;
    EXPANDED.store(expanded, Relaxed);
    Ok((x, y, up))
}

fn contains(pos: &PhysicalPosition<i32>, size: &PhysicalSize<u32>, x: i32, y: i32) -> bool {
    x >= pos.x && x < pos.x + size.width as i32 && y >= pos.y && y < pos.y + size.height as i32
}

/// Restore a saved position, unless it no longer lands on any monitor.
#[tauri::command]
fn move_to(win: WebviewWindow, x: i32, y: i32) {
    let monitors = win.available_monitors().unwrap_or_default();
    if monitors.iter().any(|m| contains(m.position(), m.size(), x, y)) {
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
}

/// The webview reports mouseleave while the window is being dragged; the UI
/// asks here before collapsing.
#[tauri::command]
fn cursor_inside(win: WebviewWindow) -> bool {
    match (win.cursor_position(), win.outer_position(), win.outer_size()) {
        (Ok(c), Ok(p), Ok(s)) => contains(&p, &s, c.x as i32, c.y as i32),
        _ => false,
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let win = app.get_webview_window("main").expect("main window");
            if let Some(m) = win.primary_monitor()? {
                let w = win.outer_size()?.width as i32;
                let x = m.position().x + (m.size().width as i32 - w) / 2;
                win.set_position(PhysicalPosition::new(x, m.position().y))?;
            }

            app.global_shortcut().on_shortcut(HOTKEY, |app, _, ev| {
                if ev.state == ShortcutState::Pressed {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.set_focus();
                        let _ = w.emit("hotkey", ());
                    }
                }
            })?;

            let themes = SubmenuBuilder::new(app, "樣式")
                .text("theme:obsidian", "黑曜")
                .text("theme:mica", "雲母")
                .text("theme:indigo", "靛夜")
                .build()?;
            let autostart = CheckMenuItemBuilder::with_id("autostart", "開機自動啟動")
                .checked(app.autolaunch().is_enabled().unwrap_or(false))
                .build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&themes)
                .item(&autostart)
                .separator()
                .text("quit", "結束")
                .build()?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("icon").clone())
                .tooltip("Quick To Do (Ctrl+Alt+T)")
                .menu(&menu)
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "quit" => app.exit(0),
                    "autostart" => {
                        let a = app.autolaunch();
                        let _ = if a.is_enabled().unwrap_or(false) { a.disable() } else { a.enable() };
                    }
                    id => {
                        if let Some(theme) = id.strip_prefix("theme:") {
                            let _ = app.emit("theme", theme);
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![load, save, set_expanded, move_to, cursor_inside])
        .run(tauri::generate_context!())
        .expect("error while running Quick To Do");
}
