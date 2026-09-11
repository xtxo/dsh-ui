pub use super::backend_base::{
    find_dsh_cli, get_injected_updater_script, is_backend_running, perform_update_check,
};

#[cfg(not(target_os = "windows"))]
pub use super::backend_base::{find_node_executable, get_extended_path};

#[cfg(target_os = "windows")]
use std::fs;
#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::time::Duration;
#[cfg(target_os = "windows")]
use tauri::{Url, WebviewWindow};

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct BackendDiagnosis {
    title: String,
    reason: String,
    action: String,
    details: String,
}

#[cfg(target_os = "windows")]
fn backend_log_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("dsh-ui")
        .join("logs")
        .join("backend.log")
}

#[cfg(target_os = "windows")]
fn current_session(log: &str) -> &str {
    const MARKER: &str = "session: DSH-UI backend startup begins";
    log.rfind(MARKER)
        .and_then(|index| log.get(index..))
        .unwrap_or(log)
}

#[cfg(target_os = "windows")]
fn strip_log_prefix(line: &str) -> &str {
    line.split_once("] ").map(|(_, rest)| rest).unwrap_or(line)
}

#[cfg(target_os = "windows")]
fn extract_plugin_name(session: &str) -> Option<String> {
    const MARKER: &str = "failed to apply loader entry ";
    for line in session.lines() {
        let payload = strip_log_prefix(line);
        let Some(index) = payload.find(MARKER) else {
            continue;
        };
        let rest = &payload[index + MARKER.len()..];
        let entry = rest.split(':').next().unwrap_or(rest).trim();
        if let Some(start) = entry.find('(') {
            if let Some(end) = entry[start + 1..].find(')') {
                let package = entry[start + 1..start + 1 + end].trim();
                if !package.is_empty() {
                    return Some(package.to_string());
                }
            }
        }
        let fallback = entry.split_whitespace().next().unwrap_or_default().trim();
        if !fallback.is_empty() {
            return Some(fallback.to_string());
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn diagnostic_details(session: &str) -> String {
    let mut lines = session
        .lines()
        .filter_map(|line| {
            let payload = strip_log_prefix(line).trim();
            if payload.is_empty() {
                return None;
            }
            let lower = payload.to_ascii_lowercase();
            if lower.contains("token=") {
                return Some("[认证 Token 已隐藏]".to_string());
            }
            if payload.starts_with("stderr:")
                || payload.starts_with("process:")
                || payload.starts_with("startup:")
            {
                return Some(payload.to_string());
            }
            None
        })
        .collect::<Vec<_>>();

    if lines.len() > 10 {
        lines = lines.split_off(lines.len() - 10);
    }
    let mut details = lines.join("\n");
    if details.len() > 2400 {
        details = details[details.len() - 2400..].to_string();
    }
    details
}

#[cfg(target_os = "windows")]
fn diagnose_backend_failure(session: &str) -> BackendDiagnosis {
    let lower = session.to_ascii_lowercase();
    let details = diagnostic_details(session);

    if lower.contains("failed to apply loader entry") {
        let plugin = extract_plugin_name(session).unwrap_or_else(|| "未知第三方插件".to_string());
        let compatibility = lower.contains("without inject")
            || lower.contains("cannot get property \"webserver\"");
        return BackendDiagnosis {
            title: "第三方插件导致 DeepSeek Harness 启动失败".to_string(),
            reason: if compatibility {
                format!(
                    "插件 {plugin} 与当前 DeepSeek Harness 的插件接口不兼容。它访问了 webServer，但没有按当前版本声明所需的 inject。"
                )
            } else {
                format!("插件 {plugin} 在加载阶段报错，导致 dsh web 进程直接退出。")
            },
            action: format!(
                "请先更新或停用 {plugin}，然后关闭并重新打开 DSH-UI。其它 DSH 配置不需要删除。"
            ),
            details,
        };
    }

    if lower.contains("pnpm failed in profile directory")
        || lower.contains("pnpm not found")
        || session.contains("系统找不到指定的路径")
    {
        return BackendDiagnosis {
            title: "DSH 插件管理器无法运行".to_string(),
            reason: "DeepSeek Harness 在加载或维护当前 profile 插件时无法正常启动 pnpm，因此后端没有完成启动。".to_string(),
            action: "请检查 pnpm 是否可用（pnpm --version）。修复 pnpm 后重新启动 DSH-UI；如果同时出现具体插件错误，优先更新或停用该插件。".to_string(),
            details,
        };
    }

    if lower.contains("eaddrinuse") || lower.contains("address already in use") {
        return BackendDiagnosis {
            title: "DeepSeek Harness 端口被占用".to_string(),
            reason: "dsh web 尝试监听本地端口时发现端口已被其它进程占用，因此启动失败。".to_string(),
            action: "关闭占用该端口的旧 DSH 进程后重新启动 DSH-UI。DSH-UI 会尽量自动清理自己留下的残留进程。".to_string(),
            details,
        };
    }

    if lower.contains("authentication required")
        || lower.contains("did not provide an authenticated launch url")
        || lower.contains("timed out waiting 10 minutes for authenticated dsh launch url")
    {
        return BackendDiagnosis {
            title: "没有拿到 DeepSeek Harness 登录凭证".to_string(),
            reason: "后端可能已经启动，但 DSH-UI 没有捕获到 dsh web 输出的带 Token 登录地址，因此 WebView 无法完成认证。".to_string(),
            action: "关闭所有残留的 dsh web 进程后重新启动 DSH-UI。如果仍然出现，请把 backend.log 发出来继续定位。".to_string(),
            details,
        };
    }

    if lower.contains("enoent")
        || lower.contains("not recognized as an internal or external command")
        || lower.contains("cannot find module")
    {
        return BackendDiagnosis {
            title: "DeepSeek Harness 运行环境缺失".to_string(),
            reason: "后端启动命令找不到所需的 Node.js、DSH 文件或依赖模块，所以进程在启动阶段退出。".to_string(),
            action: "请重新启动 DSH-UI 让首次运行环境检查完成；若仍失败，再检查 Node.js/DSH 安装是否完整。".to_string(),
            details,
        };
    }

    BackendDiagnosis {
        title: "DeepSeek Harness 后端启动失败".to_string(),
        reason: "dsh web 进程启动后异常退出，所以本地网页服务没有继续监听，WebView 才会显示“127.0.0.1 拒绝连接”。".to_string(),
        action: "下面已经显示了最后一段后端错误。可以直接把该信息或 backend.log 发给维护者继续定位。".to_string(),
        details,
    }
}

#[cfg(target_os = "windows")]
fn show_backend_failure(window: &WebviewWindow, diagnosis: &BackendDiagnosis) {
    let log_path = backend_log_path().display().to_string();
    let title = serde_json::to_string(&diagnosis.title).unwrap_or_else(|_| "\"后端启动失败\"".to_string());
    let reason = serde_json::to_string(&diagnosis.reason).unwrap_or_else(|_| "\"未知原因\"".to_string());
    let action = serde_json::to_string(&diagnosis.action).unwrap_or_else(|_| "\"请查看日志\"".to_string());
    let details = serde_json::to_string(&diagnosis.details).unwrap_or_else(|_| "\"\"".to_string());
    let log_path_json = serde_json::to_string(&log_path).unwrap_or_else(|_| "\"backend.log\"".to_string());

    let script = r#"
(() => {
  const title = __TITLE__;
  const reason = __REASON__;
  const action = __ACTION__;
  const details = __DETAILS__;
  const logPath = __LOG_PATH__;

  document.documentElement.innerHTML = `
    <head>
      <meta charset="utf-8">
      <meta name="viewport" content="width=device-width,initial-scale=1">
      <title>DSH-UI 启动诊断</title>
      <style>
        *{box-sizing:border-box} body{margin:0;background:#08101f;color:#e5edf8;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI","Microsoft YaHei",sans-serif;min-height:100vh;display:flex;align-items:center;justify-content:center;padding:28px}
        .card{width:min(760px,100%);background:#0f1a2f;border:1px solid #263552;border-radius:18px;padding:26px;box-shadow:0 28px 80px rgba(0,0,0,.42)}
        .eyebrow{color:#7aa2ff;font-size:13px;font-weight:700;margin-bottom:10px}.title{font-size:23px;font-weight:800;line-height:1.35;margin-bottom:16px;color:#fff}
        .box{background:#111f37;border:1px solid #293b5c;border-radius:12px;padding:14px 16px;margin:12px 0}.label{font-size:12px;color:#8fa5c7;margin-bottom:6px;font-weight:700}.text{font-size:14px;line-height:1.75;white-space:pre-wrap;word-break:break-word}
        .action{border-color:#315b4b;background:#0e2924}.action .label{color:#5ee0ad}.details{font-family:Consolas,"SFMono-Regular",monospace;font-size:12px;color:#b8c6dc;max-height:230px;overflow:auto;white-space:pre-wrap;word-break:break-word}
        .path{font-family:Consolas,"SFMono-Regular",monospace;font-size:12px;color:#9fb7df;word-break:break-all}.row{display:flex;gap:10px;align-items:center;flex-wrap:wrap;margin-top:18px}
        button{border:0;border-radius:9px;padding:10px 14px;background:#315ff4;color:white;font-weight:700;cursor:pointer}button:hover{filter:brightness(1.08)}.muted{font-size:12px;color:#7f91ad}
      </style>
    </head>
    <body><main class="card">
      <div class="eyebrow">🐋 DSH-UI 启动诊断</div>
      <div id="d-title" class="title"></div>
      <section class="box"><div class="label">为什么打不开</div><div id="d-reason" class="text"></div></section>
      <section class="box action"><div class="label">建议处理</div><div id="d-action" class="text"></div></section>
      <section class="box"><div class="label">后端最后错误</div><div id="d-details" class="details"></div></section>
      <section class="box"><div class="label">完整日志</div><div id="d-path" class="path"></div></section>
      <div class="row"><button id="copy-log">复制日志路径</button><span id="copy-state" class="muted">修复后关闭并重新打开 DSH-UI。</span></div>
    </main></body>`;

  document.getElementById('d-title').textContent = title;
  document.getElementById('d-reason').textContent = reason;
  document.getElementById('d-action').textContent = action;
  document.getElementById('d-details').textContent = details || '没有捕获到更多 stderr；请查看完整日志。';
  document.getElementById('d-path').textContent = logPath;
  document.getElementById('copy-log').addEventListener('click', async () => {
    const state = document.getElementById('copy-state');
    try {
      await navigator.clipboard.writeText(logPath);
      state.textContent = '✅ 日志路径已复制';
    } catch (_) {
      state.textContent = '请手动复制上面的日志路径';
    }
  });
})();
"#
    .replace("__TITLE__", &title)
    .replace("__REASON__", &reason)
    .replace("__ACTION__", &action)
    .replace("__DETAILS__", &details)
    .replace("__LOG_PATH__", &log_path_json);

    if let Ok(blank) = Url::parse("about:blank") {
        let _ = window.navigate(blank);
    }
    for _ in 0..4 {
        std::thread::sleep(Duration::from_millis(180));
        if window.eval(&script).is_ok() {
            break;
        }
    }
    let _ = window.show();
    let _ = window.set_focus();
}

#[cfg(target_os = "windows")]
fn failure_observed(session: &str) -> bool {
    session.contains("process: failed to spawn backend")
        || session.contains("startup: timed out waiting 10 minutes for authenticated DSH launch URL")
        || (session.contains("process: backend wrapper PID") && session.contains("exited with"))
}

#[cfg(target_os = "windows")]
fn start_backend_diagnostic_watcher(window: WebviewWindow) {
    std::thread::spawn(move || {
        // The backend itself may spend several minutes downloading on first run.
        // Poll the small rotating log rather than touching the child process handle.
        for _ in 0..1260 {
            std::thread::sleep(Duration::from_millis(500));
            let Ok(log) = fs::read_to_string(backend_log_path()) else {
                continue;
            };
            let session = current_session(&log);

            // Normal application shutdown deliberately kills the backend. Never turn
            // that expected exit into an alarming failure page.
            if session.contains("shutdown: DSH-UI backend cleanup begins") {
                return;
            }

            if !failure_observed(session) {
                continue;
            }

            let diagnosis = diagnose_backend_failure(session);
            show_backend_failure(&window, &diagnosis);
            return;
        }
    });
}

pub fn start_backend_service_if_needed(
    app_handle: &tauri::AppHandle,
    window: tauri::WebviewWindow,
) {
    super::backend_base::start_backend_service_if_needed(app_handle, window.clone());

    #[cfg(target_os = "windows")]
    start_backend_diagnostic_watcher(window);
}

pub fn cleanup_backend() {
    super::backend_base::cleanup_backend();
}
