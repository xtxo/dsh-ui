#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[cfg(target_os = "windows")]
mod windows_runtime {
    use std::env;
    use std::ffi::OsStr;
    use std::fs;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK,
    };

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const NODE_VERSION: &str = "22.23.2";
    const NODE_ARCHIVE: &str = "node-v22.23.2-win-x64.zip";
    const NODE_SHA256: &str = "1177b4137ba5adaa56354ae40f1080c7450e8ae09cecb47da459d1c52ac99f97";
    const NODE_URL: &str =
        "https://nodejs.org/dist/v22.23.2/node-v22.23.2-win-x64.zip";

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value).encode_wide().chain(Some(0)).collect()
    }

    fn message_box(title: &str, message: &str, flags: u32) {
        let title = wide(title);
        let message = wide(message);
        unsafe {
            MessageBoxW(0, message.as_ptr(), title.as_ptr(), flags);
        }
    }

    fn command_exists(command: &str) -> bool {
        Command::new("where.exe")
            .arg(command)
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn executable_dir() -> Option<PathBuf> {
        env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
    }

    fn bundled_portable_runtime() -> Option<PathBuf> {
        let exe_dir = executable_dir()?;
        let runtime_dir = exe_dir.join("runtime").join("node");
        let server_script = exe_dir
            .join("server")
            .join("node_modules")
            .join("@deepseek-ai")
            .join("dsh")
            .join("lib")
            .join("bin.js");

        if runtime_dir.join("node.exe").exists() && server_script.exists() {
            Some(runtime_dir)
        } else {
            None
        }
    }

    fn private_runtime_dir() -> Option<PathBuf> {
        let local_appdata = env::var_os("LOCALAPPDATA")?;
        Some(
            PathBuf::from(local_appdata)
                .join("dsh-ui")
                .join("runtime")
                .join(format!("node-v{NODE_VERSION}-win-x64")),
        )
    }

    fn add_runtime_to_path(runtime_dir: &Path, system_node_exists: bool) {
        let current_path = env::var_os("PATH").unwrap_or_default();
        let mut paths: Vec<PathBuf> = env::split_paths(&current_path).collect();

        if system_node_exists {
            paths.push(runtime_dir.to_path_buf());
        } else {
            paths.insert(0, runtime_dir.to_path_buf());
        }

        if let Ok(joined) = env::join_paths(paths) {
            env::set_var("PATH", joined);
        }
    }

    fn download_private_runtime(runtime_dir: &Path) -> Result<(), String> {
        let Some(runtime_root) = runtime_dir.parent() else {
            return Err("无法确定 Node.js 运行时目录".to_string());
        };
        fs::create_dir_all(runtime_root)
            .map_err(|error| format!("无法创建运行时目录: {error}"))?;

        let zip_path = runtime_root.join(NODE_ARCHIVE);
        let runtime_root_text = runtime_root.to_string_lossy().replace(''', "''");
        let zip_path_text = zip_path.to_string_lossy().replace(''', "''");
        let runtime_dir_text = runtime_dir.to_string_lossy().replace(''', "''");

        let script = format!(
            r#"$ErrorActionPreference = 'Stop'
$runtimeRoot = '{runtime_root}'
$zipPath = '{zip_path}'
$runtimeDir = '{runtime_dir}'
if (-not (Test-Path (Join-Path $runtimeDir 'npx.cmd'))) {{
  Invoke-WebRequest -UseBasicParsing -Uri '{node_url}' -OutFile $zipPath
  $hash = (Get-FileHash -Algorithm SHA256 $zipPath).Hash.ToLowerInvariant()
  if ($hash -ne '{node_sha256}') {{
    Remove-Item $zipPath -Force -ErrorAction SilentlyContinue
    throw "Node.js runtime checksum mismatch: $hash"
  }}
  Expand-Archive -Path $zipPath -DestinationPath $runtimeRoot -Force
  Remove-Item $zipPath -Force -ErrorAction SilentlyContinue
}}
if (-not (Test-Path (Join-Path $runtimeDir 'npx.cmd'))) {{
  throw 'Node.js runtime extraction failed'
}}
"#,
            runtime_root = runtime_root_text,
            zip_path = zip_path_text,
            runtime_dir = runtime_dir_text,
            node_url = NODE_URL,
            node_sha256 = NODE_SHA256,
        );

        let output = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| format!("无法启动 PowerShell 下载 Node.js: {error}"))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() { stderr } else { stdout };
        Err(if details.is_empty() {
            "Node.js 运行时下载或解压失败".to_string()
        } else {
            format!("Node.js 运行时准备失败: {details}")
        })
    }

    pub fn prepare() {
        let system_node = command_exists("node");
        let system_npx = command_exists("npx");

        // Best case: use the user's existing Node.js toolchain unchanged.
        if system_node && system_npx {
            return;
        }

        // Portable ZIP ships a private node.exe + bundled dsh server. If a system
        // Node exists we deliberately leave PATH alone so the system Node wins.
        if let Some(runtime_dir) = bundled_portable_runtime() {
            if !system_node {
                add_runtime_to_path(&runtime_dir, false);
            }
            return;
        }

        let Some(runtime_dir) = private_runtime_dir() else {
            message_box(
                "DSH-UI 启动失败",
                "未检测到可用的 Node.js / npx，并且无法确定用户运行时目录。",
                MB_OK | MB_ICONERROR,
            );
            return;
        };

        if runtime_dir.join("node.exe").exists() && runtime_dir.join("npx.cmd").exists() {
            add_runtime_to_path(&runtime_dir, system_node);
            return;
        }

        message_box(
            "DSH-UI 首次启动",
            "未检测到完整的 Node.js + npx 环境。\n\nDSH-UI 将从 Node.js 官方下载约 36 MB 的便携运行时到当前用户目录，仅首次需要；不会安装到系统，也不需要管理员权限。",
            MB_OK | MB_ICONINFORMATION,
        );

        match download_private_runtime(&runtime_dir) {
            Ok(()) => add_runtime_to_path(&runtime_dir, system_node),
            Err(error) => message_box(
                "DSH-UI 运行环境准备失败",
                &format!(
                    "{error}\n\n你也可以自行安装 Node.js 22+ 后重新启动 DSH-UI。"
                ),
                MB_OK | MB_ICONERROR,
            ),
        }
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    windows_runtime::prepare();

    app_lib::run()
}
