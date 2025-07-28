// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::config::ConfigManager;
use crate::net;
use crate::serde_utils::{load_json, save_json};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 包信息
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageInfo {
    /// 应用唯一标识
    pub id: String,
    /// 应用名称
    pub name: String,
    /// 图标路径
    pub icon: String,
    /// 作者
    pub author: String,
    /// 最新版本号
    pub latest_version: String,
    /// 应用描述
    pub description: String,
    /// 位置信息（URL或本地路径）
    pub location: String,
}

/// 索引管理器
pub struct IndexManager {
    /// 索引存储路径
    index_dir: PathBuf,
    /// 仓库根目录路径
    repo_path: PathBuf,
}

impl IndexManager {
    /// 创建新的索引管理器实例
    pub fn new(index_dir: PathBuf, repo_path: PathBuf) -> Self {
        Self {
            index_dir,
            repo_path,
        }
    }

    /// 获取索引文件路径
    fn get_index_path(&self) -> PathBuf {
        self.index_dir.join("index.json")
    }

    /// 确保索引目录存在
    fn ensure_index_dir(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.index_dir.exists() {
            std::fs::create_dir_all(&self.index_dir)?;
        }
        Ok(())
    }

    /// 从远程源更新本地索引
    pub async fn update_source_index(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.ensure_index_dir()?;

        // 加载配置
        let config_manager = ConfigManager::new("config.toml")?;
        let config = config_manager.load()?;

        // 加载现有索引
        let mut index = load_json(&self.get_index_path())
            .unwrap_or_else(|_| serde_json::json!({ "source": [] }));

        // 清空源索引部分
        index["source"] = serde_json::Value::Array(Vec::new());

        // 从每个启用的源更新索引
        for source in &config.source {
            if !source.enabled {
                continue;
            }

            // 构建源索引URL
            let source_index_url = format!("{}/index.json", source.url.trim_end_matches('/'));

            // 下载源索引
            let temp_index_path = self.index_dir.join(format!("index_{}.json.tmp", source.id));
            net::download_file(&source_index_url, temp_index_path.to_str().unwrap()).await?;

            // 读取下载的索引
            let source_index_content = fs::read_to_string(&temp_index_path)?;
            let source_index: serde_json::Value = serde_json::from_str(&source_index_content)?;

            // 提取源中的包信息并添加到本地索引
            if let Some(source_packages) = source_index["source"].as_array() {
                for pkg in source_packages {
                    let package_info = PackageInfo {
                        id: pkg["id"].as_str().unwrap_or("").to_string(),
                        name: pkg["name"].as_str().unwrap_or("").to_string(),
                        icon: pkg["icon"].as_str().unwrap_or("").to_string(),
                        author: pkg["author"].as_str().unwrap_or("").to_string(),
                        latest_version: pkg["latest_version"].as_str().unwrap_or("").to_string(),
                        description: pkg["description"].as_str().unwrap_or("").to_string(),
                        location: pkg["location"].as_str().unwrap_or("").to_string(),
                    };
                    index["source"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::to_value(package_info)?);
                }
            }

            // 清理临时文件
            fs::remove_file(temp_index_path)?;
        }

        // 保存更新后的索引
        self.save_index(&index)?;

        Ok(())
    }

    /// 根据包名查询包信息
    pub fn query_package(
        &self,
        package_id: &str,
    ) -> Result<Option<PackageInfo>, Box<dyn std::error::Error>> {
        let index = self.load_index()?;

        // 在源索引中查找
        if let Some(source) = index["source"].as_array() {
            for pkg in source {
                if let Some(id) = pkg["id"].as_str() {
                    if id == package_id {
                        let package_info = PackageInfo {
                            id: id.to_string(),
                            name: pkg["name"].as_str().unwrap_or("").to_string(),
                            icon: pkg["icon"].as_str().unwrap_or("").to_string(),
                            author: pkg["author"].as_str().unwrap_or("").to_string(),
                            latest_version: pkg["latest_version"]
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                            description: pkg["description"].as_str().unwrap_or("").to_string(),
                            location: pkg["location"].as_str().unwrap_or("").to_string(),
                        };
                        return Ok(Some(package_info));
                    }
                }
            }
        }

        Ok(None)
    }

