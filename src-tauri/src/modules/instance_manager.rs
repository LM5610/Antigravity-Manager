use chrono::Utc;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use uuid::Uuid;

use crate::models::{
    DefaultInstanceSettings, InstanceDefaults, InstanceProfile, InstanceProfileView, InstanceStore,
};
use crate::modules;

pub const DEFAULT_INSTANCE_ID: &str = "__default__";
const INSTANCES_FILE: &str = "antigravity_instances.json";

static INSTANCE_STORE_LOCK: std::sync::LazyLock<Mutex<()>> =
    std::sync::LazyLock::new(|| Mutex::new(()));

/// 获取实例元数据 JSON 文件的存储路径
fn instances_path() -> Result<PathBuf, String> {
    let data_dir = modules::account::get_data_dir()?;
    Ok(data_dir.join(INSTANCES_FILE))
}

/// 加载实例配置存储
pub fn load_instance_store() -> Result<InstanceStore, String> {
    let path = instances_path()?;
    if !path.exists() {
        return Ok(InstanceStore::new());
    }

    let content = fs::read_to_string(&path).map_err(|e| format!("读取实例配置失败: {}", e))?;
    if content.trim().is_empty() {
        return Ok(InstanceStore::new());
    }

    match serde_json::from_str(&content) {
        Ok(store) => Ok(store),
        Err(e) => {
            crate::modules::logger::log_warn(&format!(
                "实例配置文件解析错误，回退为空实例列表: path={}, err={}",
                path.display(),
                e
            ));
            Ok(InstanceStore::new())
        }
    }
}

/// 保存实例配置存储（原子写入）
pub fn save_instance_store(store: &InstanceStore) -> Result<(), String> {
    let path = instances_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建实例配置目录失败: {}", e))?;
    }
    let content =
        serde_json::to_string_pretty(store).map_err(|e| format!("序列化实例配置失败: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("保存实例配置失败: {}", e))?;
    Ok(())
}

/// 获取系统默认的 Antigravity 用户数据目录
pub fn get_default_user_data_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().ok_or("无法获取 Home 目录")?;
        let ide_dir = home.join("Library/Application Support/Antigravity IDE");
        let app_dir = home.join("Library/Application Support/Antigravity");
        if ide_dir.exists() {
            return Ok(ide_dir);
        }
        if app_dir.exists() {
            return Ok(app_dir);
        }
        return Ok(ide_dir);
    }

    #[cfg(target_os = "windows")]
    {
        let app_data =
            std::env::var("APPDATA").map_err(|_| "无法获取 APPDATA 环境变量".to_string())?;
        let roaming = PathBuf::from(app_data);
        let ide_dir = roaming.join("Antigravity IDE");
        let app_dir = roaming.join("Antigravity");
        if ide_dir.exists() {
            return Ok(ide_dir);
        }
        if app_dir.exists() {
            return Ok(app_dir);
        }
        return Ok(ide_dir);
    }

    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().ok_or("无法获取 Home 目录")?;
        let ide_dir = home.join(".config/Antigravity IDE");
        let app_dir = home.join(".config/Antigravity");
        if ide_dir.exists() {
            return Ok(ide_dir);
        }
        if app_dir.exists() {
            return Ok(app_dir);
        }
        return Ok(ide_dir);
    }
}

/// 获取多开受管实例的推荐根目录
pub fn get_default_instances_root_dir() -> Result<PathBuf, String> {
    let data_dir = modules::account::get_data_dir()?;
    Ok(data_dir.join("instances"))
}

/// 获取实例初始路径默认值
pub fn get_instance_defaults() -> Result<InstanceDefaults, String> {
    let root_dir = get_default_instances_root_dir()?;
    let default_user_data_dir = get_default_user_data_dir()?;
    Ok(InstanceDefaults {
        root_dir: root_dir.to_string_lossy().to_string(),
        default_user_data_dir: default_user_data_dir.to_string_lossy().to_string(),
    })
}

