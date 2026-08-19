use crate::app::window::{
    hide_all_app_windows, open_additional_window_safe, show_all_app_windows, toggle_all_app_windows,
};
use crate::cancel_startup_reveal;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WebviewWindow,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const ENGINE_REGISTRY: &str = "https://registry.npmjs.org";
const ENGINE_UPDATE_INTERVAL_SECS: u64 = 6 * 60 * 60;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn prepare_hidden_command(command: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    #[cfg(not(target_os = "windows"))]
    {
        command.env("PATH", crate::app::backend::get_extended_path());
    }
}

fn node_executable() -> String {
    #[cfg(target_os = "windows")]
    {
        "node".to_string()
    }

    #[cfg(not(target_os = "windows"))]
    {
        crate::app::backend::find_node_executable()
    }
}

fn engine_state_file() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local_appdata)
                .join("dsh-ui")
                .join("engine_update_state.json");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("dsh-ui")
                .join("engine_update_state.json");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(data_home)
                .join("dsh-ui")
                .join("engine_update_state.json");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("dsh-ui")
                .join("engine_update_state.json");
        }
    }

    PathBuf::from("engine_update_state.json")
}

fn load_engine_state() -> serde_json::Map<String, serde_json::Value> {
    let path = engine_state_file();
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn save_engine_state(state: &serde_json::Map<String, serde_json::Value>) {
    let path = engine_state_file();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(state) {
        let _ = fs::write(path, text);
    }
}

fn read_engine_version(package_json: &Path) -> Option<String> {
    let text = fs::read_to_string(package_json).ok()?;
    let json = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    json.get("version")?.as_str().map(str::to_owned)
}

fn npm_global_package_json() -> Option<PathBuf> {
    let mut command = Command::new("npm");
    command.args(["root", "-g"]);
    prepare_hidden_command(&mut command);
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return None;
    }
    let candidate = PathBuf::from(root)
        .join("@deepseek-ai")
        .join("dsh")
        .join("package.json");
    candidate.exists().then_some(candidate)
}