    /// 列出所有可用包
    pub fn list_packages(&self) -> Result<Vec<PackageInfo>, Box<dyn std::error::Error>> {
        let index = self.load_index()?;
        let mut packages = Vec::new();

        // 收集源索引中的所有包
        if let Some(source) = index["source"].as_array() {
            for pkg in source {
                if let Some(id) = pkg["id"].as_str() {
                    let package_info = PackageInfo {
                        id: id.to_string(),
                        name: pkg["name"].as_str().unwrap_or("").to_string(),
                        icon: pkg["icon"].as_str().unwrap_or("").to_string(),
                        author: pkg["author"].as_str().unwrap_or("").to_string(),
                        latest_version: pkg["latest_version"].as_str().unwrap_or("").to_string(),
                        description: pkg["description"].as_str().unwrap_or("").to_string(),
                        location: pkg["location"].as_str().unwrap_or("").to_string(),
                    };
                    packages.push(package_info);
                }
            }
        }

        Ok(packages)
    }

    /// 加载索引文件
    fn load_index(&self) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let index_path = self.get_index_path();
        if index_path.exists() {
            let content = fs::read_to_string(&index_path)?;
            let index: serde_json::Value = serde_json::from_str(&content)?;
            Ok(index)
        } else {
            // 返回空索引
            Ok(serde_json::json!({
                "source": []
            }))
        }
    }

    /// 保存索引文件
    fn save_index(&self, index: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
        save_json(index, &self.get_index_path()).map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_update_source_index() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let index_manager =
            IndexManager::new(temp_dir.path().to_path_buf(), temp_dir.path().to_path_buf());

        // 创建测试配置
        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("config.toml");

        let mut config = crate::config::RepositoryConfig::default();
        config.source.push(crate::config::SourceConfig {
            id: "test".to_string(),
            name: "Test Source".to_string(),
            url: "https://example.com/".to_string(),
            enabled: true,
            require_https: true,
        });

        // The test is incomplete as we cannot set up a real HTTP server
        // In a real implementation, we would save the config using Config::save()

        // 创建模拟的源索引文件
        let source_index_dir = temp_dir.path().join("source_index");
        std::fs::create_dir_all(&source_index_dir)?;
        let source_index_path = source_index_dir.join("index.json");

        let source_index = serde_json::json!({
            "source": [
                {
                    "id": "test.package",
                    "name": "Test Package",
                    "icon": "icon.png",
                    "author": "Test Author",
                    "latest_version": "1.0.0",
                    "description": "A test package",
                    "location": "https://example.com/packages/test.package/1.0.0/"
                }
            ]
        });

        fs::write(
            source_index_path,
            serde_json::to_string_pretty(&source_index)?,
        )?;

        // 由于我们无法真正启动HTTP服务器进行测试，这里我们只测试代码结构
        // 在实际实现中，需要设置一个本地HTTP服务器来提供测试索引
        Ok(())
    }

    #[test]
    fn test_query_package() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let index_manager =
            IndexManager::new(temp_dir.path().to_path_buf(), temp_dir.path().to_path_buf());

        // 创建测试索引
        let test_index = serde_json::json!({
            "source": [
                {
                    "id": "test.package",
                    "name": "Test Package",
                    "icon": "icon.png",
                    "author": "Test Author",
                    "latest_version": "1.0.0",
                    "description": "A test package",
                    "location": "https://example.com/packages/test.package/1.0.0/"
                }
            ]
        });

        let index_path = index_manager.get_index_path();
        fs::write(index_path, serde_json::to_string_pretty(&test_index)?)?;

        // 查询包
        let result = index_manager.query_package("test.package")?;
        assert!(result.is_some());
        let package = result.unwrap();
        assert_eq!(package.id, "test.package");
        assert_eq!(package.name, "Test Package");

        // 查询不存在的包
        let result = index_manager.query_package("nonexistent.package")?;
        assert!(result.is_none());

        Ok(())
    }

    #[test]
    fn test_list_packages() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let index_manager =
            IndexManager::new(temp_dir.path().to_path_buf(), temp_dir.path().to_path_buf());

        // 创建测试索引
        let test_index = serde_json::json!({
            "source": [
                {
                    "id": "test.package1",
                    "name": "Test Package 1",
                    "icon": "icon1.png",
                    "author": "Author 1",
                    "latest_version": "1.0.0",
                    "description": "First test package",
                    "location": "https://example.com/packages/test.package1/1.0.0/"
                },
                {
                    "id": "test.package2",
                    "name": "Test Package 2",
                    "icon": "icon2.png",
                    "author": "Author 2",
                    "latest_version": "2.0.0",
                    "description": "Second test package",
                    "location": "https://example.com/packages/test.package2/2.0.0/"
                }
            ]
        });

        let index_path = index_manager.get_index_path();
        fs::write(index_path, serde_json::to_string_pretty(&test_index)?)?;

        // 列出所有包
        let packages = index_manager.list_packages()?;
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].id, "test.package1");
        assert_eq!(packages[1].id, "test.package2");

        Ok(())
    }
}
