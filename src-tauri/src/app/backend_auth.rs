#[cfg(target_os = "windows")]
pub use super::windows_backend::*;

#[cfg(not(target_os = "windows"))]
mod unix {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };
    use std::time::Duration;

    use tauri::{Url, WebviewWindow};

    pub use super::super::legacy_backend::{
        find_dsh_cli, find_node_executable, get_extended_path, get_injected_updater_script,
        is_backend_running, perform_update_check,
    };

    const NPM_MIRROR_REGISTRY: &str = "https://registry.npmmirror.com";
    static BACKEND_CHILD: Mutex<Option<Child>> = Mutex::new(None);

    fn find_cached_dsh_script() -> Option<PathBuf> {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let candidates = [
                    exe_dir
                        .join("server")
                        .join("node_modules")
                        .join("@deepseek-ai")
                        .join("dsh")
                        .join("lib")
                        .join("bin.js"),
                    exe_dir.join("../Resources/server/node_modules/@deepseek-ai/dsh/lib/bin.js"),
                ];
                for candidate in candidates {
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
        }

        let direct_candidates = [
            PathBuf::from("/opt/homebrew/lib/node_modules/@deepseek-ai/dsh/lib/bin.js"),
            PathBuf::from("/usr/local/lib/node_modules/@deepseek-ai/dsh/lib/bin.js"),
            PathBuf::from("/usr/lib/node_modules/@deepseek-ai/dsh/lib/bin.js"),
        ];
        for candidate in direct_candidates {
            if candidate.exists() {
                return Some(candidate);
            }
        }

        if let Ok(home) = std::env::var("HOME") {
            let home = PathBuf::from(home);
            let mut candidates = Vec::new();

            let npx_dir = home.join(".npm").join("_npx");
            if let Ok(entries) = std::fs::read_dir(npx_dir) {
                for entry in entries.flatten() {
                    candidates.push(
                        entry
                            .path()
                            .join("node_modules")
                            .join("@deepseek-ai")
                            .join("dsh")
                            .join("lib")
                            .join("bin.js"),
                    );
                }
            }

            let nvm_dir = home.join(".nvm").join("versions").join("node");
            if let Ok(entries) = std::fs::read_dir(nvm_dir) {
                for entry in entries.flatten() {
                    candidates.push(
                        entry
                            .path()
                            .join("lib")
                            .join("node_modules")
                            .join("@deepseek-ai")
                            .join("dsh")
                            .join("lib")
                            .join("bin.js"),
                    );
                }
            }

            let pnpm_dir = home.join(".local").join("share").join("pnpm").join("global");
            if let Ok(entries) = std::fs::read_dir(pnpm_dir) {
                for entry in entries.flatten() {
                    candidates.push(
                        entry
                            .path()
                            .join("node_modules")
                            .join("@deepseek-ai")
                            .join("dsh")
                            .join("lib")
                            .join("bin.js"),
                    );
                }
            }

            let mut existing = candidates
                .into_iter()
                .filter(|path| path.exists())
                .map(|path| {
                    let modified = path.metadata().and_then(|meta| meta.modified()).ok();
                    (modified, path)
                })
                .collect::<Vec<_>>();
            existing.sort_by(|a, b| a.0.cmp(&b.0));
            if let Some((_, path)) = existing.pop() {
                return Some(path);
            }
        }

        None
    }

    fn authenticated_dsh_url(line: &str) -> Option<Url> {
        let marker = "dsh web:";
        let at = line.find(marker)?;
        let candidate = line[at + marker.len()..]
            .trim_start()
            .split_whitespace()
            .next()?;
        let url = Url::parse(candidate).ok()?;
        let is_loopback = url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port().is_some();
        let has_token = url
            .query_pairs()
            .any(|(key, value)| key == "token" && !value.is_empty());
        (is_loopback && has_token).then_some(url)
    }

    fn watch_stream<R>(reader: R, window: WebviewWindow, auth_seen: Arc<AtomicBool>)
    where
        R: Read + Send + 'static,
    {
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                if auth_seen.load(Ordering::Acquire) {
                    continue;
                }
                let Some(url) = authenticated_dsh_url(&line) else {
                    continue;
                };
                if auth_seen.swap(true, Ordering::AcqRel) {
                    continue;
                }

                println!(
                    "[DeepSeek Harness] Captured authenticated Web URL; navigating embedded WebView."
                );
                let _ = window.navigate(url);
                std::thread::sleep(Duration::from_millis(1500));
                let _ = window.eval(get_injected_updater_script());
            }
        });
    }

    fn plain_root_is_public(port: u16) -> bool {
        let address = format!("127.0.0.1:{port}");
        let Ok(socket_addr) = address.parse() else {
            return false;
        };
        let Ok(mut stream) = TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500)) else {
            return false;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(700)));
        let request = format!(
            "HEAD / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        );
        if stream.write_all(request.as_bytes()).is_err() {
            return false;
        }
        let mut buf = [0_u8; 256];
        let Ok(size) = stream.read(&mut buf) else {
            return false;
        };
        let head = String::from_utf8_lossy(&buf[..size]);
        let status = head.lines().next().unwrap_or_default();
        [" 200 ", " 204 ", " 301 ", " 302 ", " 303 ", " 307 ", " 308 "]
            .iter()
            .any(|code| status.contains(code))
    }

    fn spawn_backend(mut command: Command, window: WebviewWindow, auth_seen: Arc<AtomicBool>) {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PATH", get_extended_path())
            .env("npm_config_registry", NPM_MIRROR_REGISTRY);

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
                    watch_stream(stdout, window.clone(), auth_seen.clone());
                }
                if let Some(stderr) = stderr {
                    // Some Node launchers write readiness information to stderr.
                    // Parse both streams, while never echoing the token credential.
                    watch_stream(stderr, window, auth_seen);
                }
            }
            Err(error) => {
                eprintln!("[DeepSeek Harness] Failed to start backend service: {error}");
            }
        }
    }

    pub fn start_backend_service_if_needed(
        _app_handle: &tauri::AppHandle,
        window: WebviewWindow,
    ) {
        const TARGET_PORT: u16 = 3080;

        perform_update_check(&window, false);

        // Never attach a fresh WebView to a naked authenticated root. If 3080 is
        // occupied, leave that process alone and launch a private DSH instance on
        // an OS-assigned loopback port. Its printed token URL is the only authority.
        let use_ephemeral_port = is_backend_running(TARGET_PORT);
        if use_ephemeral_port {
            eprintln!(
                "[DeepSeek Harness] Port 3080 is occupied; starting an isolated authenticated DSH-UI backend on an OS-assigned port."
            );
        }

        let dsh_cli = find_dsh_cli();
        let cached_script = find_cached_dsh_script();
        let auth_seen = Arc::new(AtomicBool::new(false));

        let mut command = if let Some(ref dsh_path) = dsh_cli {
            println!("[DeepSeek Harness] Fast boot from installed dsh CLI: {:?}", dsh_path);
            let mut command = Command::new(dsh_path);
            command.args(["web", "--no-open"]);
            command
        } else if let Some(ref script) = cached_script {
            println!("[DeepSeek Harness] Fast boot from cached script: {:?}", script);
            let mut command = Command::new(find_node_executable());
            command.arg(script).args(["web", "--no-open"]);
            command
        } else {
            println!("[DeepSeek Harness] First run: downloading via npx @deepseek-ai/dsh web...");
            let mut command = Command::new("npx");
            command
                .arg(format!("--registry={NPM_MIRROR_REGISTRY}"))
                .args(["-y", "@deepseek-ai/dsh", "web", "--no-open"]);
            command
        };

        if use_ephemeral_port {
            command.args(["--port", "0"]);
        }

        spawn_backend(command, window.clone(), auth_seen.clone());

        tauri::async_runtime::spawn(async move {
            for attempt in 1..=2400 {
                tokio::time::sleep(Duration::from_millis(250)).await;
                if auth_seen.load(Ordering::Acquire) {
                    return;
                }
                if use_ephemeral_port || !is_backend_running(TARGET_PORT) {
                    continue;
                }

                // Compatibility only for old DSH releases that predate browser-token
                // authentication. Modern authenticated roots return 401 here and are
                // intentionally never opened without the printed token URL.
                if plain_root_is_public(TARGET_PORT) {
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
            }

            eprintln!(
                "[DeepSeek Harness] Backend did not provide an authenticated launch URL within 10 minutes."
            );
        });
    }

    pub fn cleanup_backend() {
        if let Ok(mut lock) = BACKEND_CHILD.lock() {
            if let Some(mut child) = lock.take() {
                println!("[DeepSeek Harness] Stopping backend service (PID {})...", child.id());
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub use unix::*;
