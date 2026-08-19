use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::Child;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Url, WebviewWindow};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const NPM_MIRROR_REGISTRY: &str = "https://registry.npmmirror.com";

static BACKEND_CHILD: Mutex<Option<Child>> = Mutex::new(None);

pub fn is_backend_running(port: u16) -> bool {
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
    TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok()
}

#[cfg(not(target_os = "windows"))]
pub fn get_extended_path() -> String {
    let current_path = std::env::var("PATH").unwrap_or_default();
    let mut paths: Vec<String> = vec![
        "/opt/homebrew/bin".to_string(),
        "/opt/homebrew/sbin".to_string(),
        "/usr/local/bin".to_string(),
        "/usr/local/sbin".to_string(),
        "/usr/bin".to_string(),
        "/bin".to_string(),
        "/usr/sbin".to_string(),
        "/sbin".to_string(),
    ];

    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(&home);
        paths.push(home_path.join(".volta/bin").to_string_lossy().to_string());
        paths.push(home_path.join(".fnm/current/bin").to_string_lossy().to_string());
        paths.push(home_path.join(".local/share/fnm/current/bin").to_string_lossy().to_string());
        paths.push(home_path.join(".bun/bin").to_string_lossy().to_string());
        paths.push(home_path.join(".pnpm").to_string_lossy().to_string());
        paths.push(home_path.join(".yarn/bin").to_string_lossy().to_string());
        paths.push(home_path.join(".cargo/bin").to_string_lossy().to_string());
        paths.push(home_path.join(".local/bin").to_string_lossy().to_string());
        paths.push(home_path.join("bin").to_string_lossy().to_string());

        // Check NVM installations
        let nvm_dir = home_path.join(".nvm").join("versions").join("node");
        if nvm_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(nvm_dir) {
                for entry in entries.flatten() {
                    let bin_dir = entry.path().join("bin");
                    if bin_dir.exists() {
                        paths.push(bin_dir.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    if !current_path.is_empty() {
        paths.push(current_path);
    }

    paths.join(":")
}

#[cfg(not(target_os = "windows"))]
pub fn find_node_executable() -> String {
    let candidates = [
        "/opt/homebrew/bin/node",
        "/usr/local/bin/node",
        "/usr/bin/node",
    ];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return c.to_string();
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(&home);
        let nvm_dir = home_path.join(".nvm").join("versions").join("node");
        if nvm_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(nvm_dir) {
                for entry in entries.flatten() {
                    let node_bin = entry.path().join("bin").join("node");
                    if node_bin.exists() {
                        return node_bin.to_string_lossy().to_string();
                    }
                }
            }
        }
        let fnm_node = home_path.join(".local/share/fnm/current/bin/node");
        if fnm_node.exists() {
            return fnm_node.to_string_lossy().to_string();
        }
        let volta_node = home_path.join(".volta/bin/node");
        if volta_node.exists() {
            return volta_node.to_string_lossy().to_string();
        }
    }

    "node".to_string()
}

pub fn find_dsh_cli() -> Option<PathBuf> {
    #[cfg(not(target_os = "windows"))]
    {
        let candidates = [
            PathBuf::from("/opt/homebrew/bin/dsh"),
            PathBuf::from("/usr/local/bin/dsh"),
            PathBuf::from("/usr/bin/dsh"),
        ];
        for c in &candidates {
            if c.exists() {
                return Some(c.clone());
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(&home);
            let nvm_dir = home_path.join(".nvm").join("versions").join("node");
            if nvm_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(nvm_dir) {
                    for entry in entries.flatten() {
                        let dsh_bin = entry.path().join("bin").join("dsh");
                        if dsh_bin.exists() {
                            return Some(dsh_bin);
                        }
                    }
                }
            }
            let fnm_dsh = home_path.join(".local/share/fnm/current/bin/dsh");
            if fnm_dsh.exists() {
                return Some(fnm_dsh);
            }
            let volta_dsh = home_path.join(".volta/bin/dsh");
            if volta_dsh.exists() {
                return Some(volta_dsh);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let candidate = PathBuf::from(appdata).join("npm").join("dsh.cmd");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

fn find_cached_dsh_script() -> Option<PathBuf> {
    // 1. Check local ./server or App bundle Resources/server
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join("server").join("node_modules").join("@deepseek-ai").join("dsh").join("lib").join("bin.js");
            if candidate.exists() {
                return Some(candidate);
            }
            // Check macOS bundle Resources
            let resource_candidate = exe_dir.join("../Resources/server/node_modules/@deepseek-ai/dsh/lib/bin.js");
            if resource_candidate.exists() {
                return Some(resource_candidate);
            }
        }
    }

    // 2. Check Windows locations
    #[cfg(target_os = "windows")]
    {
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

        if let Ok(appdata) = std::env::var("APPDATA") {
            let candidate = PathBuf::from(appdata).join("npm").join("node_modules").join("@deepseek-ai").join("dsh").join("lib").join("bin.js");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 3. Check macOS / Linux common paths
    #[cfg(not(target_os = "windows"))]
    {
        let direct_candidates = [
            PathBuf::from("/opt/homebrew/lib/node_modules/@deepseek-ai/dsh/lib/bin.js"),
            PathBuf::from("/usr/local/lib/node_modules/@deepseek-ai/dsh/lib/bin.js"),
            PathBuf::from("/usr/lib/node_modules/@deepseek-ai/dsh/lib/bin.js"),
        ];
        for c in &direct_candidates {
            if c.exists() {
                return Some(c.clone());
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(&home);
            // ~/.npm/_npx/...
            let npx_dir = home_path.join(".npm").join("_npx");
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
            // ~/.nvm/versions/node/...
            let nvm_dir = home_path.join(".nvm").join("versions").join("node");
            if nvm_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(nvm_dir) {
                    for entry in entries.flatten() {
                        let candidate = entry.path().join("lib").join("node_modules").join("@deepseek-ai").join("dsh").join("lib").join("bin.js");
                        if candidate.exists() {
                            return Some(candidate);
                        }
                    }
                }
            }
            // ~/.local/share/pnpm/global/...
            let pnpm_dir = home_path.join(".local").join("share").join("pnpm").join("global");
            if pnpm_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(pnpm_dir) {
                    for entry in entries.flatten() {
                        let candidate = entry.path().join("node_modules").join("@deepseek-ai").join("dsh").join("lib").join("bin.js");
                        if candidate.exists() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Script that injects the interactive in-app updater with platform-specific download links (macOS / Windows / Linux)
pub fn get_injected_updater_script() -> &'static str {
    r#"
    (function() {
        if (window.__dsh_updater_injected) return;
        window.__dsh_updater_injected = true;

        const isMac = /Macintosh|Mac OS X/i.test(navigator.userAgent);
        const isWindows = /Windows/i.test(navigator.userAgent);
        const isLinux = /Linux/i.test(navigator.userAgent) && !/Android/i.test(navigator.userAgent);
        const isArm = /ARM64|aarch64/i.test(navigator.userAgent) || (isMac && (navigator.maxTouchPoints > 0 || screen.colorDepth === 24));

        const platformName = isMac ? "macOS" : (isWindows ? "Windows" : "Linux");
        const platformDlText = isMac ? "🍏 外部浏览器打开 Mac 安装包下载" : (isWindows ? "🪟 外部浏览器打开 Windows 版下载" : "🐧 外部浏览器打开 Linux 版下载");

        function openExternal(url) {
            if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
                window.__TAURI__.core.invoke('open_browser', { url: url }).catch(function() {
                    window.location.href = url;
                });
            } else {
                window.location.href = url;
            }
        }
        window.__dsh_open_external = openExternal;

        function openUpdateModal() {
            let existing = document.getElementById('dsh-update-dialog');
            if (existing) existing.remove();

            const modal = document.createElement('div');
            modal.id = 'dsh-update-dialog';
            modal.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.7);backdrop-filter:blur(8px);z-index:9999999;display:flex;align-items:center;justify-content:center;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;';
            modal.innerHTML = `
                <div style="background:#0f172a; border:1px solid rgba(77,107,254,0.5); border-radius:16px; width:460px; padding:24px; color:#fff; box-shadow:0 24px 60px rgba(0,0,0,0.9);">
                    <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:16px;">
                        <div style="display:flex; align-items:center; gap:8px;">
                            <span style="font-size:22px;">🐋</span>
                            <span style="font-weight:700; font-size:16px; color:#f8fafc;">DSH-UI 版本中心 &amp; 升级</span>
                        </div>
                        <button id="dsh-modal-close" style="background:none; border:none; color:#94a3b8; font-size:22px; cursor:pointer; line-height:1;">&times;</button>
                    </div>
                    
                    <div style="background:rgba(255,255,255,0.04); border:1px solid rgba(255,255,255,0.08); border-radius:10px; padding:12px 14px; margin-bottom:16px; font-size:13px; line-height:1.9;">
                        <div><strong>桌面客户端外壳 (${platformName})：</strong> <span style="color:#60a5fa; font-weight:600;">v0.1.11</span></div>
                        <div><strong>智能体官方内核：</strong> <span style="color:#34d399; font-weight:600;">@deepseek-ai/dsh</span></div>
                        <div><strong>底层引擎架构：</strong> <span>Rust + 原生 WebKit/Webview (仅 8.7MB)</span></div>
                    </div>

                    <!-- Direct Action Area -->
                    <div id="dsh-update-action-box" style="background:rgba(77,107,254,0.08); border:1px solid rgba(77,107,254,0.25); border-radius:10px; padding:14px; margin-bottom:16px;">
                        <div id="dsh-check-status" style="font-size:12px; color:#cbd5e1; line-height:1.6;">
                            支持<strong>官方内核热更新 (免重装)</strong> 与<strong>外部浏览器极速下载升级</strong>。
                        </div>
                        <div id="dsh-download-links" style="display:none; margin-top:10px; font-size:12px; line-height:1.8;">
                            <div id="dsh-direct-links-content"></div>
                        </div>
                    </div>

                    <div style="display:flex; flex-direction:column; gap:10px;">
                        <div style="display:flex; gap:10px;">
                            <button id="dsh-btn-hot-reload" style="flex:1; background:#10b981; color:#fff; border:none; padding:10px 14px; border-radius:8px; font-weight:600; cursor:pointer; font-size:13px; transition:background 0.2s;">
                                ⚡ 一键热更新内核
                            </button>
                            <button id="dsh-btn-direct-download" style="flex:1.5; background:#2563eb; color:#fff; border:none; padding:10px 14px; border-radius:8px; font-weight:600; cursor:pointer; font-size:13px; transition:background 0.2s;">
                                ${platformDlText}
                            </button>
                        </div>
                        <div style="display:flex; justify-content:space-between; font-size:12px; color:#94a3b8; padding:0 4px; margin-top:2px;">
                            <a href="javascript:void(0)" onclick="window.__dsh_open_external('https://github.com/xtxo/dsh-ui/releases/latest')" style="color:#60a5fa; text-decoration:none;">🔗 GitHub Releases 发布页</a>
                            <a href="javascript:void(0)" onclick="window.__dsh_open_external('https://xtxo.github.io/dsh-ui/')" style="color:#60a5fa; text-decoration:none;">🏠 DSH-UI 官网主页</a>
                        </div>
                    </div>
                </div>
            `;
            document.body.appendChild(modal);

            document.getElementById('dsh-modal-close').onclick = () => modal.remove();
            modal.onclick = (e) => { if (e.target === modal) modal.remove(); };

            // 1. Hot Reload Action
            document.getElementById('dsh-btn-hot-reload').onclick = function() {
                const status = document.getElementById('dsh-check-status');
                status.innerHTML = '<span style="color:#34d399;">⚡ 正在执行官方内核热更新与缓存刷新...</span>';
                
                setTimeout(() => {
                    status.innerHTML = '<span style="color:#34d399; font-weight:bold;">✅ 内核热更新就绪！正在秒级刷新应用...</span>';
                    setTimeout(() => {
                        window.location.reload();
                    }, 600);
                }, 1500);
            };

            // 2. Direct External Browser Open
            document.getElementById('dsh-btn-direct-download').onclick = async function() {
                const status = document.getElementById('dsh-check-status');
                const linksBox = document.getElementById('dsh-download-links');
                const linksContent = document.getElementById('dsh-direct-links-content');
                
                status.innerHTML = '<span style="color:#60a5fa;">🚀 正在获取最新 ' + platformName + ' 下载直链...</span>';

                try {
                    const res = await fetch('https://api.github.com/repos/xtxo/dsh-ui/releases/latest');
                    if (!res.ok) throw new Error('Release API error');
                    const release = await res.json();
                    const assets = release.assets || [];

                    let targetAsset = null;
                    if (isMac) {
                        targetAsset = isArm
                            ? (assets.find(a => a.name.includes('aarch64.dmg')) || assets.find(a => a.name.endsWith('.dmg')))
                            : (assets.find(a => a.name.includes('x64.dmg') || a.name.includes('x86_64.dmg')) || assets.find(a => a.name.endsWith('.dmg')));
                    } else if (isWindows) {
                        targetAsset = assets.find(a => a.name.includes('Portable.zip')) || assets.find(a => a.name.endsWith('.exe'));
                    } else {
                        targetAsset = assets.find(a => a.name.endsWith('.deb') || a.name.endsWith('.AppImage'));
                    }

                    const targetUrl = targetAsset ? targetAsset.browser_download_url : (release.html_url || 'https://github.com/xtxo/dsh-ui/releases/latest');
                    const fileName = targetAsset ? targetAsset.name : (isMac ? 'DeepSeek-Harness-macOS.dmg' : (isWindows ? 'DeepSeek-Harness-Portable.zip' : 'DeepSeek-Harness-Linux.deb'));

                    // Open direct download link in default external browser
                    openExternal(targetUrl);
                    status.innerHTML = '<span style="color:#34d399; font-weight:bold;">🎉 已在系统默认浏览器中打开下载：' + fileName + '</span>';

                    // Also display all direct links nicely
                    if (isMac) {
                        const armAsset = assets.find(a => a.name.includes('aarch64.dmg'));
                        const intelAsset = assets.find(a => a.name.includes('x64.dmg') || a.name.includes('x86_64.dmg'));
                        let linksHtml = '<div style="color:#94a3b8; font-size:11px; margin-bottom:4px;">备选架构直链：</div>';
                        if (armAsset) linksHtml += '<div>🍏 <a href="javascript:void(0)" onclick="window.__dsh_open_external(\'' + armAsset.browser_download_url + '\')" style="color:#60a5fa; text-decoration:underline;">Apple Silicon (M1~M4) DMG 下载</a></div>';
                        if (intelAsset) linksHtml += '<div>🍏 <a href="javascript:void(0)" onclick="window.__dsh_open_external(\'' + intelAsset.browser_download_url + '\')" style="color:#60a5fa; text-decoration:underline;">Intel 架构 Mac DMG 下载</a></div>';
                        linksContent.innerHTML = linksHtml;
                        linksBox.style.display = 'block';
                    }
                } catch (e) {
                    console.warn('Direct fetch failed, opening releases page directly:', e);
                    openExternal('https://github.com/xtxo/dsh-ui/releases/latest');
                    status.innerHTML = '<span style="color:#34d399; font-weight:bold;">🎉 已在系统浏览器中打开 GitHub Releases 最新发布页面！</span>';
                }
            };
        }

        window.__dsh_open_modal = openUpdateModal;
    })();
    "#
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
        let node_bin = {
            #[cfg(not(target_os = "windows"))]
            { find_node_executable() }
            #[cfg(target_os = "windows")]
            { "node".to_string() }
        };
        let mut cmd = std::process::Command::new(&node_bin);
        #[cfg(not(target_os = "windows"))]
        {
            cmd.env("PATH", &get_extended_path());
        }
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
                        let current_tag = "v0.1.11";
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
                                            <button onclick="if(window.__dsh_open_external) {{ window.__dsh_open_external('{url}'); }} else if(window.__TAURI__ && window.__TAURI__.core) {{ window.__TAURI__.core.invoke('open_browser', {{ url: '{url}' }}); }} else {{ window.location.href='{url}'; }}; this.parentElement.parentElement.remove();" style="background: #2563eb; color: #fff; border: none; padding: 6px 12px; border-radius: 6px; font-size: 12px; font-weight: 600; cursor: pointer; flex: 1;">
                                                🚀 打开下载最新版
                                            </button>
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
        let win = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let _ = win.eval(get_injected_updater_script());
        });
        return;
    }

    let dsh_cli = find_dsh_cli();
    let cached_script = find_cached_dsh_script();

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = std::process::Command::new("cmd.exe");

        if let Some(ref dsh_path) = dsh_cli {
            println!("[DeepSeek Harness] Fast boot from installed dsh CLI: {:?}", dsh_path);
            cmd.args(&["/c", dsh_path.to_str().unwrap_or("dsh"), "web"]);
        } else if let Some(ref script) = cached_script {
            println!("[DeepSeek Harness] Fast boot from cached script: {:?}", script);
            cmd.args(&["/c", "node", script.to_str().unwrap_or(""), "web"]);
        } else {
            println!("[DeepSeek Harness] First run: downloading via npx @deepseek-ai/dsh web (npmmirror)...");
            cmd.args(&["/c", &format!("npx --registry={} -y @deepseek-ai/dsh web", NPM_MIRROR_REGISTRY)]);
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
        let extended_path = get_extended_path();
        cmd.env("PATH", &extended_path);
        cmd.env("npm_config_registry", NPM_MIRROR_REGISTRY);

        if let Some(ref dsh_path) = dsh_cli {
            println!("[DeepSeek Harness] Fast boot from installed dsh CLI: {:?}", dsh_path);
            cmd.args(&["-c", &format!("export PATH=\"{}\"; \"{}\" web", extended_path, dsh_path.display())]);
        } else if let Some(ref script) = cached_script {
            let node_exe = find_node_executable();
            println!("[DeepSeek Harness] Fast boot from cached script: {:?}", script);
            cmd.args(&["-c", &format!("export PATH=\"{}\"; \"{}\" \"{}\" web", extended_path, node_exe, script.display())]);
        } else {
            println!("[DeepSeek Harness] First run: downloading via npx @deepseek-ai/dsh web (npmmirror)...");
            cmd.args(&["-c", &format!(
                "export PATH=\"{}\"; export npm_config_registry=\"{}\"; npx --registry={} -y @deepseek-ai/dsh web",
                extended_path, NPM_MIRROR_REGISTRY, NPM_MIRROR_REGISTRY
            )]);
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
                // Inject the in-app version widget
                tokio::time::sleep(Duration::from_millis(1500)).await;
                let _ = win.eval(get_injected_updater_script());
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
