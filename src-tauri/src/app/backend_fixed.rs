#[cfg(target_os = "windows")]
use std::fs::{self, OpenOptions};
#[cfg(target_os = "windows")]
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(target_os = "windows")]
use std::net::TcpStream;
#[cfg(target_os = "windows")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use std::process::{Child, Command, Stdio};
#[cfg(target_os = "windows")]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
#[cfg(target_os = "windows")]
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
const TARGET_PORT: u16 = 3080;
#[cfg(target_os = "windows")]
static BACKEND_CHILD: Mutex<Option<Child>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Persistent diagnostics + process ownership
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn state_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("dsh-ui")
}

#[cfg(target_os = "windows")]
fn log_path() -> PathBuf {
    state_dir().join("logs").join("backend.log")
}

#[cfg(target_os = "windows")]
fn pid_path() -> PathBuf {
    state_dir().join("backend.pid")
}

#[cfg(target_os = "windows")]
fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(target_os = "windows")]
fn init_backend_log() {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Keep one small previous log so repeated startup failures do not grow forever.
    if path.metadata().map(|m| m.len() > 2_000_000).unwrap_or(false) {
        let old = path.with_extension("log.1");
        let _ = fs::remove_file(&old);
        let _ = fs::rename(&path, old);
    }
}

#[cfg(target_os = "windows")]
fn safe_log_line(line: &str) -> String {
    // A dsh launch token is a credential. Never persist a line that contains one.
    if line.to_ascii_lowercase().contains("token=") {
        return "[redacted: line contained an authentication token]".to_string();
    }
    line.to_string()
}

#[cfg(target_os = "windows")]
fn log_event(message: impl AsRef<str>) {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{}] {}", unix_time(), safe_log_line(message.as_ref()));
    }
}

#[cfg(target_os = "windows")]
fn write_owned_pid(pid: u32) {
    let path = pid_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(error) = fs::write(&path, pid.to_string()) {
        log_event(format!("pid: failed to persist backend PID {pid}: {error}"));
    } else {
        log_event(format!("pid: persisted backend wrapper PID {pid}"));
    }
}

#[cfg(target_os = "windows")]
fn read_owned_pid() -> Option<u32> {
    fs::read_to_string(pid_path())
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
}

#[cfg(target_os = "windows")]
fn clear_owned_pid() {
    let _ = fs::remove_file(pid_path());
}

#[cfg(target_os = "windows")]
fn hidden_output(program: &str, args: &[&str]) -> Option<std::process::Output> {
    let mut command = Command::new(program);
    command.args(args).creation_flags(CREATE_NO_WINDOW);
    command.output().ok()
}

#[cfg(target_os = "windows")]
fn process_command_line(pid: u32) -> Option<String> {
    let script = format!(
        "$p=Get-CimInstance Win32_Process -Filter \"ProcessId = {pid}\" -ErrorAction SilentlyContinue; if($p){{[Console]::Out.Write($p.CommandLine)}}"
    );
    let output = hidden_output(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    )?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(target_os = "windows")]
fn looks_like_dsh_process(command_line: &str) -> bool {
    let line = command_line.to_ascii_lowercase();
    let mentions_dsh = line.contains("@deepseek-ai")
        || line.contains("deepseek-ai\\dsh")
        || line.contains("deepseek-ai/dsh")
        || line.contains("dsh.cmd")
        || line.contains(" dsh ");
    let looks_like_web = line.contains(" web") || line.contains("bin.js");
    mentions_dsh && looks_like_web
}