fn current_engine_package_json() -> Option<PathBuf> {
    // The backend prefers an installed dsh CLI over cached/package-local copies.
    if crate::app::backend::find_dsh_cli().is_some() {
        if let Some(package_json) = npm_global_package_json() {
            return Some(package_json);
        }
    }

    // Portable bundle / local server copy.
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let bundled = exe_dir
                .join("server")
                .join("node_modules")
                .join("@deepseek-ai")
                .join("dsh")
                .join("package.json");
            if bundled.exists() {
                return Some(bundled);
            }

            #[cfg(target_os = "macos")]
            {
                let resource = exe_dir
                    .join("../Resources/server/node_modules/@deepseek-ai/dsh/package.json");
                if resource.exists() {
                    return Some(resource);
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
            let npx_dir = PathBuf::from(local_appdata).join("npm-cache").join("_npx");
            if let Ok(entries) = fs::read_dir(npx_dir) {
                for entry in entries.flatten() {
                    let candidate = entry
                        .path()
                        .join("node_modules")
                        .join("@deepseek-ai")
                        .join("dsh")
                        .join("package.json");
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
        }
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let candidate = PathBuf::from(appdata)
                .join("npm")
                .join("node_modules")
                .join("@deepseek-ai")
                .join("dsh")
                .join("package.json");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        for root in [
            "/opt/homebrew/lib/node_modules",
            "/usr/local/lib/node_modules",
            "/usr/lib/node_modules",
        ] {
            let candidate = PathBuf::from(root)
                .join("@deepseek-ai")
                .join("dsh")
                .join("package.json");
            if candidate.exists() {
                return Some(candidate);
            }
        }

        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            let npx_dir = home.join(".npm").join("_npx");
            if let Ok(entries) = fs::read_dir(npx_dir) {
                for entry in entries.flatten() {
                    let candidate = entry
                        .path()
                        .join("node_modules")
                        .join("@deepseek-ai")
                        .join("dsh")
                        .join("package.json");
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    None
}

fn current_engine_version() -> Option<String> {
    current_engine_package_json().and_then(|path| read_engine_version(&path))
}

fn latest_engine_version() -> Result<String, String> {
    let script = r#"
const https = require('https');
const req = https.get('https://registry.npmjs.org/@deepseek-ai%2Fdsh/latest', {
  headers: { 'User-Agent': 'DSH-UI-App' }
}, res => {
  let data = '';
  res.on('data', chunk => data += chunk);
  res.on('end', () => {
    try {
      const json = JSON.parse(data);
      if (!json.version) process.exit(3);
      process.stdout.write(String(json.version));
    } catch (_) {
      process.exit(4);
    }
  });
});
req.setTimeout(5000, () => req.destroy(new Error('timeout')));
req.on('error', err => {
  process.stderr.write(String(err && err.message ? err.message : err));
  process.exitCode = 2;
});
"#;

    let mut command = Command::new(node_executable());
    command.args(["-e", script]);
    prepare_hidden_command(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("无法查询 npm 最新版本: {error}"))?;
    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if details.is_empty() {
            "npm 版本查询失败".to_string()
        } else {
            format!("npm 版本查询失败: {details}")
        });
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        Err("npm 没有返回 @deepseek-ai/dsh 最新版本".to_string())
    } else {
        Ok(version)
    }
}

fn package_install_prefix(package_json: &Path) -> Option<PathBuf> {
    let mut cursor = package_json.parent();
    while let Some(path) = cursor {
        if path.file_name().and_then(|name| name.to_str()) == Some("node_modules") {
            return path.parent().map(Path::to_path_buf);
        }
        cursor = path.parent();
    }
    None
}

pub fn update_dsh_engine() -> Result<String, String> {
    let package_json = current_engine_package_json();
    let use_global = crate::app::backend::find_dsh_cli().is_some();

    let mut command = Command::new("npm");
    if use_global {
        command.args([
            "install",
            "-g",
            "--registry",
            ENGINE_REGISTRY,
            "@deepseek-ai/dsh@latest",
        ]);
    } else if let Some(prefix) = package_json
        .as_deref()
        .and_then(package_install_prefix)
    {
        command
            .arg("install")
            .arg("--prefix")
            .arg(prefix)
            .args([
                "--omit=dev",
                "--registry",
                ENGINE_REGISTRY,
                "@deepseek-ai/dsh@latest",
            ]);
    } else {
        command.args([
            "install",
            "-g",
            "--registry",
            ENGINE_REGISTRY,
            "@deepseek-ai/dsh@latest",
        ]);
    }
    prepare_hidden_command(&mut command);

    let output = command
        .output()
        .map_err(|error| format!("无法启动 npm 更新内核: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if details.is_empty() {
            "@deepseek-ai/dsh 更新失败".to_string()
        } else {
            format!("@deepseek-ai/dsh 更新失败: {details}")
        });
    }

    let version = latest_engine_version().unwrap_or_else(|_| "latest".to_string());
    Ok(format!("@deepseek-ai/dsh 已更新到 {version}，重启 DSH-UI 后生效"))
}

fn engine_button_bridge_script() -> &'static str {
    r#"
(function () {
  if (window.__dsh_engine_update_bridge) return;
  window.__dsh_engine_update_bridge = true;

  const refreshButton = () => {
    const button = document.getElementById('dsh-btn-hot-reload');
    if (button) {
      button.textContent = '⚡ 更新内核（重启生效）';
      button.title = '真实更新 @deepseek-ai/dsh，不再只是刷新页面';
    }
  };

  refreshButton();
  new MutationObserver(refreshButton).observe(document.documentElement, {
    childList: true,
    subtree: true
  });

  document.addEventListener('click', async event => {
    const target = event.target instanceof Element ? event.target.closest('#dsh-btn-hot-reload') : null;
    if (!target) return;
    event.preventDefault();
    event.stopImmediatePropagation();

    const status = document.getElementById('dsh-check-status');
    target.disabled = true;
    if (status) status.innerHTML = '<span style="color:#60a5fa;">⚡ 正在从 npm 更新 @deepseek-ai/dsh，请稍候...</span>';

    try {
      const result = await window.__TAURI__.core.invoke('webview_navigate', { action: 'update_engine' });
      if (status) status.innerHTML = '<span style="color:#34d399;font-weight:700;">✅ ' + String(result || '内核更新完成，请重启 DSH-UI 生效') + '</span>';
      target.textContent = '✅ 已更新，请重启';
    } catch (error) {
      if (status) status.innerHTML = '<span style="color:#f87171;font-weight:700;">❌ 更新失败：' + String(error) + '</span>';
      target.disabled = false;
    }
  }, true);
})();
"#
}

fn show_engine_update_banner(window: &WebviewWindow, current: &str, latest: &str) {
    let current_json = serde_json::to_string(current).unwrap_or_else(|_| "\"unknown\"".to_string());
    let latest_json = serde_json::to_string(latest).unwrap_or_else(|_| "\"latest\"".to_string());
    let script = r#"
(function () {
  const current = __CURRENT__;
  const latest = __LATEST__;
  const old = document.getElementById('dsh-engine-update-banner');
  if (old) old.remove();

  const banner = document.createElement('div');
  banner.id = 'dsh-engine-update-banner';
  banner.style.cssText = 'position:fixed;bottom:24px;right:24px;background:rgba(15,23,42,.97);border:1px solid rgba(16,185,129,.65);border-radius:12px;padding:16px 18px;color:#fff;box-shadow:0 10px 30px rgba(0,0,0,.5);z-index:1000000;max-width:380px;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;';
  banner.innerHTML = '<div style="font-weight:700;color:#34d399;margin-bottom:8px;">🐳 DeepSeek Harness 内核有新版本</div>' +
    '<div style="font-size:12px;color:#cbd5e1;line-height:1.6;margin-bottom:12px;">当前 <strong>' + current + '</strong> → 最新 <strong>' + latest + '</strong></div>' +
    '<div style="display:flex;gap:8px;">' +
      '<button id="dsh-engine-update-now" style="flex:1;background:#10b981;color:white;border:0;padding:8px 12px;border-radius:7px;font-weight:700;cursor:pointer;">立即更新内核</button>' +
      '<button id="dsh-engine-update-later" style="background:rgba(255,255,255,.08);color:#cbd5e1;border:1px solid rgba(255,255,255,.15);padding:8px 12px;border-radius:7px;cursor:pointer;">稍后</button>' +
    '</div>' +
    '<div id="dsh-engine-update-result" style="font-size:11px;color:#94a3b8;margin-top:8px;"></div>';
  document.body.appendChild(banner);

  document.getElementById('dsh-engine-update-later').onclick = () => banner.remove();
  document.getElementById('dsh-engine-update-now').onclick = async function () {
    const button = this;
    const result = document.getElementById('dsh-engine-update-result');
    button.disabled = true;
    button.textContent = '更新中...';
    result.textContent = '正在从 npm 获取并安装最新 @deepseek-ai/dsh';
    try {
      const message = await window.__TAURI__.core.invoke('webview_navigate', { action: 'update_engine' });
      result.style.color = '#34d399';
      result.textContent = String(message || '更新完成，请重启 DSH-UI 生效');
      button.textContent = '✅ 已更新';
    } catch (error) {
      result.style.color = '#f87171';
      result.textContent = '更新失败：' + String(error);
      button.disabled = false;
      button.textContent = '重试更新';
    }
  };
})();
"#
    .replace("__CURRENT__", &current_json)
    .replace("__LATEST__", &latest_json);
    let _ = window.eval(&script);
}

fn show_engine_status_banner(window: &WebviewWindow, message: &str, success: bool) {
    let message_json = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".to_string());
    let success_js = if success { "true" } else { "false" };
    let script = r#"
(function () {
  const message = __MESSAGE__;
  const success = __SUCCESS__;
  const old = document.getElementById('dsh-engine-status-banner');
  if (old) old.remove();
  const banner = document.createElement('div');
  banner.id = 'dsh-engine-status-banner';
  banner.style.cssText = 'position:fixed;bottom:24px;right:24px;background:rgba(15,23,42,.97);border:1px solid ' + (success ? 'rgba(16,185,129,.65)' : 'rgba(248,113,113,.65)') + ';border-radius:10px;padding:12px 16px;color:' + (success ? '#34d399' : '#fca5a5') + ';box-shadow:0 10px 30px rgba(0,0,0,.45);z-index:1000000;font:600 12px -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;';
  banner.textContent = message;
  document.body.appendChild(banner);
  setTimeout(() => banner.remove(), 6500);
})();
"#
    .replace("__MESSAGE__", &message_json)
    .replace("__SUCCESS__", success_js);
    let _ = window.eval(&script);
}

fn inject_engine_bridge_when_ready(window: WebviewWindow) {
    tauri::async_runtime::spawn(async move {
        for _ in 0..120 {
            if crate::app::backend::is_backend_running(3080) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        tokio::time::sleep(Duration::from_millis(1200)).await;
        let _ = window.eval(engine_button_bridge_script());
    });
}

pub fn perform_engine_update_check(app: &AppHandle, window: &WebviewWindow, force: bool) {
    inject_engine_bridge_when_ready(window.clone());

    let app = app.clone();
    let win = window.clone();
    tauri::async_runtime::spawn(async move {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut state = load_engine_state();
        let last_check = state
            .get("last_check")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);

        if !force && now.saturating_sub(last_check) < ENGINE_UPDATE_INTERVAL_SECS {
            return;
        }

        state.insert("last_check".to_string(), serde_json::Value::from(now));
        save_engine_state(&state);

        let latest = match latest_engine_version() {
            Ok(version) => version,
            Err(error) => {
                if force {
                    show_engine_status_banner(&win, &format!("内核更新检查失败：{error}"), false);
                }
                return;
            }
        };

        let Some(current) = current_engine_version() else {
            if force {
                show_engine_status_banner(
                    &win,
                    "暂时无法识别当前 @deepseek-ai/dsh 版本；后台服务启动后再试一次。",
                    false,
                );
            }
            return;
        };

        if current == latest {
            if force {
                show_engine_status_banner(
                    &win,
                    &format!("✅ @deepseek-ai/dsh 已是最新版本 {current}"),
                    true,
                );
            }
            return;
        }

        let last_notified = state
            .get("last_notified_version")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();

        if force || last_notified != latest {
            let _ = app
                .notification()
                .builder()
                .title("DeepSeek Harness 内核有新版本")
                .body(&format!("@deepseek-ai/dsh {current} → {latest}，可在 DSH-UI 中立即更新"))
                .show();
            state.insert(
                "last_notified_version".to_string(),
                serde_json::Value::String(latest.clone()),
            );
            save_engine_state(&state);
        }

        for _ in 0..120 {
            if crate::app::backend::is_backend_running(3080) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        tokio::time::sleep(Duration::from_millis(1200)).await;
        let _ = win.eval(engine_button_bridge_script());
        show_engine_update_banner(&win, &current, &latest);
    });
}

pub fn set_system_tray(
    app: &AppHandle,
    show_system_tray: bool,
    tray_icon_path: &str,
    _init_fullscreen: bool,
    allow_multi_window: bool,
    startup_revealed: Arc<AtomicBool>,
) -> tauri::Result<()> {
    // Engine update checks are independent from the tray itself so users who disable
    // the tray still receive a native notification when @deepseek-ai/dsh releases.
    if let Some(window) = app.get_webview_window("pake") {
        perform_engine_update_check(app, &window, false);
    }

    if !show_system_tray {
        app.remove_tray_by_id("pake-tray");
        return Ok(());
    }

    // Menu events are broadcast to every handler in Tauri v2, so the tray item
    let version_item = MenuItemBuilder::with_id("app_version", "DeepSeek Harness v0.1.11")
        .enabled(false)
        .build(app)?;
    let check_update = MenuItemBuilder::with_id("check_update", "🔍 检查更新 (Check Updates)").build(app)?;
    let new_window = MenuItemBuilder::with_id("tray_new_window", "New Window").build(app)?;
    let hide_app = MenuItemBuilder::with_id("hide_app", "Hide").build(app)?;
    let show_app = MenuItemBuilder::with_id("show_app", "Show").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = if allow_multi_window {
        MenuBuilder::new(app)
            .items(&[&version_item, &new_window, &check_update, &hide_app, &show_app, &quit])
            .build()?
    } else {
        MenuBuilder::new(app)
            .items(&[&version_item, &check_update, &hide_app, &show_app, &quit])
            .build()?
    };

    app.app_handle().remove_tray_by_id("pake-tray");

    let menu_revealed = startup_revealed.clone();
    let click_revealed = startup_revealed;
    let mut tray_builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("DeepSeek Harness v0.1.11 (双击打开)")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "tray_new_window" => {
                open_additional_window_safe(app);
            }
            "check_update" => {
                if let Some(window) = app.get_webview_window("pake") {
                    crate::app::backend::perform_update_check(&window, true);
                    perform_engine_update_check(app, &window, true);
                }
            }
            "hide_app" => {
                // Hide every webview (main + multi-window clones), not only "pake".
                cancel_startup_reveal(&menu_revealed);
                hide_all_app_windows(app);
            }
            "show_app" => {
                cancel_startup_reveal(&menu_revealed);
                show_all_app_windows(app, _init_fullscreen);
            }
            "quit" => {
                let flags = if _init_fullscreen {
                    StateFlags::all()
                } else {
                    StateFlags::all() & !StateFlags::FULLSCREEN
                };
                let _ = app.save_window_state(flags);
                app.exit(0);
            }
            _ => (),
        })
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                // Windows emits Click twice per physical click (Down then Up).
                // Reacting to both runs the toggle twice, so a hidden window is
                // shown and immediately re-hidden and the tray looks dead (#1343).
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    // Any tray toggle claims visibility control from startup reveal.
                    cancel_startup_reveal(&click_revealed);
                    toggle_all_app_windows(tray.app_handle(), _init_fullscreen);
                }
            }
        });

    let resolved_icon = if tray_icon_path.is_empty() {
        app.default_window_icon().cloned()
    } else {
        tauri::image::Image::from_path(tray_icon_path)
            .ok()
            .or_else(|| app.default_window_icon().cloned())
    };

    if let Some(icon) = resolved_icon {
        tray_builder = tray_builder.icon(icon);
    } else {
        eprintln!("[Pake] No tray icon available; tray will build without an icon.");
    }

    let tray = tray_builder.build(app)?;

    tray.set_icon_as_template(false)?;
    Ok(())
}

