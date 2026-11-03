use std::path::Path;
use std::process::Command;

use crate::models::{InstanceDefaults, InstanceProfileView};
use crate::modules::instance_manager;

#[tauri::command]
pub async fn get_instance_defaults() -> Result<InstanceDefaults, String> {
    instance_manager::get_instance_defaults()
}

#[tauri::command]
pub async fn list_instances() -> Result<Vec<InstanceProfileView>, String> {
    instance_manager::list_instances()
}

#[tauri::command]
pub async fn create_instance(
    name: String,
    user_data_dir: String,
    working_dir: Option<String>,
    extra_args: Option<String>,
    bind_account_id: Option<String>,
    copy_source_instance_id: Option<String>,
    init_mode: Option<String>,
) -> Result<InstanceProfileView, String> {
    instance_manager::create_instance(
        name,
        user_data_dir,
        working_dir,
        extra_args,
        bind_account_id,
        copy_source_instance_id,
        init_mode,
    )
}

#[tauri::command]
pub async fn update_instance(
    instance_id: String,
    name: Option<String>,
    working_dir: Option<String>,
    extra_args: Option<String>,
    bind_account_id: Option<Option<String>>,
) -> Result<InstanceProfileView, String> {
    instance_manager::update_instance(instance_id, name, working_dir, extra_args, bind_account_id)
}

#[tauri::command]
pub async fn delete_instance(
    instance_id: String,
    remove_files: Option<bool>,
) -> Result<(), String> {
    instance_manager::delete_instance(&instance_id, remove_files.unwrap_or(false))
}

#[tauri::command]
pub async fn start_instance(instance_id: String) -> Result<InstanceProfileView, String> {
    instance_manager::start_instance(&instance_id)
}

#[tauri::command]
pub async fn stop_instance(instance_id: String) -> Result<(), String> {
    instance_manager::stop_instance(&instance_id)
}

#[tauri::command]
pub async fn focus_instance_window(pid: u32) -> Result<(), String> {
    instance_manager::focus_instance_window(pid)
}

#[tauri::command]
pub async fn open_instance_folder(user_data_dir: String) -> Result<(), String> {
    let path = Path::new(&user_data_dir);
    if !path.exists() {
        return Err("目录不存在".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    Ok(())
}