#[cfg(target_os = "windows")]
fn listener_pid(port: u16) -> Option<u32> {
    let output = hidden_output("netstat.exe", &["-ano", "-p", "tcp"])?;
    if !output.status.success() {
        return None;
    }

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 || !fields[0].eq_ignore_ascii_case("TCP") {
            continue;
        }
        let local = fields[1];
        let state = fields[3];
        if !state.eq_ignore_ascii_case("LISTENING") || !local.ends_with(&format!(":{port}")) {
            continue;
        }
        if let Ok(pid) = fields[4].parse::<u32>() {
            return Some(pid);
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn kill_process_tree(pid: u32, reason: &str) -> bool {
    log_event(format!("process: killing PID {pid} tree ({reason})"));
    let output = hidden_output("taskkill.exe", &["/F", "/T", "/PID", &pid.to_string()]);
    match output {
        Some(output) if output.status.success() => {
            log_event(format!("process: PID {pid} tree terminated"));
            true
        }
        Some(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let details = if !stderr.is_empty() { stderr } else { stdout };
            log_event(format!("process: taskkill PID {pid} failed: {details}"));
            false
        }
        None => {
            log_event(format!("process: failed to launch taskkill for PID {pid}"));
            false
        }
    }
}

#[cfg(target_os = "windows")]
fn wait_for_port_release(port: u16) -> bool {
    for _ in 0..30 {
        if listener_pid(port).is_none() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    listener_pid(port).is_none()
}

#[cfg(target_os = "windows")]
fn cleanup_stale_backend_before_start() -> bool {
    // First clean the exact wrapper PID persisted by a previous DSH-UI session.
    if let Some(pid) = read_owned_pid() {
        match process_command_line(pid) {
            Some(command_line) if looks_like_dsh_process(&command_line) => {
                log_event(format!("startup: found stale owned backend PID {pid}"));
                let _ = kill_process_tree(pid, "stale DSH-UI backend from previous session");
            }
            Some(_) => {
                // The PID may have been recycled by Windows; never kill an unrelated process.
                log_event(format!(
                    "startup: persisted PID {pid} no longer looks like DSH; treating PID file as stale"
                ));
            }
            None => log_event(format!("startup: persisted PID {pid} is no longer running")),
        }
        clear_owned_pid();
    }

    // Then inspect the canonical DSH port. If the listener is definitely dsh web,
    // clean it so every DSH-UI launch owns a fresh process/token pair.
    if let Some(pid) = listener_pid(TARGET_PORT) {
        match process_command_line(pid) {
            Some(command_line) if looks_like_dsh_process(&command_line) => {
                log_event(format!(
                    "startup: port {TARGET_PORT} is held by DSH PID {pid}; terminating stale instance"
                ));
                let _ = kill_process_tree(pid, "DSH listener found before DSH-UI startup");
                if wait_for_port_release(TARGET_PORT) {
                    log_event(format!("startup: port {TARGET_PORT} released"));
                    return false;
                }
                log_event(format!(
                    "startup: port {TARGET_PORT} did not release in time; using OS-assigned port"
                ));
                return true;
            }
            Some(_) => {
                log_event(format!(
                    "startup: port {TARGET_PORT} belongs to non-DSH PID {pid}; leaving it untouched and using OS-assigned port"
                ));
                return true;
            }
            None => {
                log_event(format!(
                    "startup: port {TARGET_PORT} is listening but owner command line could not be verified; leaving it untouched"
                ));
                return true;
            }
        }
    }

    log_event(format!("startup: port {TARGET_PORT} is free"));
    false
}

// ---------------------------------------------------------------------------
// DSH discovery and authenticated Web launch
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn find_cached_dsh_script() -> Option<PathBuf> {
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

    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        let npx_dir = PathBuf::from(local_appdata).join("npm-cache").join("_npx");
        if let Ok(entries) = fs::read_dir(npx_dir) {
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
                    let modified = candidate.metadata().and_then(|m| m.modified()).ok();
                    candidates.push((modified, candidate));
                }
            }
            candidates.sort_by(|a, b| a.0.cmp(&b.0));
            if let Some((_, path)) = candidates.pop() {
                return Some(path);
            }
        }
    }

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
fn windows_node_executable() -> PathBuf {
    let mut candidates = Vec::new();

    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join("node.exe"));
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("runtime").join("node").join("node.exe"));
        }
    }

    if let Some(local_appdata) = std::env::var_os("LOCALAPPDATA") {
        let runtime_root = PathBuf::from(local_appdata).join("dsh-ui").join("runtime");
        if let Ok(entries) = fs::read_dir(runtime_root) {
            for entry in entries.flatten() {
                candidates.push(entry.path().join("node.exe"));
            }
        }
    }

    candidates.push(PathBuf::from(r"C:\Program Files\nodejs\node.exe"));

    candidates
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from("node"))
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

    let is_loopback = url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port().is_some();
    let has_token = url
        .query_pairs()
        .any(|(key, value)| key == "token" && !value.is_empty());

    (is_loopback && has_token).then_some(url)
}