pub fn set_global_shortcut(
    app: &AppHandle,
    shortcut: String,
    _init_fullscreen: bool,
    startup_revealed: Arc<AtomicBool>,
) -> tauri::Result<()> {
    if shortcut.is_empty() {
        return Ok(());
    }

    let app_handle = app.clone();
    let shortcut_hotkey = match Shortcut::from_str(&shortcut) {
        Ok(s) => s,
        Err(error) => {
            eprintln!("[Pake] Invalid activation shortcut '{shortcut}': {error}");
            return Ok(());
        }
    };
    let last_triggered = Arc::new(Mutex::new(Instant::now()));

    if let Err(error) = app_handle.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler({
                let last_triggered = Arc::clone(&last_triggered);
                let startup_revealed = startup_revealed.clone();
                move |app, event, _shortcut| {
                    let Ok(mut last_triggered) = last_triggered.lock() else {
                        return;
                    };
                    if Instant::now().duration_since(*last_triggered) < Duration::from_millis(300) {
                        return;
                    }
                    *last_triggered = Instant::now();

                    if shortcut_hotkey.eq(event) {
                        cancel_startup_reveal(&startup_revealed);
                        toggle_all_app_windows(app, _init_fullscreen);
                    }
                }
            })
            .build(),
    ) {
        eprintln!(
            "[Pake] Failed to register global shortcut plugin '{shortcut}': {error}; continuing without it."
        );
        return Ok(());
    }

    if let Err(error) = app.global_shortcut().register(shortcut_hotkey) {
        eprintln!("[Pake] Failed to bind global shortcut '{shortcut}': {error}");
    }

    Ok(())
}
