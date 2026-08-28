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

use serde::Serialize;
use tauri::{Emitter, Manager, WindowEvent};
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

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                WindowEvent::CloseRequested { api, .. } if window.label() == "focus" => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                WindowEvent::Focused(false) if window.label() == "main" => {
                    // If the panel is pinned on top, keep it visible on blur.
                    if let Ok(true) = window.is_always_on_top() {
                        return;
                    }
                    // Delay the blur-to-hide by 150ms. This gives the flyout's
                    // markPanelInteraction IPC and Focused(true) event enough time
                    // to call mark_internal_interaction() before we decide whether
                    // to hide. Without the delay, main hides before the flyout's
                    // interaction is registered, so flyoutTogglePanel always sees
                    // is_visible()==false and re-shows instead of toggling.
                    let app_handle = window.app_handle().clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(150));
                        if let Some(win) = app_handle.get_webview_window("main") {
                            let panel_state = app_handle.state::<tray::PanelState>();
                            if panel_state.handle_blur() {
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
    .title("EggDone 悬浮窗")
    .inner_size(70.0, 70.0)
    .decorations(false)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(true)
    .transparent(true)
    .shadow(false)
    .build()?;

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

    Ok(())
}
