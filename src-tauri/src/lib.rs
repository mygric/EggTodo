mod commands;
mod data_exchange;
mod db;
mod error_codes;
mod i18n;
mod note_asset_store;
mod note_attachment_sync;
mod note_attachments;
mod note_sync;
mod notes;
mod panel_position;
mod reminders;
mod s3_sync;
mod schedule;
mod sync;
mod tray;
#[cfg(target_os = "linux")]
mod tray_ksni;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, WindowEvent};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Persisted main window state (size + position). Saved to app_data_dir
/// so it survives app restarts. More reliable than frontend localStorage
/// because the async Tauri APIs sometimes don't finish before unload.
#[derive(Clone, Serialize, Deserialize)]
struct WindowState {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
}

fn window_state_path(app: &tauri::AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap().join("window_state.json")
}

fn load_window_state(app: &tauri::AppHandle) -> Option<WindowState> {
    let path = window_state_path(app);
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn save_window_state(app: &tauri::AppHandle, state: &WindowState) {
    let path = window_state_path(app);
    if let Ok(content) = serde_json::to_string_pretty(state) {
        let _ = fs::write(path, content);
    }
}

/// Debounced save: coalesces rapid Resized/Moved events into one save
/// 1 second after the last event. Uses an AtomicBool flag so only one
/// pending save thread exists at a time.
static SAVE_PENDING: AtomicBool = AtomicBool::new(false);

fn schedule_window_state_save(app_handle: tauri::AppHandle) {
    if SAVE_PENDING.swap(true, Ordering::SeqCst) {
        return; // a save is already pending
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1000));
        SAVE_PENDING.store(false, Ordering::SeqCst);
        if let Some(win) = app_handle.get_webview_window("main") {
            if let (Ok(size), Ok(pos)) = (win.inner_size(), win.outer_position()) {
                if size.width >= 100 && size.height >= 100 {
                    save_window_state(&app_handle, &WindowState {
                        width: size.width,
                        height: size.height,
                        x: pos.x,
                        y: pos.y,
                    });
                }
            }
        }
    });
}

/// Returns true if the left mouse button is currently held down.
/// Used to suppress blur-to-hide while the user is dragging or resizing
/// the window — if the mouse button is held, the window must never hide.
#[cfg(target_os = "windows")]
fn is_left_mouse_down() -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(vKey: i32) -> i16;
    }
    // VK_LBUTTON = 0x01. High bit set means key is down.
    unsafe { GetAsyncKeyState(0x01) < 0 }
}

#[cfg(not(target_os = "windows"))]
fn is_left_mouse_down() -> bool {
    false
}
use tauri_plugin_autostart::MacosLauncher;

