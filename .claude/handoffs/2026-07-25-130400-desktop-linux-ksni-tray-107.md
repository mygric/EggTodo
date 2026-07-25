# Handoff: EggDone Desktop Linux Build and ksni Tray Left-Click Fix (1.0.7)

## Session Metadata
- Created: 2026-07-25 13:04:00
- Project: /persistent/home/caozp/Develop/EggDone
- Branch: main
- Current version: 1.0.7
- Session duration: approximately 2.5 hours

### Recent Commits (for context)
  - 5e13c61 chore(release): 版本升级至 1.0.7
  - 7690d0d feat(tray): 重构托盘后端以ksni支持Linux左键激活面板
  - 32dc765 docs: 添加国际化DI6发布回归交接文档
  - 81d9648 fix: 调整摘要菜单按钮样式并更新待办计数文本
  - c5dbf04 feat: 实现国际化版本 1.0.6，新增中英文界面与质量门禁

## Handoff Chain

- **Continues from**: [2026-07-18-212216-desktop-i18n-di6-release-regression.md](./2026-07-18-212216-desktop-i18n-di6-release-regression.md)
  - Previous title: EggDone Desktop Internationalization DI6 Release Regression
- **Supersedes**: None

> Review the previous handoff for full context before filling this one.

## Current State Summary

The desktop app now builds on Linux (Deepin 25, Debian 12 base) and version was bumped to 1.0.7. The headline fix: tray left-click on Linux never worked because Tauri's tray-icon GTK backend (0.23.1) contains no event handling and libappindicator does not forward the StatusNotifierItem `Activate` D-Bus method that Deepin/KDE send on left click (confirmed via dbus-monitor). The Linux tray is now implemented with the `ksni` crate (native SNI protocol), so left-click toggles the panel directly; Tauri's tray remains for Windows/macOS and as the Linux fallback. All changes are committed (`7690d0d`, `5e13c61`), 111 Rust tests pass, and 1.0.7 deb + AppImage artifacts are built and user-verified on Deepin (left-click toggle confirmed working). DI6 Windows manual acceptance from the previous handoff remains open and untouched.

## Codebase Understanding

## Architecture Overview

- `src-tauri/src/tray.rs` now defines `TrayBackend` (enum: `Tauri(TrayIcon)` / Linux-only `Ksni(ksni::blocking::Handle<LinuxTray>)`) and a backend-neutral `TraySnapshot` (badged icon RGBA, tooltip, locale, today-task titles). `create_tray` tries ksni first on Linux and falls back to the Tauri tray; `lib.rs` manages the `TrayBackend` in state.
- `src-tauri/src/tray_ksni.rs` (cfg linux only) implements `ksni::Tray`: `activate()` → `toggle_panel(app, None)` (no tray rect available, panel goes to screen corner like the menu open action); `menu()` mirrors the Tauri menu items and their emit actions; `icon_pixmap()` converts RGBA → ARGB32 network byte order.
- `update_task_badge` / `update_focus_tooltip` branch on the backend: Tauri path uses `tray_by_id`-style setters via `apply_snapshot`; ksni path uses `handle.update(|tray| ...)`.
- The About page reads its version from `package.json`; bundle metadata comes from `tauri.conf.json`; crate version from `Cargo.toml` — all three must bump together, and `cargo check` syncs `Cargo.lock`.

## Critical Files

| File | Purpose | Relevance |
|------|---------|-----------|
| `src-tauri/src/tray_ksni.rs` | Linux SNI tray via ksni | New; owns left-click activate, menu, icon, tooltip on Linux |
| `src-tauri/src/tray.rs` | Tray backend abstraction + shared snapshot/menu logic | `TrayBackend`, `TraySnapshot`, `tray_snapshot`, `apply_snapshot`; Windows/macOS behavior unchanged |
| `src-tauri/Cargo.toml` | Dependencies | Linux target adds `ksni = { version = "0.3", default-features = false, features = ["async-io", "blocking"] }` |
| `pnpm-workspace.yaml` | pnpm build-script policy | `allowBuilds: { esbuild: true }` + `onlyBuiltDependencies: [esbuild]` — without this pnpm 11 skips the esbuild postinstall and frontend builds fail |
| `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` | Version metadata | All at 1.0.7 |

### Key Patterns Discovered

