#[cfg(target_os = "windows")]
use std::io::{BufRead, BufReader};
#[cfg(target_os = "windows")]
use std::net::TcpStream;
#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
#[cfg(target_os = "windows")]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
#[cfg(target_os = "windows")]
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::io::{Read, Write};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
use tauri::{Url, WebviewWindow};

pub use super::legacy_backend::{
    find_dsh_cli, get_injected_updater_script, is_backend_running, perform_update_check,
};

#[cfg(not(target_os = "windows"))]
pub use super::legacy_backend::{
    cleanup_backend, find_node_executable, get_extended_path, start_backend_service_if_needed,
};

#[cfg(target_os = "windows")]
const NPM_MIRROR_REGISTRY: &str = "https://registry.npmmirror.com";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(target_os = "windows")]
static BACKEND_CHILD: Mutex<Option<Child>> = Mutex::new(None);

#[cfg(target_os = "windows")]
fn find_cached_dsh_script() -> Option<PathBuf> {
    // Legacy self-contained Portable bundle.
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir
                .join("server")
                .join("node_modules")
                .join("@deepseek-ai")
                .join("dsh")
                .join("lib")
                .join("bin.js");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // npx cache used by the lightweight build.
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        let npx_dir = PathBuf::from(local_appdata).join("npm-cache").join("_npx");
        if let Ok(entries) = std::fs::read_dir(npx_dir) {
            // Prefer the most recently modified cache. The in-app updater keeps all
            // DSH caches on the same version, and mtime gives deterministic preference
            // to the cache touched by the latest npm/npx operation.
            let mut candidates = Vec::new();
            for entry in entries.flatten() {
                let candidate = entry
                    .path()
                    .join("node_modules")
                    .join("@deepseek-ai")
                    .join("dsh")
                    .join("lib")
                    .join("bin.js");
                if candidate.exists() {
                    let modified = candidate
                        .metadata()
                        .and_then(|meta| meta.modified())
                        .ok();
                    candidates.push((modified, candidate));
                }
            }
            candidates.sort_by(|a, b| a.0.cmp(&b.0));
            if let Some((_, path)) = candidates.pop() {
                return Some(path);
            }
        }
    }

    // System/global npm package.
    if let Ok(appdata) = std::env::var("APPDATA") {
        let candidate = PathBuf::from(appdata)
            .join("npm")
            .join("node_modules")
            .join("@deepseek-ai")
            .join("dsh")
            .join("lib")
            .join("bin.js");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn authenticated_dsh_url(line: &str) -> Option<Url> {
    let marker = "dsh web:";
    let at = line.find(marker)?;
    let candidate = line[at + marker.len()..]
        .trim_start()
        .split_whitespace()
        .next()?;
    let url = Url::parse(candidate).ok()?;

    // Modern DSH may run on 3080 or an OS-assigned fallback port. Only accept
    // explicit loopback HTTP URLs carrying the per-process launch token.
    let is_loopback = url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port().is_some();
    let has_token = url
        .query_pairs()
        .any(|(key, value)| key == "token" && !value.is_empty());

    (is_loopback && has_token).then_some(url)
}

#[cfg(target_os = "windows")]
fn watch_stdout(stdout: ChildStdout, window: WebviewWindow, auth_seen: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Some(url) = authenticated_dsh_url(&line) else {
                continue;
            };

            // Never print the authenticated URL: the query token is a credential.
            auth_seen.store(true, Ordering::Release);
            println!("[DeepSeek Harness] Captured authenticated Web URL; navigating embedded WebView.");
            let _ = window.navigate(url);
            std::thread::sleep(Duration::from_millis(1500));
            let _ = window.eval(get_injected_updater_script());
            break;
        }
    });
}

#[cfg(target_os = "windows")]
fn drain_stderr(stderr: ChildStderr) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut sink = std::io::sink();
        let _ = std::io::copy(&mut reader, &mut sink);
    });
}

#[cfg(target_os = "windows")]
fn plain_root_is_public() -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(
        &"127.0.0.1:3080".parse().expect("static loopback address"),
        Duration::from_millis(500),
    ) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(700)));
    if stream
        .write_all(b"HEAD / HTTP/1.1\r\nHost: 127.0.0.1:3080\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }

    let mut buf = [0_u8; 256];
    let Ok(size) = stream.read(&mut buf) else {
        return false;
    };
    let head = String::from_utf8_lossy(&buf[..size]);
    let status = head.lines().next().unwrap_or_default();
    status.contains(" 200 ") || status.contains(" 204 ") || status.contains(" 301 ")
        || status.contains(" 302 ") || status.contains(" 303 ") || status.contains(" 307 ")
        || status.contains(" 308 ")
}

#[cfg(target_os = "windows")]
fn webview_has_auth_cookie(window: &WebviewWindow) -> bool {
    let Ok(url) = Url::parse("http://127.0.0.1:3080/") else {
        return false;
    };
    window
        .cookies_for_url(url)
        .map(|cookies| {
            cookies
                .iter()
                .any(|cookie| cookie.name().starts_with("dsh-auth-"))
        })
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn spawn_backend(mut command: Command, window: WebviewWindow, auth_seen: Arc<AtomicBool>) {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW);

    match command.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            println!("[DeepSeek Harness] Backend process spawned with PID: {pid}");

            if let Ok(mut lock) = BACKEND_CHILD.lock() {
                *lock = Some(child);
            }
            if let Some(stdout) = stdout {
                watch_stdout(stdout, window, auth_seen);
            }
            if let Some(stderr) = stderr {
                drain_stderr(stderr);
            }
        }
        Err(error) => {
            eprintln!("[DeepSeek Harness] Failed to start backend service: {error}");
        }
    }
}