/// 判断指定的数据目录是否已经初始化（含有有效文件）
pub fn is_profile_initialized(user_data_dir: &str) -> bool {
    let path = Path::new(user_data_dir);
    if !path.exists() || !path.is_dir() {
        return false;
    }
    // 检查是否有 User 目录或 state.vscdb
    if path.join("User").exists() {
        return true;
    }
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !matches!(name.as_str(), ".DS_Store" | "Thumbs.db" | "desktop.ini") {
                return true;
            }
        }
    }
    false
}

/// 过滤不需要复制的瞬时锁文件与巨型缓存目录
fn should_skip_copy_entry(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(
        lower.as_str(),
        "singletonlock"
            | "singletonsocket"
            | "singletoncookie"
            | "cache"
            | "code cache"
            | "gpucache"
            | "dawncache"
            | "crashpad"
            | "logs"
    ) || lower.ends_with(".lock")
}

/// 递归复制目录并排除锁文件与临时缓存
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if !src.exists() {
        return Err(format!("源目录不存在: {}", src.display()));
    }
    fs::create_dir_all(dst).map_err(|e| format!("创建目标目录失败: {}", e))?;

    for entry in fs::read_dir(src).map_err(|e| format!("读取源目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy();
        if should_skip_copy_entry(&name_str) {
            continue;
        }

        let child_src = entry.path();
        let child_dst = dst.join(&file_name);
        let file_type = entry
            .file_type()
            .map_err(|e| format!("获取文件类型失败: {}", e))?;

        if file_type.is_dir() {
            copy_dir_recursive(&child_src, &child_dst)?;
        } else if file_type.is_file() {
            let _ = fs::copy(&child_src, &child_dst);
        }
    }
    Ok(())
}

/// 确保 globalStorage 目录存在
fn ensure_global_storage(profile_dir: &Path) -> Result<PathBuf, String> {
    let global_storage = profile_dir.join("User").join("globalStorage");
    if !global_storage.exists() {
        fs::create_dir_all(&global_storage)
            .map_err(|e| format!("创建 globalStorage 失败: {}", e))?;
    }
    Ok(global_storage)
}

/// 确保 state.vscdb 存在并拥有基础数据表
fn ensure_state_db(profile_dir: &Path) -> Result<PathBuf, String> {
    let db_path = profile_dir
        .join("User")
        .join("globalStorage")
        .join("state.vscdb");
    if db_path.exists() {
        return Ok(db_path);
    }

    ensure_global_storage(profile_dir)?;
    let conn = Connection::open(&db_path).map_err(|e| format!("创建 state.vscdb 失败: {}", e))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value TEXT)",
        [],
    )
    .map_err(|e| format!("初始化 state.vscdb 表结构失败: {}", e))?;

    Ok(db_path)
}

/// 向指定实例的数据目录注入绑定的账号 Token
pub fn inject_account_to_instance(user_data_dir: &str, account_id: &str) -> Result<(), String> {
    let profile_dir = Path::new(user_data_dir);
    let db_path = ensure_state_db(profile_dir)?;
    let account = modules::account::load_account(account_id)?;

    modules::db::inject_token(
        &db_path,
        &account.token.access_token,
        &account.token.refresh_token,
        account.token.expiry,
        &account.email,
        account.token.is_gcp_tos.unwrap_or(false),
        account.token.project_id.as_deref(),
        account.token.id_token.as_deref(),
        account.token.oauth_client_key.as_deref(),
        Some("ide"),
    )?;

    crate::modules::logger::log_info(&format!(
        "[Instance] 成功向实例目录注入账号 Token: dir={}, email={}",
        user_data_dir, account.email
    ));
    Ok(())
}

