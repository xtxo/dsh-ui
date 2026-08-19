#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[cfg(target_os = "windows")]
fn prepend_bundled_node_to_path() {
    use std::env;

    let Ok(exe_path) = env::current_exe() else {
        return;
    };
    let Some(exe_dir) = exe_path.parent() else {
        return;
    };

    // Tauri resources resolve next to the executable on Windows. Release builds
    // bundle a private Node runtime here so users do not need to install Node.js.
    let bundled_node_dir = exe_dir.join("runtime").join("node");
    if !bundled_node_dir.join("node.exe").exists() {
        return;
    }

    let current_path = env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bundled_node_dir];
    paths.extend(env::split_paths(&current_path));

    if let Ok(joined) = env::join_paths(paths) {
        env::set_var("PATH", joined);
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    prepend_bundled_node_to_path();

    app_lib::run()
}
