use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceProfile {
    pub id: String,
    pub name: String,
    pub user_data_dir: String,
    pub working_dir: Option<String>,
    #[serde(default)]
    pub extra_args: String,
    pub bind_account_id: Option<String>,
    pub created_at: i64,
    pub last_launched_at: Option<i64>,
    pub last_pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DefaultInstanceSettings {
    #[serde(default)]
    pub extra_args: String,
    pub bind_account_id: Option<String>,
    pub last_pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstanceStore {
    #[serde(default)]
    pub instances: Vec<InstanceProfile>,
    #[serde(default)]
    pub default_settings: DefaultInstanceSettings,
}

impl InstanceStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDefaults {
    pub root_dir: String,
    pub default_user_data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceProfileView {
    pub id: String,
    pub name: String,
    pub user_data_dir: String,
    pub working_dir: Option<String>,
    pub extra_args: String,
    pub bind_account_id: Option<String>,
    pub created_at: i64,
    pub last_launched_at: Option<i64>,
    pub last_pid: Option<u32>,
    pub running: bool,
    pub initialized: bool,
    pub is_default: bool,
}

impl InstanceProfileView {
    pub fn from_profile(profile: InstanceProfile, running: bool, initialized: bool) -> Self {
        Self {
            id: profile.id,
            name: profile.name,
            user_data_dir: profile.user_data_dir,
            working_dir: profile.working_dir,
            extra_args: profile.extra_args,
            bind_account_id: profile.bind_account_id,
            created_at: profile.created_at,
            last_launched_at: profile.last_launched_at,
            last_pid: profile.last_pid,
            running,
            initialized,
            is_default: false,
        }
    }
}