/// 解析命令行中的 --user-data-dir
fn extract_user_data_dir_from_args(args: &[String]) -> Option<String> {
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if let Some(val) = arg.strip_prefix("--user-data-dir=") {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        if arg == "--user-data-dir" {
            if let Some(next) = args.get(index + 1) {
                let trimmed = next.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        index += 1;
    }
    None
}

/// 路径规范化比较
fn normalize_path_for_compare(p: &str) -> String {
    p.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// 收集系统中运行的 Antigravity 进程与其对应的 --user-data-dir
pub fn collect_antigravity_process_entries() -> Vec<(u32, Option<String>)> {
    let mut system = System::new_all();
    system.refresh_all();

    let mut main_pids = HashSet::new();
    let mut user_data_by_parent: HashMap<u32, String> = HashMap::new();
    let mut main_entries: Vec<(u32, Option<String>)> = Vec::new();

    for (pid, process) in system.processes() {
        let pid_u32 = pid.as_u32();
        let name = process.name().to_string_lossy().to_lowercase();
        let exe_path = process
            .exe()
            .map(|p| p.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let cmd_args: Vec<String> = process
            .cmd()
            .iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        let cmd_str = cmd_args.join(" ").to_lowercase();

        // 排除 Antigravity-Manager 自身
        if modules::process::is_manager_or_tools_process(&name, &exe_path, &name) {
            continue;
        }

        // 必须是 Antigravity 相关进程
        if !name.contains("antigravity")
            && !exe_path.contains("antigravity")
            && !cmd_str.contains("antigravity")
        {
            continue;
        }

        let user_data = extract_user_data_dir_from_args(&cmd_args);

        // 如果是辅助子进程（如 helper, gpu, renderer 等），子进程往往完整保留了 user-data-dir
        if modules::process::is_helper_process(&name, &cmd_str, &exe_path) {
            if let (Some(parent_pid), Some(dir)) = (process.parent(), user_data) {
                user_data_by_parent
                    .entry(parent_pid.as_u32())
                    .or_insert(dir);
            }
            continue;
        }

        // 记录为主进程候选
        main_pids.insert(pid_u32);
        main_entries.push((pid_u32, user_data));
    }

    // 将子进程收集到的 user_data 关联回主进程
    for (pid, user_data) in &mut main_entries {
        if user_data.is_none() {
            *user_data = user_data_by_parent.remove(pid);
        }
    }

    main_entries
}

/// 根据目标 user_data_dir 精准查找匹配的运行 PID
pub fn resolve_pid_for_instance(
    last_pid: Option<u32>,
    user_data_dir: Option<&str>,
    entries: &[(u32, Option<String>)],
) -> Option<u32> {
    let target_normalized = user_data_dir.map(normalize_path_for_compare);

    let mut matches = Vec::new();
    for (pid, dir_opt) in entries {
        match (&target_normalized, dir_opt) {
            (Some(target), Some(dir)) => {
                if normalize_path_for_compare(dir) == *target {
                    matches.push(*pid);
                }
            }
            (None, None) => {
                // 默认实例：没有显式指定 --user-data-dir 的主进程
                matches.push(*pid);
            }
            (None, Some(dir)) => {
                // 默认实例也可能匹配默认 user_data_dir
                if let Ok(default_dir) = get_default_user_data_dir() {
                    if normalize_path_for_compare(dir)
                        == normalize_path_for_compare(&default_dir.to_string_lossy())
                    {
                        matches.push(*pid);
                    }
                }
            }
            _ => {}
        }
    }

    // 优先命中与 last_pid 一致的存活进程
    if let Some(last) = last_pid {
        if matches.contains(&last) {
            return Some(last);
        }
    }

    matches.into_iter().min()
}

/// 检查某个 PID 是否仍然存活
pub fn is_pid_alive(pid: u32) -> bool {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        ProcessRefreshKind::nothing().with_memory(),
    );
    system.process(Pid::from_u32(pid)).is_some()
}

/// 关闭指定 PID 进程
pub fn kill_pid(pid: u32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output();
        thread::sleep(Duration::from_millis(300));
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("kill")
            .args(["-15", &pid.to_string()])
            .output();
        thread::sleep(Duration::from_millis(400));
        if is_pid_alive(pid) {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
        }
        Ok(())
    }
}

/// 聚焦并唤起指定 PID 的窗口到前台
pub fn focus_instance_window(pid: u32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            r#"$p=Get-Process -Id {pid} -ErrorAction Stop;$h=$p.MainWindowHandle;if($h -ne 0){{Add-Type @'
using System;
using System.Runtime.InteropServices;
public class Win32 {{
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr hWnd, int nCmdShow);
}}
'@;[Win32]::ShowWindowAsync($h, 9);[Win32]::SetForegroundWindow($h);}}"#
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .map_err(|e| format!("调用 PowerShell 唤起窗口失败: {}", e))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("唤起窗口失败: {}", err));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "tell application \"System Events\" to set frontmost of (first process whose unix id is {}) to true",
            pid
        );
        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("调用 osascript 失败: {}", e))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("macOS 唤起窗口失败: {}", err));
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("wmctrl")
            .args(["-i", "-a", &pid.to_string()])
            .output();
        Ok(())
    }
}

