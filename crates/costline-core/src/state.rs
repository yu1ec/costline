use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub assets: BTreeMap<String, AssetState>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetState {
    /// 上一次检查时是否已经高于触发价
    #[serde(default)]
    pub above: bool,
    /// 上一次成功通知的 Unix 时间戳（秒）
    #[serde(default)]
    pub last_notified: i64,
    /// 上一次检查到的价格
    #[serde(default)]
    pub last_price: Option<f64>,
}

impl State {
    pub fn load(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        match serde_json::from_str(&text) {
            Ok(state) => state,
            Err(error) => {
                eprintln!(
                    "[costline] 状态文件 {} 解析失败，将重新开始：{error}",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建状态目录失败：{}", parent.display()))?;
        }
        let temp_path = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).context("序列化状态失败")?;
        fs::write(&temp_path, text)
            .with_context(|| format!("写入状态文件失败：{}", temp_path.display()))?;
        fs::rename(&temp_path, path)
            .with_context(|| format!("更新状态文件失败：{}", path.display()))?;
        Ok(())
    }
}
