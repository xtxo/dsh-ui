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

pub fn start_backend_service_if_needed(_app_handle: &AppHandle, window: WebviewWindow) {
    let target_port: u16 = 3080;

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
