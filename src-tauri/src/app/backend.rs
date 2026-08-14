use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::Child;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Url, WebviewWindow};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

static BACKEND_CHILD: Mutex<Option<Child>> = Mutex::new(None);

pub fn is_backend_running(port: u16) -> bool {
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
    TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok()
}

fn find_cached_dsh_script() -> Option<PathBuf> {
    // 1. Check local ./server
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join("server").join("node_modules").join("@deepseek-ai").join("dsh").join("lib").join("bin.js");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 2. Check %LOCALAPPDATA%\npm-cache\_npx
    #[cfg(target_os = "windows")]
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        let npx_dir = PathBuf::from(local_appdata).join("npm-cache").join("_npx");
        if npx_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(npx_dir) {
                for entry in entries.flatten() {
                    let candidate = entry.path().join("node_modules").join("@deepseek-ai").join("dsh").join("lib").join("bin.js");
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    // 3. Check %APPDATA%\npm\node_modules
    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let candidate = PathBuf::from(appdata).join("npm").join("node_modules").join("@deepseek-ai").join("dsh").join("lib").join("bin.js");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

/// Perform check for both DSH-UI Shell (GitHub) and DeepSeek Engine (npm)
pub fn perform_update_check(window: &WebviewWindow, force: bool) {
    let win = window.clone();
    tauri::async_runtime::spawn(async move {
        let state_dir = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
        let state_file = std::path::PathBuf::from(state_dir).join("dsh-ui").join("update_state.json");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 1. If not forced, check 24h throttling
        if !force {
            if let Ok(content) = std::fs::read_to_string(&state_file) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(last_check) = json.get("last_check").and_then(|v| v.as_u64()) {
                        if now < last_check + 86400 {
                            return;
                        }
                    }
                }
            }
        }

        // Save current timestamp
        if let Some(parent) = state_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&state_file, format!(r#"{{"last_check": {}}}"#, now));

        // 2. Query GitHub Releases for DSH-UI Shell updates
        let mut cmd = std::process::Command::new("node");
        cmd.args(&["-e", r#"
            const https = require('https');
            const options = {
                headers: { 'User-Agent': 'DSH-UI-App' },
                timeout: 4000
            };
            const req = https.get('https://api.github.com/repos/xtxo/dsh-ui/releases/latest', options, res => {
                let data = '';
                res.on('data', chunk => data += chunk);
                res.on('end', () => {
                    try {
                        const json = JSON.parse(data);
                        if (json && json.tag_name) {
                            process.stdout.write(JSON.stringify({
                                tag: json.tag_name,
                                name: json.name || json.tag_name,
                                url: json.html_url || 'https://github.com/xtxo/dsh-ui/releases/latest',
                                body: (json.body || '').substring(0, 300)
                            }));
                        }
                    } catch(e) {}
                });
            });
            req.on('error', () => {});
        "#]);

        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000);

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Ok(release) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    if let Some(tag) = release.get("tag").and_then(|v| v.as_str()) {
                        let current_tag = "v0.1.3";
                        if tag != current_tag && !tag.is_empty() {
                            println!("[DSH-UI] New client shell release found on GitHub: {}", tag);
                            let url = release.get("url").and_then(|v| v.as_str()).unwrap_or("https://github.com/xtxo/dsh-ui/releases/latest");
                            let name = release.get("name").and_then(|v| v.as_str()).unwrap_or(tag);
                            
                            // Inject in-app update notification modal into webview
                            let js_code = format!(
                                r#"
                                (function() {{
                                    if (document.getElementById('dsh-update-banner')) return;
                                    const banner = document.createElement('div');
                                    banner.id = 'dsh-update-banner';
                                    banner.style.cssText = `
                                        position: fixed;
                                        bottom: 24px;
                                        right: 24px;
                                        background: rgba(15, 23, 42, 0.95);
                                        border: 1px solid rgba(77, 107, 254, 0.6);
                                        border-radius: 12px;
                                        padding: 16px 20px;
                                        color: #ffffff;
                                        box-shadow: 0 10px 30px rgba(0,0,0,0.5), 0 0 20px rgba(77,107,254,0.3);
                                        z-index: 999999;
                                        display: flex;
                                        flex-direction: column;
                                        gap: 10px;
                                        max-width: 360px;
                                        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                                        backdrop-filter: blur(12px);
                                        animation: dshSlideUp 0.3s ease;
                                    `;
                                    banner.innerHTML = `
                                        <div style="display: flex; align-items: center; justify-content: space-between;">
                                            <div style="font-weight: 700; font-size: 14px; color: #60a5fa; display: flex; align-items: center; gap: 6px;">
                                                <span>✨ 发现 DSH-UI 新版本</span>
                                                <span style="background: rgba(77,107,254,0.3); padding: 1px 6px; border-radius: 4px; font-size: 11px;">{tag}</span>
                                            </div>
                                            <button onclick="this.parentElement.parentElement.remove()" style="background: transparent; border: none; color: #94a3b8; cursor: pointer; font-size: 16px; line-height: 1;">&times;</button>
                                        </div>
                                        <div style="font-size: 12px; color: #cbd5e1; line-height: 1.5;">
                                            检测到客户端外壳已发布新版本 <strong>{name}</strong>，建议立即升级体验最新功能与修复。
                                        </div>
                                        <div style="display: flex; gap: 8px; margin-top: 4px;">
                                            <a href="{url}" target="_blank" style="background: #2563eb; color: #fff; text-decoration: none; padding: 6px 12px; border-radius: 6px; font-size: 12px; font-weight: 600; text-align: center; flex: 1; transition: background 0.2s;">
                                                🚀 立即下载更新
                                            </a>
                                            <button onclick="this.parentElement.parentElement.remove()" style="background: rgba(255,255,255,0.08); border: 1px solid rgba(255,255,255,0.15); color: #cbd5e1; padding: 6px 12px; border-radius: 6px; font-size: 12px; cursor: pointer;">
                                                稍后提醒
                                            </button>
                                        </div>
                                    `;
                                    document.body.appendChild(banner);
                                }})();
                                "#,
                                tag = tag,
                                name = name,
                                url = url
                            );
                            let _ = win.eval(&js_code);
                        }
                    }
                }
            }
        }
    });
}