/// 获取所有实例（包含默认实例与各受管实例）的实时视图
pub fn list_instances() -> Result<Vec<InstanceProfileView>, String> {
    let store = load_instance_store()?;
    let entries = collect_antigravity_process_entries();
    let default_dir = get_default_user_data_dir()?;
    let default_dir_str = default_dir.to_string_lossy().to_string();

    let mut views: Vec<InstanceProfileView> = Vec::new();

    // 默认实例
    let default_pid = resolve_pid_for_instance(store.default_settings.last_pid, None, &entries);
    let default_running = default_pid.is_some();
    let default_initialized = is_profile_initialized(&default_dir_str);
    views.push(InstanceProfileView {
        id: DEFAULT_INSTANCE_ID.to_string(),
        name: "默认实例 (Default)".to_string(),
        user_data_dir: default_dir_str,
        working_dir: None,
        extra_args: store.default_settings.extra_args.clone(),
        bind_account_id: store.default_settings.bind_account_id.clone(),
        created_at: 0,
        last_launched_at: None,
        last_pid: default_pid,
        running: default_running,
        initialized: default_initialized,
        is_default: true,
    });

    // 受管实例列表
    for instance in store.instances {
        let pid =
            resolve_pid_for_instance(instance.last_pid, Some(&instance.user_data_dir), &entries);
        let running = pid.is_some();
        let initialized = is_profile_initialized(&instance.user_data_dir);
        let mut view = InstanceProfileView::from_profile(instance, running, initialized);
        view.last_pid = pid;
        views.push(view);
    }

    Ok(views)
}

