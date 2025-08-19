// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::serde_utils::{load_toml, save_toml};

/// 源配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SourceConfig {
    /// 唯一标识符（用于命令行操作，如 `pageos-pkgr repo install pageos:pageos-settings-manager`）
    pub id: String,
    /// 显示名称
    pub name: String,
    /// 仓库根 URL（必须以 / 结尾）或是本地目录如：/home/user/repo/another/
    pub url: String,
    /// 是否启用此源
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 是否强制使用 HTTPS
    #[serde(default = "default_require_https")]
    pub require_https: bool,
}

/// 仓库配置
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepositoryConfig {
    /// 缓存目录（用于下载临时文件等）默认值 `$HOME/.cache/pageos-pkgr/cache`
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,
    /// 软件源列表
    #[serde(default)]
    pub source: Vec<SourceConfig>,
}

impl Default for RepositoryConfig {
    fn default() -> Self {
        Self {
            cache_dir: default_cache_dir(),
            source: Vec::new(),
        }
    }
}

/// 配置管理模块
pub struct ConfigManager {
    /// 配置文件路径
    config_path: String,
}

impl ConfigManager {
    /// 创建新的配置管理器实例
    pub fn new<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_path_str = config_path
            .as_ref()
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("配置路径包含无效的 UTF-8 字符"))?
            .to_string();

        Ok(Self {
            config_path: config_path_str,
        })
    }

    /// 加载配置
    ///
    /// 读取配置文件，解析并验证配置。
    pub fn load(&self) -> Result<RepositoryConfig> {
        // 检查配置文件是否存在
        if !Path::new(&self.config_path).exists() {
            // 如果文件不存在，创建默认配置
            let default_config = RepositoryConfig::default();
            self.save(&default_config)
                .with_context(|| format!("无法创建默认配置文件: {}", self.config_path))?;
            return Ok(default_config);
        }

        // 解析 TOML 配置
        let config: RepositoryConfig =
            load_toml(Path::new(&self.config_path))
                .with_context(|| format!("无法读取或解析配置文件: {}", self.config_path))?;

        // 验证配置的有效性
        self.validate_config(&config)
            .with_context(|| "配置验证失败")?;

        Ok(config)
    }

    /// 保存配置
    ///
    /// 将配置对象序列化为 TOML 格式并写入文件。
    pub fn save(&self, config: &RepositoryConfig) -> Result<()> {
        // 验证配置的有效性
        self.validate_config(config)
            .with_context(|| "配置验证失败")?;

        // 确保配置目录存在
        if let Some(parent) = Path::new(&self.config_path).parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建配置目录: {}", parent.display()))?;
        }

        // 写入文件
        save_toml(config, Path::new(&self.config_path))
            .with_context(|| format!("无法保存配置文件: {}", self.config_path))?;

        // 设置文件权限（如果可能）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.config_path)
                .with_context(|| format!("无法获取配置文件元数据: {}", self.config_path))?
                .permissions();
            perms.set_mode(0o600); // 仅用户可读写
            fs::set_permissions(&self.config_path, perms)
                .with_context(|| format!("无法设置配置文件权限: {}", self.config_path))?;
        }

        Ok(())
    }

    /// 管理软件源
    ///
    /// 添加新的软件源到配置中。
    pub fn add_source(&self, source: SourceConfig) -> Result<()> {
        let mut config = self.load().with_context(|| "无法加载现有配置")?;

        // 检查源ID是否已存在
        if config.source.iter().any(|s| s.id == source.id) {
            return Err(anyhow::anyhow!("软件源ID '{}' 已存在", source.id));
        }

        config.source.push(source);
        self.save(&config).with_context(|| "无法保存更新后的配置")?;

        Ok(())
    }

    /// 启用软件源
    pub fn enable_source(&self, source_id: &str) -> Result<()> {
        let mut config = self.load().with_context(|| "无法加载现有配置")?;

        let source = config
            .source
            .iter_mut()
            .find(|s| s.id == source_id)
            .ok_or_else(|| anyhow::anyhow!("未找到软件源: {}", source_id))?;

        source.enabled = true;
        self.save(&config).with_context(|| "无法保存更新后的配置")?;

        Ok(())
    }

    /// 禁用软件源
    pub fn disable_source(&self, source_id: &str) -> Result<()> {
        let mut config = self.load().with_context(|| "无法加载现有配置")?;

        let source = config
            .source
            .iter_mut()
            .find(|s| s.id == source_id)
            .ok_or_else(|| anyhow::anyhow!("未找到软件源: {}", source_id))?;

        source.enabled = false;
        self.save(&config).with_context(|| "无法保存更新后的配置")?;

        Ok(())
    }

    /// 删除软件源
    pub fn remove_source(&self, source_id: &str) -> Result<()> {
        let mut config = self.load().with_context(|| "无法加载现有配置")?;

        let initial_len = config.source.len();
        config.source.retain(|s| s.id != source_id);

        if config.source.len() == initial_len {
            return Err(anyhow::anyhow!("未找到软件源: {}", source_id));
        }

        self.save(&config).with_context(|| "无法保存更新后的配置")?;

        Ok(())
    }

    /// 更新软件源信息
    pub fn update_source(&self, source_id: &str, updated_source: SourceConfig) -> Result<()> {
        let mut config = self.load().with_context(|| "无法加载现有配置")?;

        let source = config
            .source
            .iter_mut()
            .find(|s| s.id == source_id)
            .ok_or_else(|| anyhow::anyhow!("未找到软件源: {}", source_id))?;

        // 保留原有的ID
        let old_id = source.id.clone();
        *source = updated_source;
        source.id = old_id;

        self.save(&config).with_context(|| "无法保存更新后的配置")?;

        Ok(())
    }

    /// 验证配置的有效性
    fn validate_config(&self, config: &RepositoryConfig) -> Result<()> {
        // 检查源ID是否唯一
        let mut source_ids = HashMap::new();
        for source in &config.source {
            if source_ids.insert(&source.id, ()).is_some() {
                return Err(anyhow::anyhow!("软件源ID '{}' 重复", source.id));
            }
        }

        // 验证URL格式
        for source in &config.source {
            if source.url.is_empty() {
                return Err(anyhow::anyhow!("软件源 '{}' 的URL不能为空", source.id));
            }

            // 如果不是本地路径，检查是否为有效URL
            if !source.url.starts_with("http://")
                && !source.url.starts_with("https://")
                && !source.url.starts_with("/")
            {
                return Err(anyhow::anyhow!(
                    "软件源 '{}' 的URL格式无效: {}",
                    source.id,
                    source.url
                ));
            }

            // 如果要求HTTPS，确保URL以https://开头
            if source.require_https && !source.url.starts_with("https://") {
                return Err(anyhow::anyhow!(
                    "软件源 '{}' 要求HTTPS，但URL不是https://开头",
                    source.id
                ));
            }
        }

        Ok(())
    }
}