- Linux tray click behavior is DE-protocol-dependent: Deepin 25 dock sends `org.kde.StatusNotifierItem.Activate` on left click and renders the context menu itself via dbusmenu on right click (verified with `dbus-monitor --session "type='method_call',interface='org.kde.StatusNotifierItem'"`).
- The tray-icon crate 0.23.1 GTK backend source has zero event wiring — `TrayIconEvent` and `show_menu_on_left_click` are dead on Linux regardless of configuration.
- pnpm 11 auto-writes an `allowBuilds` placeholder into `pnpm-workspace.yaml` when it ignores build scripts; the placeholder string must be replaced with `true`/`false`, and `pnpm install` regenerates the placeholder if left unset.
- Tauri AppImage bundling downloads linuxdeploy at bundle time; a transient `failed to run linuxdeploy` is usually a download hiccup — retry before investigating.

## Work Completed

### Tasks Finished

- [x] Installed Linux build dependencies on Deepin 25 (webkit2gtk-4.1, gtk-3, ayatana-appindicator3, rsvg, soup-3 dev packages) and produced the first deb + AppImage.
- [x] Diagnosed dead left-click: dbus-monitor proved Deepin sends `Activate`; tray-icon crate source proved it is never forwarded.
- [x] Implemented the ksni Linux tray (`tray_ksni.rs`) with panel toggle, full menu parity, badge icon, and tooltip; added `TrayBackend` abstraction with automatic Tauri fallback.
- [x] User verified on Deepin: left-click shows/hides the panel; right-click menu, badge, and tooltip work.
- [x] Bumped version to 1.0.7 (package.json, tauri.conf.json, Cargo.toml, Cargo.lock) and rebuilt 1.0.7 deb + AppImage.
- [x] `cargo fmt --check`, `cargo check`, and 111 Rust tests all pass (added an RGBA→ARGB conversion test).
- [x] Updated README tray interaction note and AGENTS.md architecture boundary for the Linux tray.

## Files Modified

| File | Changes | Rationale |
|------|---------|-----------|
| `src-tauri/src/tray_ksni.rs` | New file: ksni `Tray` impl, menu parity, RGBA→ARGB conversion, spawn helper | Linux left-click cannot work through libappindicator |
| `src-tauri/src/tray.rs` | `TrayBackend` enum, `TraySnapshot`, `tray_snapshot`/`base_snapshot`/`apply_snapshot`, `create_tauri_tray` split, backend-branched updates | Share one snapshot across both backends; keep Windows/macOS logic intact |
| `src-tauri/src/lib.rs` | `#[cfg(target_os = "linux")] mod tray_ksni;` | Register Linux-only module |
| `src-tauri/Cargo.toml` / `Cargo.lock` | ksni 0.3.6 Linux-target dependency; version 1.0.7 | Native SNI implementation; release bump |
| `package.json`, `src-tauri/tauri.conf.json` | version 1.0.7 | About page and bundle metadata |
| `pnpm-workspace.yaml` | `allowBuilds: esbuild: true` | Unblock esbuild postinstall under pnpm 11 |
| `README.md`, `AGENTS.md` | Linux tray behavior and architecture notes | Keep docs aligned with behavior |

## Decisions Made

| Decision | Options Considered | Rationale |
|----------|-------------------|-----------|
| Implement Linux tray with ksni | gtk StatusIcon (XEmbed, X11-only, deprecated); accept right-click-only menu; patch libappindicator | ksni is the protocol-correct SNI implementation, works on Wayland too, matches the direction stalled upstream Tauri PRs are taking (clash-verge-rev did the same) |
| Keep Tauri tray as Linux fallback | ksni-only with hard failure | AGENTS.md requires a generic fallback for platform-specific implementations; menu still works if no SNI watcher exists |
| Left-click toggles panel with corner placement | Hide left-click; open menu on left-click (first attempt, failed) | Deepin sends Activate not ContextMenu on left click; menu-on-left-click also never fired because tray-icon ignores it |
| Bump to 1.0.7 (patch) | 1.1.0 | Bug-fix level change for the Linux platform, no new user-facing feature surface |

## Pending Work

## Immediate Next Steps