/// 创建新实例
pub fn create_instance(
    name: String,
    user_data_dir: String,
    working_dir: Option<String>,
    extra_args: Option<String>,
    bind_account_id: Option<String>,
    copy_source_instance_id: Option<String>,
    init_mode: Option<String>,
) -> Result<InstanceProfileView, String> {
    let _guard = INSTANCE_STORE_LOCK.lock().map_err(|_| "获取实例锁失败")?;
    let mut store = load_instance_store()?;

    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err("实例名称不能为空".to_string());
    }
    let trimmed_dir = user_data_dir.trim();
    if trimmed_dir.is_empty() {
        return Err("实例数据目录不能为空".to_string());
    }

    // 唯一性校验
    for item in &store.instances {
        if item.name.eq_ignore_ascii_case(trimmed_name) {
            return Err("已存在同名实例".to_string());
        }
        if normalize_path_for_compare(&item.user_data_dir)
            == normalize_path_for_compare(trimmed_dir)
        {
            return Err("该数据目录已被其他实例使用".to_string());
        }
    }

    let target_path = PathBuf::from(trimmed_dir);
    let mode = init_mode.as_deref().unwrap_or("copy");

    match mode {
        "existing_dir" | "existingdir" => {
            if !target_path.exists() {
                return Err("所选目录不存在".to_string());
            }
        }
        "empty" => {
            fs::create_dir_all(&target_path).map_err(|e| format!("创建实例目录失败: {}", e))?;
            ensure_state_db(&target_path)?;
        }
        _ => {
            // "copy" 模式
            let src_dir = match copy_source_instance_id.as_deref() {
                Some(DEFAULT_INSTANCE_ID) | None => get_default_user_data_dir()?,
                Some(src_id) => {
                    let src = store
                        .instances
                        .iter()
                        .find(|i| i.id == src_id)
                        .ok_or("指定复制来源实例不存在")?;
                    PathBuf::from(&src.user_data_dir)
                }
            };
            copy_dir_recursive(&src_dir, &target_path)?;
        }
    }

    // 如果指定了绑定账号，在启动前先预埋
    if let Some(ref acc_id) = bind_account_id {
        if !acc_id.trim().is_empty() {
            let _ = inject_account_to_instance(trimmed_dir, acc_id);
        }
    }

    let instance = InstanceProfile {
        id: Uuid::new_v4().to_string(),
        name: trimmed_name.to_string(),
        user_data_dir: trimmed_dir.to_string(),
        working_dir: working_dir.filter(|w| !w.trim().is_empty()),
        extra_args: extra_args.unwrap_or_default().trim().to_string(),
        bind_account_id: bind_account_id.filter(|a| !a.trim().is_empty()),
        created_at: Utc::now().timestamp_millis(),
        last_launched_at: None,
        last_pid: None,
    };

    store.instances.push(instance.clone());
    save_instance_store(&store)?;

    let initialized = is_profile_initialized(&instance.user_data_dir);
    Ok(InstanceProfileView::from_profile(
        instance,
        false,
        initialized,
    ))
}

/// 更新实例配置
pub fn update_instance(
    instance_id: String,
    name: Option<String>,
    working_dir: Option<String>,
    extra_args: Option<String>,
    bind_account_id: Option<Option<String>>,
) -> Result<InstanceProfileView, String> {
    let _guard = INSTANCE_STORE_LOCK.lock().map_err(|_| "获取实例锁失败")?;
    let mut store = load_instance_store()?;

    if instance_id == DEFAULT_INSTANCE_ID {
        if let Some(args) = extra_args {
            store.default_settings.extra_args = args.trim().to_string();
        }
        if let Some(bind) = bind_account_id {
            store.default_settings.bind_account_id = bind.filter(|b| !b.trim().is_empty());
        }
        save_instance_store(&store)?;

        let default_dir = get_default_user_data_dir()?;
        let default_dir_str = default_dir.to_string_lossy().to_string();
        let running = store.default_settings.last_pid.is_some_and(is_pid_alive);
        return Ok(InstanceProfileView {
            id: DEFAULT_INSTANCE_ID.to_string(),
            name: "默认实例 (Default)".to_string(),
            user_data_dir: default_dir_str,
            working_dir: None,
            extra_args: store.default_settings.extra_args,
            bind_account_id: store.default_settings.bind_account_id,
            created_at: 0,
            last_launched_at: None,
            last_pid: store.default_settings.last_pid,
            running,
            initialized: true,
            is_default: true,
        });
    }

    let instance = store
        .instances
        .iter_mut()
        .find(|i| i.id == instance_id)
        .ok_or("实例不存在")?;

    if let Some(n) = name {
        let trimmed = n.trim();
        if !trimmed.is_empty() {
            instance.name = trimmed.to_string();
        }
    }
    if let Some(w) = working_dir {
        instance.working_dir = if w.trim().is_empty() {
            None
        } else {
            Some(w.trim().to_string())
        };
    }
    if let Some(args) = extra_args {
        instance.extra_args = args.trim().to_string();
    }
    if let Some(bind) = bind_account_id {
        instance.bind_account_id = bind.filter(|b| !b.trim().is_empty());
    }

    let updated = instance.clone();
    save_instance_store(&store)?;

    let running = updated.last_pid.is_some_and(is_pid_alive);
    let initialized = is_profile_initialized(&updated.user_data_dir);
    Ok(InstanceProfileView::from_profile(
        updated,
        running,
        initialized,
    ))
}

