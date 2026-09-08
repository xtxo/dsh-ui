use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const ENGINE_REGISTRY: &str = "https://registry.npmjs.org";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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

    // The lightweight Portable build adds its private runtime to PATH before
    // Tauri starts, so PATH is the most accurate source of the runtime actually
    // used by the current process.
    if let Some(path) = env::var_os("PATH") {
        dirs.extend(env::split_paths(&path));
    }

    // Old self-contained Portable builds kept Node next to the executable.
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            dirs.push(exe_dir.join("runtime").join("node"));
        }
    }

    // New lightweight builds download a private Node distribution here on first
    // launch. Scan one directory level so this keeps working after future Node
    // runtime bumps instead of hard-coding 22.23.2 forever.
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
    // Do not call Command::new("npm") on Windows. npm is normally npm.cmd and
    // CreateProcess cannot discover that shim by the bare name reliably. Invoke
    // npm-cli.js with the matching node.exe instead; this also works for DSH-UI's
    // private first-launch Node runtime without a system-wide Node installation.
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

    // Legacy self-contained Portable/local server copy.
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

pub fn update_dsh_engine() -> Result<String, String> {
    let prefixes = npx_engine_prefixes();
    if prefixes.is_empty() {
        return Err(
            "没有找到当前 DeepSeek Harness 的 npx/Portable 缓存。请先正常启动一次 DSH-UI，让内核完成首次下载后再更新。"
                .to_string(),
        );
    }

    // backend.rs historically picked the first _npx cache directory it saw. Keep
    // every existing DSH cache on the same version so restart cannot accidentally
    // select a stale cache after a successful in-app update.
    let mut versions = Vec::new();
    let mut failures = Vec::new();
    for prefix in &prefixes {
        match install_latest_into_prefix(prefix) {
            Ok(version) => versions.push(version),
            Err(error) => failures.push(error),
        }
    }

    if !failures.is_empty() {
        return Err(format!(
            "部分内核缓存更新失败：{}",
            failures.join("；")
        ));
    }

    let version = versions
        .into_iter()
        .max()
        .unwrap_or_else(|| "latest".to_string());

    Ok(format!(
        "@deepseek-ai/dsh 已更新到 {version}（已同步 {} 个缓存），重启 DSH-UI 后生效",
        prefixes.len()
    ))
}