// 默认值函数
fn default_cache_dir() -> String {
    use dirs::cache_dir;
    if let Some(cache_dir) = cache_dir() {
        cache_dir
            .join("pageos-pkgr")
            .join("cache")
            .to_string_lossy()
            .to_string()
    } else {
        // Fallback to home directory
        use dirs::home_dir;
        if let Some(home) = home_dir() {
            home.join(".cache")
                .join("pageos-pkgr")
                .join("cache")
                .to_string_lossy()
                .to_string()
        } else {
            // Last resort
            "/tmp/pageos-pkgr-cache".to_string()
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_require_https() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = RepositoryConfig::default();
        assert!(!config.cache_dir.is_empty());
        assert!(config.source.is_empty());
    }

    #[test]
    fn test_config_serialization() -> Result<()> {
        let mut config = RepositoryConfig::default();
        config.cache_dir = "/tmp/test-cache".to_string();

        let source = SourceConfig {
            id: "test".to_string(),
            name: "Test Source".to_string(),
            url: "https://example.com/".to_string(),
            enabled: true,
            require_https: true,
        };
        config.source.push(source);

        let toml_string = toml::to_string_pretty(&config)?;
        let parsed_config: RepositoryConfig = toml::from_str(&toml_string)?;

        assert_eq!(config.cache_dir, parsed_config.cache_dir);
        assert_eq!(config.source.len(), parsed_config.source.len());
        assert_eq!(config.source[0].id, parsed_config.source[0].id);
        assert_eq!(config.source[0].name, parsed_config.source[0].name);
        assert_eq!(config.source[0].url, parsed_config.source[0].url);
        assert_eq!(config.source[0].enabled, parsed_config.source[0].enabled);
        assert_eq!(
            config.source[0].require_https,
            parsed_config.source[0].require_https
        );

        Ok(())
    }

    #[test]
    fn test_config_manager_load_save() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.toml");

        let manager = ConfigManager::new(&config_path)?;

        // 测试加载默认配置（文件不存在时）
        let config = manager.load()?;
        assert_eq!(config.source.len(), 0);

        // 修改配置并保存
        let mut config = RepositoryConfig::default();
        config.cache_dir = "/tmp/custom-cache".to_string();

        let source = SourceConfig {
            id: "test".to_string(),
            name: "Test Source".to_string(),
            url: "https://example.com/".to_string(),
            enabled: true,
            require_https: true,
        };
        config.source.push(source);

        manager.save(&config)?;

        // 验证文件已创建
        assert!(config_path.exists());

        // 重新加载并验证
        let loaded_config = manager.load()?;
        assert_eq!(loaded_config.cache_dir, "/tmp/custom-cache");
        assert_eq!(loaded_config.source.len(), 1);
        assert_eq!(loaded_config.source[0].id, "test");

        Ok(())
    }

    #[test]
    fn test_config_manager_add_source() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.toml");

        let manager = ConfigManager::new(&config_path)?;
        let source = SourceConfig {
            id: "test".to_string(),
            name: "Test Source".to_string(),
            url: "https://example.com/".to_string(),
            enabled: true,
            require_https: true,
        };

        manager.add_source(source)?;

        let config = manager.load()?;
        assert_eq!(config.source.len(), 1);
        assert_eq!(config.source[0].id, "test");

        // 尝试添加重复ID的源
        let duplicate_source = SourceConfig {
            id: "test".to_string(),
            name: "Duplicate Source".to_string(),
            url: "https://duplicate.com/".to_string(),
            enabled: true,
            require_https: true,
        };

        let result = manager.add_source(duplicate_source);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_config_manager_enable_disable_source() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.toml");

        let manager = ConfigManager::new(&config_path)?;

        // 添加源
        let source = SourceConfig {
            id: "test".to_string(),
            name: "Test Source".to_string(),
            url: "https://example.com/".to_string(),
            enabled: false, // 初始禁用
            require_https: true,
        };
        manager.add_source(source)?;

        // 启用源
        manager.enable_source("test")?;
        let config = manager.load()?;
        assert!(config.source[0].enabled);

        // 禁用源
        manager.disable_source("test")?;
        let config = manager.load()?;
        assert!(!config.source[0].enabled);

        // 尝试操作不存在的源
        let result = manager.enable_source("nonexistent");
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_config_manager_remove_source() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.toml");

        let manager = ConfigManager::new(&config_path)?;

        // 添加源
        let source = SourceConfig {
            id: "test".to_string(),
            name: "Test Source".to_string(),
            url: "https://example.com/".to_string(),
            enabled: true,
            require_https: true,
        };
        manager.add_source(source)?;

        // 验证源存在
        let config = manager.load()?;
        assert_eq!(config.source.len(), 1);

        // 删除源
        manager.remove_source("test")?;
        let config = manager.load()?;
        assert_eq!(config.source.len(), 0);

        // 尝试删除不存在的源
        let result = manager.remove_source("nonexistent");
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_config_manager_update_source() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.toml");

        let manager = ConfigManager::new(&config_path)?;

        // 添加源
        let source = SourceConfig {
            id: "test".to_string(),
            name: "Test Source".to_string(),
            url: "https://example.com/".to_string(),
            enabled: true,
            require_https: true,
        };
        manager.add_source(source)?;

        // 更新源
        let mut updated_source = SourceConfig {
            id: "updated".to_string(), // 这个ID会被忽略，保留原来的ID
            name: "Updated Source".to_string(),
            url: "https://updated.com/".to_string(),
            enabled: false,
            require_https: false,
        };
        manager.update_source("test", updated_source)?;

        // 验证更新后的源
        let config = manager.load()?;
        assert_eq!(config.source.len(), 1);
        assert_eq!(config.source[0].id, "test"); // ID应该保持不变
        assert_eq!(config.source[0].name, "Updated Source");
        assert_eq!(config.source[0].url, "https://updated.com/");
        assert!(!config.source[0].enabled);
        assert!(!config.source[0].require_https);

        // 尝试更新不存在的源
        let result = manager.update_source(
            "nonexistent",
            SourceConfig {
                id: "dummy".to_string(),
                name: "Dummy".to_string(),
                url: "https://dummy.com/".to_string(),
                enabled: true,
                require_https: true,
            },
        );
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_config_validation() -> Result<()> {
        // 测试重复的源ID
        let mut config = RepositoryConfig::default();
        config.source.push(SourceConfig {
            id: "duplicate".to_string(),
            name: "First".to_string(),
            url: "https://example.com/".to_string(),
            enabled: true,
            require_https: true,
        });
        config.source.push(SourceConfig {
            id: "duplicate".to_string(),
            name: "Second".to_string(),
            url: "https://example.org/".to_string(),
            enabled: true,
            require_https: true,
        });

        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.toml");
        let manager = ConfigManager::new(&config_path)?;
        let result = manager.save(&config);
        assert!(result.is_err());

        // 测试无效URL
        let mut config = RepositoryConfig::default();
        config.source.push(SourceConfig {
            id: "invalid".to_string(),
            name: "Invalid URL".to_string(),
            url: "not-a-url".to_string(),
            enabled: true,
            require_https: true,
        });

        let result = manager.save(&config);
        assert!(result.is_err());

        // 测试要求HTTPS但使用HTTP
        let mut config = RepositoryConfig::default();
        config.source.push(SourceConfig {
            id: "http-but-require-https".to_string(),
            name: "HTTP with HTTPS required".to_string(),
            url: "http://example.com/".to_string(),
            enabled: true,
            require_https: true,
        });

        let result = manager.save(&config);
        assert!(result.is_err());

        Ok(())
    }
}
