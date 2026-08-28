//! Linux tray over the StatusNotifierItem protocol via ksni.
//!
//! Tauri's tray-icon GTK backend (libappindicator) never forwards the
//! `Activate` D-Bus method that Deepin/KDE send on left click, so the shared
//! `TrayIconEvent` handler never fires on Linux. This module owns a native
//! SNI service instead: left click reaches `Tray::activate` and toggles the
//! panel, while the context menu keeps working through dbusmenu.

use ksni::{blocking::TrayMethods, menu::StandardItem, Icon, MenuItem, Status, ToolTip, Tray};
use tauri::{AppHandle, Emitter};

use crate::tray::{self, TraySnapshot};

pub struct LinuxTray {
    app: AppHandle,
    pub snapshot: TraySnapshot,
}

pub fn spawn(app: &AppHandle) -> Result<ksni::blocking::Handle<LinuxTray>, ksni::Error> {
    let snapshot = tray::tray_snapshot(app).unwrap_or_else(|| tray::base_snapshot(app));
    let tray = LinuxTray {
        app: app.clone(),
        snapshot,
    };
    tray.spawn()
}

impl Tray for LinuxTray {
    fn id(&self) -> String {
        "eggdone-tray".to_string()
    }

    fn title(&self) -> String {
        self.snapshot.locale.app_title().to_string()
    }

    fn status(&self) -> Status {
        Status::Active
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        // No tray rectangle is available here, so the panel falls back to the
        // screen corner, matching the tray menu's open action.
        tray::toggle_panel(&self.app, None);
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![Icon {
            width: self.snapshot.icon_width as i32,
            height: self.snapshot.icon_height as i32,
            data: rgba_to_argb(&self.snapshot.icon_rgba),
        }]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: self.snapshot.tooltip.clone(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let locale = self.snapshot.locale;
        let mut items = vec![
            action(locale.tray_toggle(), |tray| {
                tray::toggle_panel(&tray.app, None);
            }),
            action(locale.tray_new_task(), |tray| {
                tray::show_panel(&tray.app, None);
                let _ = tray.app.emit_to("main", "focus-new-todo", ());
            }),
            action(locale.tray_today(), |tray| {
                tray::show_panel(&tray.app, None);
                let _ = tray.app.emit_to("main", "show-today", ());
            }),
        ];
        if !self.snapshot.today_task_titles.is_empty() {
            items.push(MenuItem::Separator);
            for (index, title) in self.snapshot.today_task_titles.iter().enumerate() {
                let label = tray::today_task_menu_label(index, title);
                items.push(action(label, |tray| {
                    tray::show_panel(&tray.app, None);
                    let _ = tray.app.emit_to("main", "show-today", ());
                }));
            }
        }
        items.push(MenuItem::Separator);
        items.push(action(locale.tray_focus_start(), |tray| {
            let _ = tray.app.emit_to("focus", "focus-start", ());
        }));
        items.push(action(locale.tray_focus_toggle(), |tray| {
            let _ = tray.app.emit_to("focus", "focus-toggle", ());
        }));
        items.push(action(locale.tray_focus_end(), |tray| {
            let _ = tray.app.emit_to("focus", "focus-end", ());
        }));
        items.push(action(locale.tray_flyout_toggle(), |tray| {
            tray::toggle_flyout_window(&tray.app);
        }));
        items.push(MenuItem::Separator);
        items.push(action(locale.tray_about(), |tray| {
            tray::show_panel(&tray.app, None);
            let _ = tray.app.emit_to("main", "show-about", ());
        }));
        items.push(action(locale.tray_quit(), |tray| {
            tray.app.exit(0);
        }));
        items
    }
}

fn action(
    label: impl Into<String>,
    activate: impl Fn(&mut LinuxTray) + Send + 'static,
) -> MenuItem<LinuxTray> {
    MenuItem::Standard(StandardItem {
        label: label.into(),
        activate: Box::new(activate),
        ..Default::default()
    })
}

// ksni expects ARGB32 in network byte order while Tauri icons are RGBA.
fn rgba_to_argb(rgba: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(rgba.len());
    for pixel in rgba.chunks_exact(4) {
        data.extend_from_slice(&[pixel[3], pixel[0], pixel[1], pixel[2]]);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_rgba_pixels_to_argb_network_order() {
        let rgba = [1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(rgba_to_argb(&rgba), vec![4, 1, 2, 3, 8, 5, 6, 7]);
    }
}
