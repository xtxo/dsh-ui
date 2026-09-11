use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const ENGINE_REGISTRY: &str = "https://registry.npmjs.org";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, serde::Serialize)]
struct EngineUpdateProgress {
    percent: u8,
    stage: String,
    detail: String,
    done: bool,
    success: Option<bool>,
}

impl Default for EngineUpdateProgress {
    fn default() -> Self {
        Self {
            percent: 0,
            stage: "等待开始".to_string(),
            detail: "点击更新后将显示实时阶段进度".to_string(),
            done: false,
            success: None,
        }
    }
}

static ENGINE_UPDATE_PROGRESS: OnceLock<Mutex<EngineUpdateProgress>> = OnceLock::new();

fn progress_state() -> &'static Mutex<EngineUpdateProgress> {
    ENGINE_UPDATE_PROGRESS.get_or_init(|| Mutex::new(EngineUpdateProgress::default()))
}

fn set_progress(percent: u8, stage: &str, detail: impl Into<String>, done: bool, success: Option<bool>) {
    if let Ok(mut state) = progress_state().lock() {
        state.percent = percent.min(100);
        state.stage = stage.to_string();
        state.detail = detail.into();
        state.done = done;
        state.success = success;
    }
}

fn mark_failed(error: &str) {
    if let Ok(mut state) = progress_state().lock() {
        state.stage = "更新失败".to_string();
        state.detail = error.to_string();
        state.done = true;
        state.success = Some(false);
    }
}

pub fn progress_json() -> String {
    progress_state()
        .lock()
        .ok()
        .and_then(|state| serde_json::to_string(&*state).ok())
        .unwrap_or_else(|| {
            r#"{"percent":0,"stage":"状态读取失败","detail":"无法读取更新进度","done":true,"success":false}"#
                .to_string()
        })
}

fn read_engine_version(package_json: &Path) -> Option<String> {
    let text = fs::read_to_string(package_json).ok()?;
    let json = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    json.get("version")?.as_str().map(str::to_owned)
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

#[cfg(target_os = "windows")]
fn windows_node_runtime_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(path) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&path));
    }

    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            dirs.push(exe_dir.join("runtime").join("node"));
        }
    }

    if let Some(local_appdata) = env::var_os("LOCALAPPDATA") {
        let runtime_root = PathBuf::from(local_appdata).join("dsh-ui").join("runtime");
        if let Ok(entries) = fs::read_dir(&runtime_root) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push(entry.path());
                }
            }
        }
    }

    let mut seen = HashSet::new();
    dirs.retain(|dir| seen.insert(dir.clone()));
    dirs
}

#[cfg(target_os = "windows")]
fn npm_command() -> Result<Command, String> {
    for runtime_dir in windows_node_runtime_dirs() {
        let node = runtime_dir.join("node.exe");
        let npm_cli = runtime_dir
            .join("node_modules")
            .join("npm")
            .join("bin")
            .join("npm-cli.js");
        if node.exists() && npm_cli.exists() {
            let mut command = Command::new(node);
            command.arg(npm_cli);
            command.creation_flags(CREATE_NO_WINDOW);
            return Ok(command);
        }
    }

    Err(
        "找不到可用的 npm 运行时。请重启 DSH-UI 让首次启动环境检测完成；如仍失败，可安装 Node.js 22.19+ 或 24+ 后重试。"
            .to_string(),
    )
}

#[cfg(not(target_os = "windows"))]
fn npm_command() -> Result<Command, String> {
    let mut command = Command::new("npm");
    command.env("PATH", crate::app::backend::get_extended_path());
    Ok(command)
}