#[cfg(target_os = "windows")]
fn watch_backend_output<R: Read + Send + 'static>(
    reader: R,
    source: &'static str,
    window: WebviewWindow,
    auth_seen: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            if let Some(url) = authenticated_dsh_url(&line) {
                let port = url.port().unwrap_or(TARGET_PORT);
                log_event(format!(
                    "{source}: captured authenticated DSH launch URL on port {port}; token redacted"
                ));
                if !auth_seen.swap(true, Ordering::AcqRel) {
                    let _ = window.navigate(url);
                    std::thread::sleep(Duration::from_millis(1200));
                    let _ = window.eval(get_injected_updater_script());
                }
                continue;
            }

            let clean = safe_log_line(&line);
            if !clean.trim().is_empty() {
                log_event(format!("{source}: {clean}"));
            }
        }
        log_event(format!("{source}: stream closed"));
    });
}

#[cfg(target_os = "windows")]
fn plain_root_is_public(port: u16) -> bool {
    let address = format!("127.0.0.1:{port}");
    let Ok(socket) = address.parse() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&socket, Duration::from_millis(500)) else {
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
    status.contains(" 200 ") || status.contains(" 204 ") || status.contains(" 301 ")
        || status.contains(" 302 ") || status.contains(" 303 ") || status.contains(" 307 ")
        || status.contains(" 308 ")
}

#[cfg(target_os = "windows")]
fn monitor_backend_exit(pid: u32) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let mut finished = None;
        if let Ok(mut lock) = BACKEND_CHILD.lock() {
            if let Some(child) = lock.as_mut() {
                if child.id() != pid {
                    return;
                }
                match child.try_wait() {
                    Ok(Some(status)) => finished = Some(format!("{status}")),
                    Ok(None) => {}
                    Err(error) => {
                        log_event(format!("process: failed to poll backend PID {pid}: {error}"));
                        return;
                    }
                }
            } else {
                return;
            }

            if finished.is_some() {
                *lock = None;
            }
        }

        if let Some(status) = finished {
            log_event(format!("process: backend wrapper PID {pid} exited with {status}"));
            clear_owned_pid();
            return;
        }
    });
}

#[cfg(target_os = "windows")]
fn spawn_backend(mut command: Command, window: WebviewWindow, auth_seen: Arc<AtomicBool>) -> Option<u32> {
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
            log_event(format!("process: backend wrapper spawned PID {pid}"));
            write_owned_pid(pid);

            if let Ok(mut lock) = BACKEND_CHILD.lock() {
                *lock = Some(child);
            }
            if let Some(stdout) = stdout {
                watch_backend_output(stdout, "stdout", window.clone(), auth_seen.clone());
            }
            if let Some(stderr) = stderr {
                watch_backend_output(stderr, "stderr", window, auth_seen);
            }
            monitor_backend_exit(pid);
            Some(pid)
        }
        Err(error) => {
            eprintln!("[DeepSeek Harness] Failed to start backend service: {error}");
            log_event(format!("process: failed to spawn backend: {error}"));
            None
        }
    }
}