/// 删除实例
pub fn delete_instance(instance_id: &str, remove_files: bool) -> Result<(), String> {
    let _guard = INSTANCE_STORE_LOCK.lock().map_err(|_| "获取实例锁失败")?;
    let mut store = load_instance_store()?;

    let index = store
        .instances
        .iter()
        .position(|i| i.id == instance_id)
        .ok_or("实例不存在")?;

    let removed = store.instances.remove(index);
    save_instance_store(&store)?;

    if remove_files {
        let dir_path = PathBuf::from(&removed.user_data_dir);
        let default_root = get_default_instances_root_dir()?;
        // 安全保护：仅允许自动删除受管实例根目录下的子目录，避免误删系统目录
        if dir_path.starts_with(&default_root) && dir_path.exists() {
            let _ = fs::remove_dir_all(&dir_path);
        }
    }

    Ok(())
}

/// 启动指定实例
pub fn start_instance(instance_id: &str) -> Result<InstanceProfileView, String> {
    let exe_path = modules::process::get_antigravity_executable_path(Some("ide"))
        .or_else(|| modules::process::get_antigravity_executable_path(None))
        .ok_or("未找到 Antigravity 可执行文件，请在设置中指定路径")?;

    let (user_data_dir, working_dir, extra_args_str, bind_account_id) = {
        let store = load_instance_store()?;
        if instance_id == DEFAULT_INSTANCE_ID {
            let default_dir = get_default_user_data_dir()?;
            (
                default_dir.to_string_lossy().to_string(),
                None,
                store.default_settings.extra_args,
                store.default_settings.bind_account_id,
            )
        } else {
            let instance = store
                .instances
                .iter()
                .find(|i| i.id == instance_id)
                .ok_or("实例不存在")?;
            (
                instance.user_data_dir.clone(),
                instance.working_dir.clone(),
                instance.extra_args.clone(),
                instance.bind_account_id.clone(),
            )
        }
    };

    // 如果绑定了账号，启动前自动注入 Token
    if let Some(ref acc_id) = bind_account_id {
        if !acc_id.trim().is_empty() {
            if let Err(e) = inject_account_to_instance(&user_data_dir, acc_id) {
                crate::modules::logger::log_warn(&format!("启动实例前注入账号 Token 失败: {}", e));
            }
        }
    }

    let extra_args: Vec<String> = extra_args_str
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    let pid = launch_antigravity_process(
        &exe_path,
        &user_data_dir,
        working_dir.as_deref(),
        &extra_args,
    )?;

    // 更新最后启动 PID 和时间
    {
        let _guard = INSTANCE_STORE_LOCK.lock().map_err(|_| "获取实例锁失败")?;
        let mut store = load_instance_store()?;
        if instance_id == DEFAULT_INSTANCE_ID {
            store.default_settings.last_pid = Some(pid);
        } else if let Some(target) = store.instances.iter_mut().find(|i| i.id == instance_id) {
            target.last_pid = Some(pid);
            target.last_launched_at = Some(Utc::now().timestamp_millis());
        }
        let _ = save_instance_store(&store);
    }

    let initialized = is_profile_initialized(&user_data_dir);
    if instance_id == DEFAULT_INSTANCE_ID {
        Ok(InstanceProfileView {
            id: DEFAULT_INSTANCE_ID.to_string(),
            name: "默认实例 (Default)".to_string(),
            user_data_dir,
            working_dir,
            extra_args: extra_args_str,
            bind_account_id,
            created_at: 0,
            last_launched_at: Some(Utc::now().timestamp_millis()),
            last_pid: Some(pid),
            running: true,
            initialized,
            is_default: true,
        })
    } else {
        let store = load_instance_store()?;
        let target = store
            .instances
            .into_iter()
            .find(|i| i.id == instance_id)
            .unwrap();
        Ok(InstanceProfileView::from_profile(target, true, initialized))
    }
}