fn npx_engine_prefixes() -> Vec<PathBuf> {
    let mut prefixes = Vec::new();

    #[cfg(target_os = "windows")]
    if let Some(local_appdata) = env::var_os("LOCALAPPDATA") {
        let npx_root = PathBuf::from(local_appdata).join("npm-cache").join("_npx");
        if let Ok(entries) = fs::read_dir(npx_root) {
            for entry in entries.flatten() {
                let package_json = entry
                    .path()
                    .join("node_modules")
                    .join("@deepseek-ai")
                    .join("dsh")
                    .join("package.json");
                if package_json.exists() {
                    if let Some(prefix) = package_install_prefix(&package_json) {
                        prefixes.push(prefix);
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    if let Some(home) = env::var_os("HOME") {
        let npx_root = PathBuf::from(home).join(".npm").join("_npx");
        if let Ok(entries) = fs::read_dir(npx_root) {
            for entry in entries.flatten() {
                let package_json = entry
                    .path()
                    .join("node_modules")
                    .join("@deepseek-ai")
                    .join("dsh")
                    .join("package.json");
                if package_json.exists() {
                    if let Some(prefix) = package_install_prefix(&package_json) {
                        prefixes.push(prefix);
                    }
                }
            }
        }
    }

    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let package_json = exe_dir
                .join("server")
                .join("node_modules")
                .join("@deepseek-ai")
                .join("dsh")
                .join("package.json");
            if package_json.exists() {
                if let Some(prefix) = package_install_prefix(&package_json) {
                    prefixes.push(prefix);
                }
            }
        }
    }

    let mut seen = HashSet::new();
    prefixes.retain(|prefix| seen.insert(prefix.clone()));
    prefixes
}

fn global_engine_package_json() -> Option<PathBuf> {
    // Ask the same npm runtime used for updating so nvm/fnm/private Node prefixes
    // are resolved correctly instead of assuming a single global directory.
    if let Ok(mut command) = npm_command() {
        command.args(["root", "-g"]);
        if let Ok(output) = command.output() {
            if output.status.success() {
                let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !root.is_empty() {
                    let candidate = PathBuf::from(root)
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

    #[cfg(target_os = "windows")]
    if let Some(appdata) = env::var_os("APPDATA") {
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

    None
}

fn probe_npm_version() -> Result<String, String> {
    let mut command = npm_command()?;
    command.arg("--version");
    let output = command
        .output()
        .map_err(|error| format!("无法启动 npm: {error}"))?;
    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if details.is_empty() {
            "npm 运行环境不可用".to_string()
        } else {
            format!("npm 运行环境不可用：{details}")
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn install_latest_into_prefix(prefix: &Path) -> Result<String, String> {
    let mut command = npm_command()?;
    command
        .arg("install")
        .arg("--prefix")
        .arg(prefix)
        .args([
            "--omit=dev",
            "--no-audit",
            "--no-fund",
            "--registry",
            ENGINE_REGISTRY,
            "@deepseek-ai/dsh@latest",
        ]);

    let output = command
        .output()
        .map_err(|error| format!("无法启动 npm 更新内核: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if details.is_empty() {
            format!("@deepseek-ai/dsh 更新失败（目录 {}）", prefix.display())
        } else {
            format!(
                "@deepseek-ai/dsh 更新失败（目录 {}）：{}",
                prefix.display(),
                details
            )
        });
    }

    let package_json = prefix
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join("package.json");

    read_engine_version(&package_json).ok_or_else(|| {
        format!(
            "npm 已执行完成，但无法读取更新后的 @deepseek-ai/dsh 版本（{}）",
            package_json.display()
        )
    })
}

fn install_latest_global() -> Result<String, String> {
    let mut command = npm_command()?;
    command.args([
        "install",
        "-g",
        "--omit=dev",
        "--no-audit",
        "--no-fund",
        "--registry",
        ENGINE_REGISTRY,
        "@deepseek-ai/dsh@latest",
    ]);

    let output = command
        .output()
        .map_err(|error| format!("无法启动 npm 更新全局内核: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if details.is_empty() {
            "全局 @deepseek-ai/dsh 更新失败".to_string()
        } else {
            format!("全局 @deepseek-ai/dsh 更新失败：{details}")
        });
    }

    let package_json = global_engine_package_json().ok_or_else(|| {
        "npm 全局更新已执行完成，但无法定位更新后的 @deepseek-ai/dsh".to_string()
    })?;
    read_engine_version(&package_json).ok_or_else(|| {
        format!(
            "npm 全局更新已执行完成，但无法读取更新后的 @deepseek-ai/dsh 版本（{}）",
            package_json.display()
        )
    })
}

fn update_dsh_engine_inner() -> Result<String, String> {
    set_progress(5, "定位 npm 运行环境", "正在检查 Node.js 与 npm...", false, None);
    let npm_version = probe_npm_version()?;

    set_progress(
        15,
        "扫描本地内核",
        format!("npm {npm_version} 可用，正在查找 DeepSeek Harness 安装与缓存..."),
        false,
        None,
    );
    let prefixes = npx_engine_prefixes();
    let global_package = global_engine_package_json();
    let has_global = global_package.is_some();

    if prefixes.is_empty() && !has_global {
        return Err(
            "没有找到当前 DeepSeek Harness 安装或 npx/Portable 缓存。请先正常启动一次 DSH-UI，让内核完成首次下载后再更新。"
                .to_string(),
        );
    }

    let total = prefixes.len() + usize::from(has_global);
    set_progress(
        25,
        "准备更新",
        format!(
            "找到 {} 个内核目标{}，准备同步到最新版",
            total,
            if has_global { "（包含全局安装）" } else { "" }
        ),
        false,
        None,
    );

    let mut versions = Vec::new();
    let mut failures = Vec::new();
    let mut completed = 0usize;

    if has_global {
        let percent = 35 + ((completed * 50) / total.max(1)) as u8;
        set_progress(
            percent,
            "更新全局内核",
            "检测到 Windows/npm 全局 DeepSeek Harness，正在更新...",
            false,
            None,
        );
        match install_latest_global() {
            Ok(version) => versions.push(version),
            Err(error) => failures.push(error),
        }
        completed += 1;
    }

    for prefix in &prefixes {
        let percent = 35 + ((completed * 50) / total.max(1)) as u8;
        set_progress(
            percent,
            "下载安装最新内核",
            format!(
                "正在更新第 {}/{} 个目标：{}",
                completed + 1,
                total,
                prefix.display()
            ),
            false,
            None,
        );

        match install_latest_into_prefix(prefix) {
            Ok(version) => versions.push(version),
            Err(error) => failures.push(error),
        }
        completed += 1;
    }

    set_progress(88, "校验更新结果", "正在核对所有本地内核版本...", false, None);
    if !failures.is_empty() {
        return Err(format!("部分内核目标更新失败：{}", failures.join("；")));
    }

    let version = versions
        .into_iter()
        .max()
        .unwrap_or_else(|| "latest".to_string());

    set_progress(
        96,
        "完成最后检查",
        format!("已同步 {total} 个内核目标，确认版本 {version}"),
        false,
        None,
    );

    Ok(format!(
        "@deepseek-ai/dsh 已更新到 {version}（已同步 {total} 个内核目标），重启 DSH-UI 后生效"
    ))
}

pub fn update_dsh_engine() -> Result<String, String> {
    set_progress(2, "开始更新", "正在初始化 DeepSeek Harness 内核更新...", false, None);
    let result = update_dsh_engine_inner();
    match &result {
        Ok(message) => set_progress(100, "更新完成", message.clone(), true, Some(true)),
        Err(error) => mark_failed(error),
    }
    result
}