#[cfg(target_os = "windows")]
pub fn start_backend_service_if_needed(
    _app_handle: &tauri::AppHandle,
    window: WebviewWindow,
) {
    init_backend_log();
    log_event(format!(
        "session: DSH-UI backend startup begins; executable={}",
        std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    ));

    perform_update_check(&window, false);

    // DSH-UI owns exactly one backend per app session. Clean stale owned DSH first.
    // If 3080 belongs to an unrelated program, leave it alone and use --port 0.
    let use_ephemeral_port = cleanup_stale_backend_before_start();

    let dsh_cli = find_dsh_cli();
    let cached_script = find_cached_dsh_script();
    let auth_seen = Arc::new(AtomicBool::new(false));
    let mut command = Command::new("cmd.exe");
    command.arg("/d").arg("/c");

    if let Some(ref dsh_path) = dsh_cli {
        log_event(format!("launch: using installed DSH CLI at {}", dsh_path.display()));
        command
            .arg(dsh_path.to_str().unwrap_or("dsh"))
            .args(["web", "--no-open"]);
        if use_ephemeral_port {
            command.args(["--port", "0"]);
        }
    } else if let Some(ref script) = cached_script {
        let node = windows_node_executable();
        log_event(format!(
            "launch: using cached DSH script {} with node {}",
            script.display(),
            node.display()
        ));
        command
            .arg(node.to_string_lossy().as_ref())
            .arg(script.to_str().unwrap_or_default())
            .args(["web", "--no-open"]);
        if use_ephemeral_port {
            command.args(["--port", "0"]);
        }
    } else {
        log_event("launch: no installed/cached DSH found; bootstrapping with npx");
        let port_arg = if use_ephemeral_port { " --port 0" } else { "" };
        command.arg(format!(
            "npx --registry={NPM_MIRROR_REGISTRY} -y @deepseek-ai/dsh web --no-open{port_arg}"
        ));
    }

    let pid = spawn_backend(command, window.clone(), auth_seen.clone());
    if pid.is_none() {
        log_event("startup: backend spawn failed; WebView will remain unavailable");
        return;
    }

    // Modern DSH prints a token-bearing authenticated URL. Wait for that exact URL.
    // Legacy DSH without token auth may still expose a public 3080 root.
    tauri::async_runtime::spawn(async move {
        for attempt in 1..=2400 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if auth_seen.load(Ordering::Acquire) {
                log_event(format!("startup: authenticated URL observed after {attempt} checks"));
                return;
            }

            if use_ephemeral_port {
                continue;
            }
            if !is_backend_running(TARGET_PORT) {
                continue;
            }

            if plain_root_is_public(TARGET_PORT) {
                log_event(format!(
                    "startup: legacy/public backend became ready on {TARGET_PORT} after {attempt} checks"
                ));
                if let Ok(url) = Url::parse("http://127.0.0.1:3080/") {
                    let _ = window.navigate(url);
                }
                tokio::time::sleep(Duration::from_millis(1200)).await;
                let _ = window.eval(get_injected_updater_script());
                return;
            }
        }

        log_event("startup: timed out waiting 10 minutes for authenticated DSH launch URL");
        eprintln!(
            "[DeepSeek Harness] Backend did not provide an authenticated launch URL within 10 minutes."
        );
    });
}

#[cfg(target_os = "windows")]
pub fn cleanup_backend() {
    log_event("shutdown: DSH-UI backend cleanup begins");

    let mut owned_pid = None;
    if let Ok(mut lock) = BACKEND_CHILD.lock() {
        if let Some(mut child) = lock.take() {
            let pid = child.id();
            owned_pid = Some(pid);
            match child.try_wait() {
                Ok(Some(status)) => {
                    log_event(format!("shutdown: backend PID {pid} already exited with {status}"));
                }
                _ => {
                    let _ = kill_process_tree(pid, "DSH-UI application shutdown");
                    let _ = child.kill();
                }
            }
        }
    }

    // If the in-memory handle disappeared (crash/reparenting), use the persisted PID.
    if owned_pid.is_none() {
        if let Some(pid) = read_owned_pid() {
            if let Some(command_line) = process_command_line(pid) {
                if looks_like_dsh_process(&command_line) {
                    let _ = kill_process_tree(pid, "persisted DSH-UI backend on shutdown");
                } else {
                    log_event(format!(
                        "shutdown: persisted PID {pid} was recycled/non-DSH; not killing it"
                    ));
                }
            }
        }
    }
    clear_owned_pid();

    // Last safety net for an orphaned node child whose cmd.exe wrapper disappeared.
    if let Some(pid) = listener_pid(TARGET_PORT) {
        if owned_pid != Some(pid) {
            match process_command_line(pid) {
                Some(command_line) if looks_like_dsh_process(&command_line) => {
                    let _ = kill_process_tree(pid, "orphaned DSH listener on shutdown");
                }
                Some(_) => log_event(format!(
                    "shutdown: port {TARGET_PORT} is held by non-DSH PID {pid}; leaving it untouched"
                )),
                None => log_event(format!(
                    "shutdown: could not verify owner of port {TARGET_PORT} PID {pid}; leaving it untouched"
                )),
            }
        }
    }

    log_event("shutdown: cleanup complete");
}
