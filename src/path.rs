// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::fs;
use std::path::{Path, PathBuf};

/// 展开路径中的特殊符号
/// 目前支持 `~` 符号展开为用户的主目录
pub fn expand_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    if path.starts_with("~") {
        if path == Path::new("~") {
            // 只有 ~，直接返回主目录
            dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"))
        } else {
            // ~ 开头但后面还有内容，如 ~/documents
            let mut p = PathBuf::new();
            if let Some(home) = dirs::home_dir() {
                p.push(home);
            }
            p.push(
                path.strip_prefix("~/")
                    .unwrap_or_else(|_| path.strip_prefix("~").unwrap()),
            );
            p
        }
    } else {
        // 不以 ~ 开头，返回原路径
        path.to_path_buf()
    }
}

/// 递归创建目录
/// 如果目录已存在，不会返回错误
pub fn create_dir_all<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

/// 规范化路径
/// - 展开 `.` 和 `..` 部分
/// - 统一使用正斜杠（在 Unix 系统上）
/// - 移除重复的斜杠
/// - 确保路径格式一致
pub fn normalize_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    let mut components = Vec::new();
    let is_absolute = path.is_absolute();

    for component in path.components() {
        match component {
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                components.push(component.as_os_str().to_owned());
            }
            std::path::Component::CurDir => {
                // 忽略当前目录 (.)
                continue;
            }
            std::path::Component::ParentDir => {
                // 处理父目录 (..)
                // 如果组件列表为空或最后一个组件是根目录/前缀，则保留 ..
                if components.is_empty()
                    || components
                        .last()
                        .map(|c| c == std::path::Component::RootDir.as_os_str())
                        .unwrap_or(false)
                {
                    components.push(component.as_os_str().to_owned());
                } else {
                    // 否则，移除上一个组件
                    components.pop();
                }
            }
            std::path::Component::Normal(name) => {
                components.push(name.to_owned());
            }
        }
    }

    // 重建路径
    let mut result = PathBuf::new();
    for component in &components {
        result.push(component);
    }

    // 如果原始路径以分隔符结尾，保持这个特性
    if path
        .to_str()
        .map(|s| s.ends_with('/') || s.ends_with('\\'))
        .unwrap_or(false)
    {
        result.push("");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_expand_path_home() {
        let home = dirs::home_dir().unwrap();
        let expanded = expand_path("~");
        assert_eq!(expanded, home);
    }

    #[test]
    fn test_expand_path_home_with_subdir() {
        let home = dirs::home_dir().unwrap();
        let expanded = expand_path("~/documents");
        let mut expected = home.clone();
        expected.push("documents");
        assert_eq!(expanded, expected);
    }

    #[test]
    fn test_expand_path_relative() {
        let expanded = expand_path("relative/path");
        assert_eq!(expanded, Path::new("relative/path"));
    }

    #[test]
    fn test_create_dir_all() {
        let temp_dir = std::env::temp_dir().join("pageos-pkgr-test");
        let test_path = temp_dir.join("a/b/c");

        // 清理测试环境
        let _ = std::fs::remove_dir_all(&temp_dir);

        // 创建目录
        let result = create_dir_all(&test_path);
        assert!(result.is_ok());

        // 验证目录存在
        assert!(test_path.exists() && test_path.is_dir());

        // 再次创建，应该成功（目录已存在）
        let result2 = create_dir_all(&test_path);
        assert!(result2.is_ok());

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_normalize_path_simple() {
        let normalized = normalize_path("/a/b/c");
        assert_eq!(normalized, Path::new("/a/b/c"));
    }

    #[test]
    fn test_normalize_path_current_dir() {
        let normalized = normalize_path("/a/./b");
        assert_eq!(normalized, Path::new("/a/b"));
    }

    #[test]
    fn test_normalize_path_parent_dir() {
        let normalized = normalize_path("/a/b/../c");
        assert_eq!(normalized, Path::new("/a/c"));
    }

    #[test]
    fn test_normalize_path_multiple_dots() {
        let normalized = normalize_path("/a/./b/../c/./d");
        assert_eq!(normalized, Path::new("/a/c/d"));
    }

    #[test]
    fn test_normalize_path_root_dot_dot() {
        let normalized = normalize_path("/../a/b");
        assert_eq!(normalized, Path::new("/../a/b"));
    }

    #[test]
    fn test_normalize_path_relative() {
        let normalized = normalize_path("a/./b/../c");
        assert_eq!(normalized, Path::new("a/c"));
    }

    #[test]
    fn test_normalize_path_trailing_slash() {
        let normalized = normalize_path("/a/b/");
        assert_eq!(normalized, Path::new("/a/b/"));
    }
}

/// 获取配置文件路径
///
/// # 返回值
///
/// 返回 `PathBuf`，表示配置文件的路径
///
/// # 功能特性
///
/// * 遵循 XDG 基础目录规范
/// * 在 Linux 系统上返回 `$XDG_CONFIG_HOME/pageos-pkgr/config.toml` 或默认的 `~/.config/pageos-pkgr/config.toml`
/// * 确保路径格式正确
pub fn get_config_path() -> PathBuf {
    // 尝试从环境变量获取 XDG_CONFIG_HOME
    if let Ok(xdg_config_home) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg_config_home.is_empty() {
            return PathBuf::from(xdg_config_home)
                .join("pageos-pkgr")
                .join("config.toml");
        }
    }

    // 如果 XDG_CONFIG_HOME 未设置或为空，使用默认的 ~/.config
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("pageos-pkgr")
        .join("config.toml")
}

/// 获取缓存目录路径
///
/// # 返回值
///
/// 返回 `PathBuf`，表示缓存目录的路径
///
/// # 功能特性
///
/// * 遵循 XDG 基础目录规范
/// * 在 Linux 系统上返回 `$XDG_CACHE_HOME/pageos-pkgr` 或默认的 `~/.cache/pageos-pkgr`
/// * 确保目录存在
pub fn get_cache_dir() -> PathBuf {
    // 尝试从环境变量获取 XDG_CACHE_HOME
    if let Ok(xdg_cache_home) = std::env::var("XDG_CACHE_HOME") {
        if !xdg_cache_home.is_empty() {
            return PathBuf::from(xdg_cache_home).join("pageos-pkgr");
        }
    }

    // 如果 XDG_CACHE_HOME 未设置或为空，使用默认的 ~/.cache
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("~/.cache"))
        .join("pageos-pkgr")
}