pub fn start_backend_service_if_needed(_app_handle: &AppHandle, window: WebviewWindow) {
    let target_port: u16 = 3080;

    // Trigger 24-hour non-blocking update check for both shell & engine
    perform_update_check(&window, false);

    if is_backend_running(target_port) {
        println!("[DeepSeek Harness] Backend is already running on port {}", target_port);
        return;
    }

    let cached_script = find_cached_dsh_script();

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = std::process::Command::new("cmd.exe");

        if let Some(ref script) = cached_script {
            println!("[DeepSeek Harness] Fast boot from cached script: {:?}", script);
            cmd.args(&["/c", "node", script.to_str().unwrap_or(""), "web"]);
        } else {
            println!("[DeepSeek Harness] First run: downloading via npx @deepseek-ai/dsh web...");
            cmd.args(&["/c", "npx @deepseek-ai/dsh web"]);
        }

        cmd.creation_flags(CREATE_NO_WINDOW);

        match cmd.spawn() {
            Ok(child) => {
                println!("[DeepSeek Harness] Backend process spawned with PID: {}", child.id());
                if let Ok(mut lock) = BACKEND_CHILD.lock() {
                    *lock = Some(child);
                }
            }
            Err(e) => {
                eprintln!("[DeepSeek Harness] Failed to start backend service: {}", e);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut cmd = std::process::Command::new("sh");
        if let Some(ref script) = cached_script {
            cmd.args(&["-c", &format!("node \"{}\" web", script.display())]);
        } else {
            cmd.args(&["-c", "npx @deepseek-ai/dsh web"]);
        }

        match cmd.spawn() {
            Ok(child) => {
                if let Ok(mut lock) = BACKEND_CHILD.lock() {
                    *lock = Some(child);
                }
            }
            Err(e) => {
                eprintln!("[DeepSeek Harness] Failed to start backend service: {}", e);
            }
        }
    }

    // Background watcher to navigate as soon as port 3080 opens
    let win = window.clone();
    tauri::async_runtime::spawn(async move {
        for attempt in 1..=300 {
            tokio::time::sleep(Duration::from_millis(300)).await;
            if is_backend_running(target_port) {
                println!("[DeepSeek Harness] Backend ready after {} attempts! Navigating window...", attempt);
                if let Ok(target_url) = Url::parse("http://127.0.0.1:3080") {
                    let _ = win.navigate(target_url);
                }
                break;
            }
        }
    });
}

pub fn cleanup_backend() {
    if let Ok(mut lock) = BACKEND_CHILD.lock() {
        if let Some(mut child) = lock.take() {
            println!("[DeepSeek Harness] Stopping backend service (PID {})...", child.id());
            #[cfg(target_os = "windows")]
            {
                let mut kill_cmd = std::process::Command::new("taskkill");
                kill_cmd.args(&["/F", "/T", "/PID", &child.id().to_string()]);
                kill_cmd.creation_flags(0x08000000);
                let _ = kill_cmd.output();
            }
            let _ = child.kill();
        }
    }
}
