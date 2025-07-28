// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::Result;
use std::fs;
use std::path::Path;

/// 从文件加载 TOML 配置
pub fn load_toml<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> Result<T> {
    let content = fs::read_to_string(path)?;
    let value: T = toml::from_str(&content)?;
    Ok(value)
}

/// 保存 TOML 配置到文件
pub fn save_toml<T: serde::Serialize>(value: &T, path: &Path) -> Result<()> {
    let content = toml::to_string_pretty(value)?;
    fs::write(path, content)?;
    Ok(())
}

/// 从文件加载 JSON 配置
pub fn load_json<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> Result<T> {
    let content = fs::read_to_string(path)?;
    let value: T = serde_json::from_str(&content)?;
    Ok(value)
}

/// 保存 JSON 配置到文件
pub fn save_json<T: serde::Serialize>(value: &T, path: &Path) -> Result<()> {
    let content = serde_json::to_string_pretty(value)?;
    fs::write(path, content)?;
    Ok(())
}