#[cfg(desktop)]
#[derive(Clone, Serialize)]
struct SingleInstancePayload {
    args: Vec<String>,
    cwd: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // The single-instance plugin must be registered before every other plugin.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
        tray::show_panel(app, None);
        let _ = app.emit_to(
            "main",
            "single-instance",
            SingleInstancePayload { args, cwd },
        );
        let _ = app.emit_to("main", "focus-new-todo", ());
    }));

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_notification::init())
        .manage(i18n::I18nState::default())
        .manage(tray::PanelState::default())
        .manage(s3_sync::SyncRuntime::default())

        .setup(|app| {
            let app_dir = app.path().app_data_dir().unwrap();

            let database = db::Database::open(app.handle())?;
            app.manage(database);
            let note_asset_store = note_asset_store::NoteAssetStore::from_app(app.handle())?;
            app.manage(note_asset_store);
            // Tauri removes a tray icon when its last handle is dropped.
            // Store the handle in application state for the whole process lifetime.
            let tray_icon = tray::create_tray(app.handle())?;
            app.manage(tray_icon);
            reminders::start_reminder_scheduler(app.handle().clone());

            #[cfg(desktop)]
            create_flyout_window(app)?;

            // Restore main window size + position from previous session.
            // Must run after the window is created (it's defined in tauri.conf.json).
            if let Some(state) = load_window_state(app.handle()) {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.set_size(tauri::PhysicalSize::new(state.width, state.height));
                    let _ = win.set_position(tauri::PhysicalPosition::new(state.x, state.y));
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                    api.prevent_close();
                    // Save window state synchronously before hiding (the
                    // debounced save may not have fired yet if the user
                    // resized then immediately closed).
                    if let (Ok(size), Ok(pos)) = (window.inner_size(), window.outer_position()) {
                        if size.width >= 100 && size.height >= 100 {
                            save_window_state(&window.app_handle(), &WindowState {
                                width: size.width,
                                height: size.height,
                                x: pos.x,
                                y: pos.y,
                            });
                        }
                    }
                    let _ = window.hide();
                }
                WindowEvent::CloseRequested { api, .. } if window.label() == "focus" => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                WindowEvent::Focused(false) if window.label() == "main" => {
                    // 100ms delay — nearly instant to the user, but long enough to
                    // distinguish a click-outside (button released by then) from a
                    // drag-start (button still held). After the delay: if the left
                    // mouse button is held, the user is dragging/resizing — never
                    // hide. Otherwise run the normal blur-to-hide checks.
                    let app_handle = window.app_handle().clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        // If the left mouse button is held, the user is dragging
                        // or resizing — don't hide, no matter what.
                        if is_left_mouse_down() {
                            return;
                        }
                        if let Some(win) = app_handle.get_webview_window("main") {
                            // If the window is pinned (always on top), never hide
                            // on blur — the user explicitly wants it to stay visible.
                            if let Ok(true) = win.is_always_on_top() {
                                return;
                            }
                            // If the focus window is open and visible, don't hide
                            // the main window — the user just clicked "专注" and
                            // expects both windows to coexist.
                            if let Some(focus_win) = app_handle.get_webview_window("focus") {
                                if let Ok(true) = focus_win.is_visible() {
                                    return;
                                }
                            }
                            let panel_state = app_handle.state::<tray::PanelState>();
                            if panel_state.handle_blur() {
                                panel_state.set_panel_visible(false);
                                panel_state.clear_toggle_history();
                                panel_state.set_force_show_next();
                                let _ = win.hide();
                            }
                        }
                    });
                }
                // When the floating ball gains focus (user clicked it), mark an internal
                // interaction so the main panel's blur-to-hide is suppressed for a short
                // window. Without this, a second click on the ball first blurs the panel
                // (auto-hiding it), then the toggle sees it as hidden and re-shows it.
                WindowEvent::Focused(true) if window.label() == "flyout" => {
                    let panel_state = window.app_handle().state::<tray::PanelState>();
                    panel_state.mark_internal_interaction();
                }
                // When the main window is being resized or moved, mark dragging
                // so blur-to-hide is suppressed for 1 second after the last
                // gesture event. Without this, the window gets hidden mid-drag
                // (causing the "flash and snap back" bug on Windows).
                WindowEvent::Resized(_) if window.label() == "main" => {
                    let panel_state = window.app_handle().state::<tray::PanelState>();
                    panel_state.mark_dragging();
                    schedule_window_state_save(window.app_handle().clone());
                }
                WindowEvent::Moved(_) if window.label() == "main" => {
                    let panel_state = window.app_handle().state::<tray::PanelState>();
                    panel_state.mark_dragging();
                    schedule_window_state_save(window.app_handle().clone());
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_todos,
            commands::list_groups,
            commands::list_notes,
            commands::create_todo,
            commands::create_group,
            commands::create_note,
            commands::update_note,
            commands::set_note_pinned,
            commands::set_note_color,
            commands::delete_note,
            commands::restore_note,
            commands::list_note_attachments,
            commands::reorder_note_attachments,
            commands::create_note_image_attachment,
            commands::create_note_file_attachment,
            commands::read_note_attachment_preview,
            commands::read_note_attachment_original,
            commands::open_note_file_attachment,
            commands::get_note_attachment_cache_stats,
            commands::clear_note_attachment_cache,
            commands::delete_note_attachment,
            commands::restore_note_attachment,
            commands::retry_note_attachment,
            commands::update_group_name,
            commands::update_group_color,
            commands::delete_group,
            commands::reorder_groups,
            commands::set_todo_completed,
            commands::set_todo_completed_by_uuid,
            commands::update_todo_title,
            commands::update_todo_note,
            commands::set_todo_pinned,
            commands::set_todo_priority,
            commands::set_todo_url,
            commands::open_url,
            commands::set_todo_schedule,
            commands::set_todo_group,
            commands::reorder_todos,
            commands::delete_todo,
            commands::restore_todo,
            commands::clear_completed_todos,
            commands::archive_completed_todos,
            commands::hide_panel,
            commands::open_focus_window,
            commands::hide_focus_window,
            commands::set_focus_window_compact,
            commands::publish_focus_notification,
            commands::update_focus_tray_tooltip,
            commands::restore_tray_task_tooltip,
            commands::set_runtime_locale,
            commands::mark_panel_interaction,
            commands::toggle_panel_from_shortcut,
            commands::prepare_sync_document,
            commands::apply_remote_sync_document,
            commands::get_sync_settings,
            commands::save_sync_settings,
            commands::delete_sync_credentials,
            commands::test_sync_connection,
            commands::get_remote_sync_state,
            commands::upload_note_asset,
            commands::download_note_asset,
            commands::delete_remote_note_asset,
            commands::sync_now,
            commands::flyout_toggle_panel,
            commands::set_flyout_ignore_cursor,
            commands::play_complete_sound,
            commands::count_today_due,
            data_exchange::export_todos,
            data_exchange::export_full_backup,
            data_exchange::preview_todo_import,
            data_exchange::confirm_todo_import,
            data_exchange::preview_full_backup_import,
            data_exchange::confirm_full_backup_import,
            data_exchange::backup_database,
            commands::set_window_always_on_top,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Creates the always-on-top floating ball window. Tauri keeps the window
/// alive in its window registry, so it can be shown or hidden later by label.
#[cfg(desktop)]
fn create_flyout_window(app: &tauri::App) -> tauri::Result<()> {
    use tauri::{PhysicalPosition, WebviewWindowBuilder};

    // ~2 cm in logical pixels (96 DPI) for the default top-right placement.
    const DEFAULT_MARGIN_LP: f64 = 76.0;

    let flyout = WebviewWindowBuilder::new(
        app,
        "flyout",
        tauri::WebviewUrl::App("/flyout".into()),
    )
    .title("EggTodo 悬浮窗")
    .inner_size(70.0, 70.0)
    .min_inner_size(0.0, 0.0)
    .decorations(false)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(true)
    .transparent(true)
    .shadow(false)
    .build()?;

    // Force the window to exactly 70x70 after build. WebView2 has a minimum
    // width (~136px) for its control, but setting the *window* size to 70x70
    // works — the WebView is clipped by the window bounds. We also set the
    // window to ignore cursor events by default; the frontend re-enables them
    // only when the pointer is over the hit-area (via set_flyout_ignore_cursor).
    let _ = flyout.set_size(tauri::PhysicalSize::new(70, 70));
    let _ = flyout.set_min_size(Some(tauri::PhysicalSize::new(0, 0)));

    // Default position: top-right with ~2 cm margin.
    if let Ok(Some(monitor)) = flyout.primary_monitor() {
        let scale = monitor.scale_factor();
        let screen = monitor.size();
        if let Ok(win_size) = flyout.outer_size() {
            let margin = (DEFAULT_MARGIN_LP * scale) as i32;
            let x = screen.width as i32 - win_size.width as i32 - margin;
            let y = margin;
            let _ = flyout.set_position(PhysicalPosition::new(x, y));
        }
    }

    // Start a background task that tracks the global cursor position and
    // toggles click-through: when the cursor is over the ball's hit area,
    // the window receives input; otherwise it's fully transparent to clicks.
    // This works around WebView2's ~136px minimum window width — the extra
    // area on the right is always click-through.
    #[cfg(target_os = "windows")]
    {
        let flyout_clone = flyout.clone();
        // Use a native thread (not async) so std::thread::sleep doesn't block
        // the tokio runtime. This thread only does cursor polling + window calls.
        std::thread::spawn(move || {
            use windows::Win32::Foundation::POINT;
            use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

            let mut last_ignore = true;
            let _ = flyout_clone.set_ignore_cursor_events(true);

            loop {
                std::thread::sleep(std::time::Duration::from_millis(30));

                // Get global cursor position (physical pixels)
                let mut point = POINT { x: 0, y: 0 };
                if unsafe { GetCursorPos(&mut point) }.is_err() {
                    continue;
                }

                // Get window position and size
                let (win_x, win_y, win_w, win_h) = match (
                    flyout_clone.outer_position(),
                    flyout_clone.outer_size(),
                ) {
                    (Ok(pos), Ok(size)) => (
                        pos.x as i32,
                        pos.y as i32,
                        size.width as i32,
                        size.height as i32,
                    ),
                    _ => continue,
                };

                // Hit area: centered circle, ~56px diameter. The ball icon is
                // at the left side of the 136px-wide window, so shift the hit
                // area left by ~30px to align with the actual icon.
                let cx = win_x + win_w / 2 - 30;
                let cy = win_y + win_h / 2;
                let radius = 35;

                let dx = point.x - cx;
                let dy = point.y - cy;
                let over_ball = (dx * dx + dy * dy) <= (radius * radius);

                let should_ignore = !over_ball;
                if should_ignore != last_ignore {
                    let _ = flyout_clone.set_ignore_cursor_events(should_ignore);
                    last_ignore = should_ignore;
                }
            }
        });
    }

    Ok(())
}