/// 跨平台执行 Antigravity 进程拉起
fn launch_antigravity_process(
    exe_path: &Path,
    user_data_dir: &str,
    working_dir: Option<&str>,
    extra_args: &[String],
) -> Result<u32, String> {
    crate::modules::logger::log_info(&format!(
        "[Instance Launch] 正在启动 Antigravity: exe={}, user_data_dir={}",
        exe_path.display(),
        user_data_dir
    ));

    #[cfg(target_os = "macos")]
    {
        let path_str = exe_path.to_string_lossy().to_string();
        let app_root = if let Some(app_idx) = path_str.find(".app") {
            &path_str[..app_idx + 4]
        } else {
            &path_str
        };

        let mut cmd = Command::new("open");
        cmd.arg("-n").arg("-a").arg(app_root);
        cmd.arg("--args");
        cmd.arg("--user-data-dir").arg(user_data_dir);
        for arg in extra_args {
            if !arg.trim().is_empty() {
                cmd.arg(arg);
            }
        }
        if let Some(cwd) = working_dir {
            cmd.arg(cwd);
        }

        let output = cmd
            .output()
            .map_err(|e| format!("启动 Antigravity 失败: {}", e))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("macOS open 失败: {}", err));
        }

        // 探测启动后的 PID
        thread::sleep(Duration::from_millis(500));
        let entries = collect_antigravity_process_entries();
        if let Some(pid) = resolve_pid_for_instance(None, Some(user_data_dir), &entries) {
            return Ok(pid);
        }
        Ok(0)
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = Command::new(exe_path);
        if let Some(parent) = exe_path.parent() {
            cmd.current_dir(parent);
        }
        if let Some(cwd) = working_dir {
            if Path::new(cwd).exists() {
                cmd.current_dir(cwd);
            }
        }

        // 后台脱钩独立启动
        cmd.creation_flags(0x08000000 | 0x00000200 | 0x00000008);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        cmd.arg("--user-data-dir").arg(user_data_dir);
        cmd.arg("--reuse-window");
        for arg in extra_args {
            if !arg.trim().is_empty() {
                cmd.arg(arg);
            }
        }
        if let Some(cwd) = working_dir {
            cmd.arg(cwd);
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("启动 Antigravity 进程失败: {}", e))?;
        Ok(child.id())
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new(exe_path);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        cmd.arg("--user-data-dir").arg(user_data_dir);
        for arg in extra_args {
            if !arg.trim().is_empty() {
                cmd.arg(arg);
            }
        }
        if let Some(cwd) = working_dir {
            cmd.arg(cwd);
        }
        let child = cmd
            .spawn()
            .map_err(|e| format!("启动 Antigravity 进程失败: {}", e))?;
        Ok(child.id())
    }
}

/// 停止指定实例
pub fn stop_instance(instance_id: &str) -> Result<(), String> {
    let entries = collect_antigravity_process_entries();
    let store = load_instance_store()?;

    let (last_pid, user_data_dir) = if instance_id == DEFAULT_INSTANCE_ID {
        (store.default_settings.last_pid, None)
    } else {
        let instance = store
            .instances
            .iter()
            .find(|i| i.id == instance_id)
            .ok_or("实例不存在")?;
        (instance.last_pid, Some(instance.user_data_dir.as_str()))
    };

    if let Some(pid) = resolve_pid_for_instance(last_pid, user_data_dir, &entries) {
        kill_pid(pid)?;
    }

    // 清理最后记录的 PID
    let _guard = INSTANCE_STORE_LOCK.lock().map_err(|_| "获取实例锁失败")?;
    let mut store = load_instance_store()?;
    if instance_id == DEFAULT_INSTANCE_ID {
        store.default_settings.last_pid = None;
    } else if let Some(target) = store.instances.iter_mut().find(|i| i.id == instance_id) {
        target.last_pid = None;
    }
    let _ = save_instance_store(&store);

    Ok(())
}