#[cfg(target_os = "windows")]
pub fn start_backend_service_if_needed(
    _app_handle: &tauri::AppHandle,
    window: WebviewWindow,
) {
    const TARGET_PORT: u16 = 3080;

    perform_update_check(&window, false);

    let mut use_ephemeral_port = false;
    if is_backend_running(TARGET_PORT) {
        println!("[DeepSeek Harness] Backend is already running on port {TARGET_PORT}");

        // A DSH process started by somebody else has a per-process launch token
        // that cannot be reconstructed by this client. Reuse it only when this
        // WebView already owns a valid cookie (or when talking to a legacy public
        // root that predates browser-token authentication).
        if webview_has_auth_cookie(&window) || plain_root_is_public() {
            if let Ok(url) = Url::parse("http://127.0.0.1:3080/") {
                let _ = window.navigate(url);
            }
            let win = window.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(1200)).await;
                let _ = win.eval(get_injected_updater_script());
            });
            return;
        }

        // Do not kill a user's independently started dsh process. Start an isolated
        // DSH-UI-owned instance on an OS-assigned loopback port and authenticate the
        // embedded WebView with that process's own launch token.
        eprintln!(
            "[DeepSeek Harness] Port 3080 is occupied by an authenticated dsh process without a reusable WebView cookie; starting an isolated DSH-UI backend on an OS-assigned port."
        );
        use_ephemeral_port = true;
    }

    let dsh_cli = find_dsh_cli();
    let cached_script = find_cached_dsh_script();
    let auth_seen = Arc::new(AtomicBool::new(false));
    let mut command = Command::new("cmd.exe");
    command.arg("/d").arg("/c");

    if let Some(ref dsh_path) = dsh_cli {
        println!("[DeepSeek Harness] Fast boot from installed dsh CLI: {:?}", dsh_path);
        command
            .arg(dsh_path.to_str().unwrap_or("dsh"))
            .args(["web", "--no-open"]);
        if use_ephemeral_port {
            command.args(["--port", "0"]);
        }
    } else if let Some(ref script) = cached_script {
        println!("[DeepSeek Harness] Fast boot from cached script: {:?}", script);
        command
            .arg("node")
            .arg(script.to_str().unwrap_or_default())
            .args(["web", "--no-open"]);
        if use_ephemeral_port {
            command.args(["--port", "0"]);
        }
    } else {
        println!("[DeepSeek Harness] First run: downloading via npx @deepseek-ai/dsh web...");
        let port_arg = if use_ephemeral_port { " --port 0" } else { "" };
        command.arg(format!(
            "npx --registry={NPM_MIRROR_REGISTRY} -y @deepseek-ai/dsh web --no-open{port_arg}"
        ));
    }

    spawn_backend(command, window.clone(), auth_seen.clone());

    // Readiness fallback. Modern DSH is authenticated: wait for stdout's token
    // URL and never replace it with a naked / request (which is a guaranteed 401).
    // For the normal 3080 path, legacy DSH versions can still be opened after the
    // port becomes ready. When 3080 belongs to another authenticated process and
    // DSH-UI uses --port 0, only the captured token URL identifies our actual port.
    tauri::async_runtime::spawn(async move {
        for attempt in 1..=2400 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if auth_seen.load(Ordering::Acquire) {
                return;
            }

            if use_ephemeral_port {
                continue;
            }
            if !is_backend_running(TARGET_PORT) {
                continue;
            }

            if plain_root_is_public() {
                println!(
                    "[DeepSeek Harness] Legacy/public backend ready after {attempt} attempts; navigating WebView."
                );
                if let Ok(url) = Url::parse("http://127.0.0.1:3080/") {
                    let _ = window.navigate(url);
                }
                tokio::time::sleep(Duration::from_millis(1200)).await;
                let _ = window.eval(get_injected_updater_script());
                return;
            }

            // Authenticated DSH may bind the port before its Loader tree settles
            // and prints the launch URL. Keep waiting for stdout instead of causing
            // the Chromium/WebView2 HTTP ERROR 401 page.
        }

        eprintln!(
            "[DeepSeek Harness] Backend did not provide an authenticated launch URL within 10 minutes."
        );
    });
}

#[cfg(target_os = "windows")]
pub fn cleanup_backend() {
    if let Ok(mut lock) = BACKEND_CHILD.lock() {
        if let Some(mut child) = lock.take() {
            println!("[DeepSeek Harness] Stopping backend service (PID {})...", child.id());
            let mut kill_cmd = Command::new("taskkill");
            kill_cmd
                .args(["/F", "/T", "/PID", &child.id().to_string()])
                .creation_flags(CREATE_NO_WINDOW);
            let _ = kill_cmd.output();
            let _ = child.kill();
        }
    }
}