1. Smoke-test the 1.0.7 AppImage: left-click panel toggle plus About page showing 1.0.7 (the 1.0.6-asset build was already verified; 1.0.7 is identical code with new metadata).
2. DI6 manual Windows acceptance matrix remains open from the previous handoff: Chinese/English/system language × light/dark × default/narrow widths; tray, notifications, focus windows, runtime language switching, countdown continuity; record results in `docs/INTERNATIONALIZATION_ROADMAP.md` before checking DI6 boxes.
3. Decide whether 1.0.7 release packaging ships Linux artifacts only or waits for the DI6 Windows acceptance to publish a combined release.

### Blockers/Open Questions

- [ ] DI6 still requires real Windows UI access; no code blocker.
- [ ] Unverified edge: whether Deepin dock ever steals panel focus on tray click (blur-then-activate flicker). Not observed in testing, but the `PanelState` press/blur suppression only guards the Windows down/up event sequence; the ksni `activate` path relies on Deepin not blurring the panel.

### Deferred Items

- Old `EggDone_1.0.6_amd64.*` artifacts still sit in `src-tauri/target/release/bundle/` alongside 1.0.7 — clean up when publishing to avoid mix-ups.
- Pseudo-localization expanded-text screenshots (carried over from DI6, needs manual inspection).

## Context for Resuming Agent

## Important Context

- HEAD is `5e13c61` on `main`; the working tree was clean except this handoff file. All session work is committed — do not redo it.
- The Linux build host is this same machine (Deepin 25, x11 session). System deps for Tauri Linux builds are already installed; `pnpm install` has been run and must not lose the `allowBuilds: esbuild: true` line in `pnpm-workspace.yaml`.
- Build commands: `pnpm tauri build --bundles deb,appimage` (Rust release ≈ 4–5 min). Artifacts land in `src-tauri/target/release/bundle/{deb,appimage}/`.
- The ksni tray callbacks run on the ksni service thread; only Tauri window/emit APIs are called there (thread-safe) — do not touch GTK from those callbacks.
- `TraySnapshot.icon_rgba` is RGBA; ksni needs ARGB32 big-endian — the `rgba_to_argb` helper in `tray_ksni.rs` handles it; do not pass RGBA directly.
- The desktop repository remains independent from the Harmony client; align semantics only.

## Assumptions Made

- `system`, `zh-CN`, `en-US` remain the supported language modes; version is now 1.0.7.
- ksni failure at runtime falls back to the Tauri tray automatically (menu works, left-click dead) — this is intentional per AGENTS.md.
- Deepin dock renders dbusmenu from ksni the same way it did for libappindicator (verified in user testing).

## Potential Gotchas

- Rebuilding after a version bump: the deb/AppImage filenames embed the version — stale 1.0.6 files linger in the bundle directory.
- `cargo test` prints three result lines; the real suite is the first line (111 passed), the two zero-test lines are the bin targets.
- Ten pre-existing dead-code warnings in `reminders.rs`/`commands.rs` are known and unrelated — do not "fix" them during unrelated work.
- sudo in this environment cannot prompt for a password; ask the user to run apt commands themselves.
- Windows path was not regression-tested after the tray refactor (no Windows host here); logic is unchanged but the `TrayBackend` abstraction touched shared code — run the Windows tray checklist on the next Windows session.

## Environment State

### Tools/Services Used

- `pnpm tauri build --bundles deb,appimage`, `cargo fmt/check/test`, `pnpm install`
- `dbus-monitor` for SNI protocol diagnosis
- `dpkg-deb -I/-f` for deb metadata verification

### Active Processes

- A user-launched EggDone AppImage instance may be running for manual testing; stop it via the tray Quit menu before starting a new build's binary (single-instance plugin forwards to the running one).

### Environment Variables

- No special environment variables or credentials are required. No secrets are recorded in this handoff.

## Related Resources

- [Internationalization roadmap](../../docs/INTERNATIONALIZATION_ROADMAP.md) — DI6 manual items still open
- [Internationalization release notes](../../docs/INTERNATIONALIZATION_RELEASE_NOTES.md)
- [ksni crate docs](https://docs.rs/ksni/latest/ksni/)
- Previous handoff: [2026-07-18-212216-desktop-i18n-di6-release-regression.md](./2026-07-18-212216-desktop-i18n-di6-release-regression.md)

---

**Security Reminder**: Before finalizing, run `validate_handoff.py` to check for accidental secret exposure.
