// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::config::{ConfigManager, RepositoryConfig};
use crate::fsxg;
use crate::metadata::PackageMetadata;
use crate::net;
use crate::path::{expand_path, get_cache_dir};
use crate::serde_utils::{load_json, save_json};
use crate::transaction::Transaction;
use crate::crypto;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// 仓库管理状态
pub struct RepoManager {
    /// 仓库根目录
    repo_path: PathBuf,
    /// 仓库配置
    config: RepositoryConfig,
    /// 事务管理器
    _transaction: Option<Transaction>,
}

/// 仓库索引结构
#[derive(Serialize, Deserialize, Debug)]
pub struct RepositoryIndex {
    /// 已安装的包列表
    pub packages: Vec<PackageInfo>,
    /// 软件源中的包列表
    pub source: Vec<PackageInfo>,
}

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
    /// 位置信息（本地路径或URL）
    pub location: String,
}

impl RepoManager {
    /// 初始化仓库
    pub fn init<P: AsRef<Path>>(repo_path: P) -> Result<Self> {
        let repo_path = expand_path(repo_path);
        let config_path = repo_path.join("config.toml");

        // 创建仓库目录结构
        fsxg::create_directory(repo_path.join("packages"))
            .with_context(|| format!("无法创建 packages 目录: {}", repo_path.display()))?;

        // 初始化配置文件
        let config = RepositoryConfig::default();
        ConfigManager::new(&config_path).and_then(|cm| cm.save(&config))?;

        // 初始化索引文件
        let index = RepositoryIndex {
            packages: Vec::new(),
            source: Vec::new(),
        };
        save_json(&index, &repo_path.join("index.json"))?;

        Ok(Self {
            repo_path,
            config,
            _transaction: None,
        })
    }

    /// 创建新仓库
    pub fn new<P: AsRef<Path>>(repo_name: &str, base_dir: P) -> Result<Self> {
        let repo_path = base_dir.as_ref().join(repo_name);
        Self::init(repo_path)
    }

    /// 打开已有仓库
    pub fn open<P: AsRef<Path>>(repo_path: P) -> Result<Self> {
        let repo_path = expand_path(repo_path);
        let config_path = repo_path.join("config.toml");

        // 确保仓库目录存在
        if !repo_path.exists() {
            return Err(anyhow!("仓库目录不存在: {}", repo_path.display()));
        }

        // 安全加载配置（仅在文件不存在时创建默认配置）
        let config = ConfigManager::new(&config_path)?.load()?;

        Ok(Self {
            repo_path,
            config,
            _transaction: None,
        })
    }

    /// 清理仓库
    pub fn clean(&mut self) -> Result<()> {
        // 清空下载缓存
        let cache_dir = get_cache_dir();
        if cache_dir.exists() {
            fsxg::remove_directory(&cache_dir)?;
        }

        // 清理旧版本（保留最新两个版本）
        for package_dir in fs::read_dir(self.repo_path.join("packages"))? {
            let package_dir = package_dir?.path();
            if package_dir.is_dir() {
                clean_old_versions(&package_dir)?;
            }
        }

        // 清空source索引
        let mut index: RepositoryIndex = load_json(&self.repo_path.join("index.json"))?;
        index.source.clear();
        save_json(&index, &self.repo_path.join("index.json"))?;

        Ok(())
    }

    /// 更新索引的 source 部分
    ///
    /// 遍历所有启用的软件源，从每个源获取索引，并合并到本地索引的 source 部分。
    /// 合并策略：对于同一个包 ID，后处理的源会覆盖先处理的源。
    pub async fn update_source_index(&mut self) -> Result<()> {
        // 获取索引文件路径
        let index_path = self.repo_path.join("index.json");

        // 加载现有索引（如果存在）
        let mut local_index: RepositoryIndex = if index_path.exists() {
            load_json(&index_path)?
        } else {
            RepositoryIndex {
                packages: Vec::new(),
                source: Vec::new(),
            }
        };

        // 创建一个 HashMap 来合并包（包ID -> PackageInfo）
        use std::collections::HashMap;
        let mut merged_source = HashMap::new();

        // 遍历所有启用的软件源
        for source in &self.config.source {
            if !source.enabled {
                continue;
            }

            // 构建索引 URL
            let index_url = format!("{}/index.json", source.url.trim_end_matches('/'));

            // 获取索引（返回的是 serde_json::Value）
            let source_index_value = net::fetch_index(&index_url)
                .await
                .map_err(|e| anyhow::anyhow!("从源 {} 获取索引失败: {}", source.id, e))?;

            // 尝试将 Value 转换为 RepositoryIndex
            let source_index: RepositoryIndex = serde_json::from_value(source_index_value)
                .map_err(|e| anyhow::anyhow!("解析源 {} 的索引失败: {}", source.id, e))?;

            // 将源索引中的包合并到 HashMap，并将相对路径转换为绝对路径
            for mut package in source_index.packages {
                if package.location.starts_with("./packages/") {
                    let package_path = &package.location["./packages/".len()..];
                    package.location = format!(
                        "{}/packages/{}",
                        source.url.trim_end_matches('/'),
                        package_path
                    );
                }
                merged_source.insert(package.id.clone(), package);
            }
        }

        // 将 HashMap 中的值转换为 Vec，作为新的 source 部分
        local_index.source = merged_source.into_values().collect();

        // 保存更新后的索引
        save_json(&local_index, &index_path)?;

        Ok(())
    }

    /// 添加包到仓库
    pub fn add_package<P: AsRef<Path>>(&mut self, package_path: P) -> Result<()> {
        let package_path = expand_path(package_path);
        let metadata_path = package_path.join("metadata.json");
        let metadata: PackageMetadata = load_json(&metadata_path)?;

        // 创建包目标目录
        let package_dir = self
            .repo_path
            .join("packages")
            .join(&metadata.id)
            .join(&metadata.version);

        fsxg::create_directory(&package_dir)?;

        // 确保 metadata.all_files 至少包含一项
        if metadata.all_files.is_empty() {
            return Err(anyhow!("metadata.all_files 必须至少包含一项"));
        }

        // 确保 metadata.all_files 列表中的文件的 SHA256 值验证成功
        for (file_path, expected_hash) in &metadata.all_files {
            let src_path = package_path.join(file_path);
            if !src_path.exists() {
                return Err(anyhow!("文件不存在: {}", src_path.display()));
            }
            if src_path.is_dir() {
                return Err(anyhow!("路径是目录，不是文件: {}", src_path.display()));
            }
            let actual_hash = crypto::file_hash(src_path.to_str().unwrap())?;
            if actual_hash != *expected_hash {
                return Err(anyhow!(
                    "文件哈希不匹配: {} (预期: {}, 实际: {})",
                    file_path,
                    expected_hash,
                    actual_hash
                ));
            }
        }

        // 复制所有文件
        for file_path in metadata.all_files.keys() {
            let src_path = package_path.join(file_path);
            let dest_path = package_dir.join(file_path);

            if let Some(parent) = dest_path.parent() {
                fsxg::create_directory(parent)?;
            }

            fs::copy(src_path, dest_path)?;
        }

        // 复制 metadata.json 文件
        let src_metadata_path = package_path.join("metadata.json");
        let dest_metadata_path = package_dir.join("metadata.json");
        fs::copy(src_metadata_path, dest_metadata_path)?;

        // 更新版本历史
        update_version_history(&metadata.id, &metadata.version, &self.repo_path)?;

        // 更新索引
        update_package_index(&metadata, &package_dir, &self.repo_path.join("index.json"))?;

        Ok(())
    }

    /// 安装软件包
    pub async fn install_package(
        &mut self,
        package_spec: &str,
        version: Option<&str>,
    ) -> Result<()> {
        // 解析 package_spec，支持三种格式：
        // 1. package_id (使用默认源和最新版本)
        // 2. source:package_id (使用指定源和最新版本)
        // 3. source:package_id:version (使用指定源和版本)
        let parts: Vec<&str> = package_spec.split(':').collect();

        let (source_id, package_id, final_version) = match parts.len() {
            1 => {
                // 只提供了包ID，使用默认源
                let default_source = self
                    .config
                    .source
                    .first()
                    .map(|s| s.id.as_str())
                    .unwrap_or("default");
                (default_source, parts[0], version.unwrap_or("latest"))
            }
            2 => {
                // 提供了 source:package_id
                (parts[0], parts[1], version.unwrap_or("latest"))
            }
            3 => {
                // 提供了完整的 source:package_id:version
                // 覆盖传入的 version 参数
                (parts[0], parts[1], parts[2])
            }
            _ => {
                return Err(anyhow!("错误: 请使用 source:package:version 格式"));
            }
        };

        // 查找软件源配置
        let source = self
            .config
            .source
            .iter()
            .find(|s| s.id == source_id)
            .ok_or_else(|| anyhow!("未找到软件源: {}", source_id))?;

        // 根据源和版本获取包元数据 URL 【费案】实现获取不同源不同版本的元数据
        // let metadata_url = format!(
        //     "{}{}/{}/metadata.json",
        //     source.url, package_id, final_version
        // );

        // 从索引中获取软件包的 location 值
        let index_path = self.repo_path.join("index.json");
        let index: RepositoryIndex = load_json(&index_path)?;

        // 在源索引中查找包
        let package_info = index
            .source
            .iter()
            .find(|p| p.id == package_id)
            .ok_or_else(|| anyhow!("未在索引中找到包: {}", package_id))?;

        // 构建元数据 URL
        let metadata_url = format!(
            "{}/metadata.json",
            package_info.location.trim_end_matches('/')
        );

        // 下载元数据
        let metadata_path = get_cache_dir().join("metadata.json");
        let metadata_str = metadata_path
            .to_str()
            .ok_or_else(|| anyhow!("无效的缓存路径"))?;
        net::download_file(&metadata_url, metadata_str)
            .await
            .map_err(|e| anyhow!("下载失败: {}", e))?;
        let metadata_content = fs::read(&metadata_path)?;
        let metadata: PackageMetadata = serde_json::from_slice(&metadata_content)?;

        // 创建包目录
        let package_dir = self
            .repo_path
            .join("packages")
            .join(&metadata.id)
            .join(&metadata.version);

        fsxg::create_directory(&package_dir)?;

        // 下载并验证所有文件
        for (file_path, expected_hash) in &metadata.all_files {
            let file_url = format!(
                "{}packages/{}/{}/{}",
                source.url, package_id, metadata.version, file_path
            );

            let dest_path = package_dir.join(file_path);
            if let Some(parent) = dest_path.parent() {
                fsxg::create_directory(parent)?;
            }

            // 添加日志调试
            eprintln!("下载文件: {}", &file_url);
            eprintln!("目标路径: {:?}", &dest_path);

            let dest_str = dest_path
                .to_str()
                .ok_or_else(|| anyhow!("无效的文件路径"))?;
            net::download_file(&file_url, dest_str)
                .await
                .map_err(|e| anyhow!("下载失败: {}", e))?;

            // 验证文件哈希
            let actual_hash = crypto::file_hash(dest_str)?;
            if &actual_hash != expected_hash {
                return Err(anyhow!(
                    "文件哈希不匹配: {} (预期: {}, 实际: {})",
                    file_path,
                    expected_hash,
                    actual_hash
                ));
            }
        }

        // 复制 metadata.json 文件
        let src_metadata_path = metadata_path; // 缓存目录中的 metadata.json
        let dest_metadata_path = package_dir.join("metadata.json");
        fs::copy(src_metadata_path, dest_metadata_path)?;

        // 更新版本历史
        update_version_history(&metadata.id, &metadata.version, &self.repo_path)?;

        // 更新索引
        update_package_index(&metadata, &package_dir, &self.repo_path.join("index.json"))?;

        Ok(())
    }

    /// 卸载软件包
    pub fn remove_package(&mut self, package_id: &str, version: Option<&str>) -> Result<()> {
        let package_dir = self.repo_path.join("packages").join(package_id);

        if let Some(version) = version {
            // 移除特定版本
            let version_dir = package_dir.join(version);
            if version_dir.exists() {
                fsxg::remove_directory(&version_dir)?;
            }
        } else {
            // 移除整个包
            if package_dir.exists() {
                fsxg::remove_directory(&package_dir)?;
            }
        }

        // 更新版本历史
        if let Some(version) = version {
            remove_version_from_history(package_id, version, &self.repo_path)?;
        } else {
            remove_package_history(package_id, &self.repo_path)?;
        }

        // 更新索引
        remove_package_from_index(package_id, version, &self.repo_path.join("index.json"))?;

        Ok(())
    }

    /// 升级软件包
    pub async fn upgrade_package(&mut self, package_id: &str) -> Result<()> {
        // 获取当前安装的最新版本
        let history_path = self
            .repo_path
            .join("packages")
            .join(package_id)
            .join("versions.txt");

        let versions = read_version_history(&history_path)?;
        let current_version = versions
            .last()
            .ok_or_else(|| anyhow!("没有安装的版本: {}", package_id))?
            .clone();

        // 从索引中获取软件源中的最新版本和源信息
        let index_path = self.repo_path.join("index.json");
        let index: RepositoryIndex = load_json(&index_path)?;

        let remote_pkg = index
            .source
            .iter()
            .find(|p| p.id == package_id)
            .ok_or_else(|| anyhow!("未在软件源中找到包: {}", package_id))?;

        let latest_version = remote_pkg.latest_version.clone();

        // 查找软件源 ID（使用第一个包含该包的启用源）
        let source_id = self
            .config
            .source
            .iter()
            .find(|s| s.enabled && index.source.iter().any(|p| p.id == package_id))
            .map(|s| s.id.clone())
            .ok_or_else(|| anyhow!("没有找到包含 {} 的启用源", package_id))?;

        // 比较版本
        if latest_version != current_version {
            // 安装新版本
            self.install_package(
                &format!("{source_id}:{package_id}"),
                Some(&latest_version),
            )
            .await?;
        }

        Ok(())
    }

    /// 同步仓库
    pub async fn sync_repository(&mut self, source_id: &str, mirror: bool) -> Result<()> {
        // 获取软件源配置
        let source = self
            .config
            .source
            .iter()
            .find(|s| s.id == source_id)
            .ok_or_else(|| anyhow!("未找到软件源: {}", source_id))?;

        if mirror {
            // 镜像同步
            net::mirror_sync(
                &source.url,
                &self.repo_path.join("packages").to_string_lossy(),
                source.enabled,
                source.require_https,
            )
            .await
            .map_err(|e| anyhow!("镜像同步失败: {}", e))?;
        } else {
            // 增量同步 (简化实现)
            let index_url = format!("{}/index.json", source.url.trim_end_matches('/'));
            let remote_index = net::fetch_index(&index_url)
                .await
                .map_err(|e| anyhow!("获取索引失败: {}", e))?;

            // 更新本地索引
            let mut local_index: RepositoryIndex = load_json(&self.repo_path.join("index.json"))?;
            local_index.source = serde_json::from_value(remote_index["source"].clone())?;
            save_json(&local_index, &self.repo_path.join("index.json"))?;
        }

        Ok(())
    }

    /// 更新本地索引
    ///
    /// 扫描 packages/ 目录下的所有已安装包，并更新 index.json 文件中的 packages 部分
    pub fn update_local_index(&mut self) -> Result<()> {
        // 获取索引文件路径
        let index_path = self.repo_path.join("index.json");

        // 加载现有索引
        let mut index: RepositoryIndex = if index_path.exists() {
            load_json(&index_path)?
        } else {
            RepositoryIndex {
                packages: Vec::new(),
                source: Vec::new(),
            }
        };

        // 清空 packages 部分
        index.packages.clear();

        // 扫描 packages/ 目录
        let packages_dir = self.repo_path.join("packages");
        if packages_dir.exists() && packages_dir.is_dir() {
            for entry in fs::read_dir(packages_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let package_dir = entry.path();
                    let package_name = entry.file_name().to_string_lossy().to_string();

                    // 获取最新版本的目录
                    let versions = read_version_history(&package_dir.join("versions.txt"))?;
                    if let Some(latest_version) = versions.last() {
                        let version_dir = package_dir.join(latest_version);
                        if version_dir.exists() && version_dir.is_dir() {
                            // 读取 metadata.json
                            let metadata_path = version_dir.join("metadata.json");
                            if metadata_path.exists() {
                                let metadata: PackageMetadata = load_json(&metadata_path)?;

                                // 创建包信息
                                let package_info = PackageInfo {
                                    id: metadata.id.clone(),
                                    name: metadata.name.clone(),
                                    icon: metadata.icon.clone(),
                                    author: metadata.author.clone(),
                                    latest_version: metadata.version.clone(),
                                    description: metadata.description.clone(),
                                    location: version_dir.to_string_lossy().to_string(),
                                };

                                // 添加到索引
                                index.packages.push(package_info);
                            }
                        }
                    }
                }
            }
        }

        // 保存更新后的索引
        save_json(&index, &index_path)?;

        Ok(())
    }
}

/// 清理旧版本 (保留最新的2个版本)
fn clean_old_versions(package_dir: &Path) -> Result<()> {
    let mut versions: Vec<String> = fs::read_dir(package_dir)?
        .filter_map(|entry| entry.ok().and_then(|e| e.file_name().into_string().ok()))
        .collect();

    versions.sort();

    // 保留最新两个版本
    if versions.len() > 2 {
        for version in versions.iter().take(versions.len() - 2) {
            let version_dir = package_dir.join(version);
            if version_dir.is_dir() {
                fsxg::remove_directory(&version_dir)?;
            }
        }
    }

    Ok(())
}

/// 更新版本历史
fn update_version_history(package_id: &str, version: &str, repo_path: &Path) -> Result<()> {
    let history_path = repo_path
        .join("packages")
        .join(package_id)
        .join("versions.txt");

    let mut versions = if history_path.exists() {
        fs::read_to_string(&history_path)?
            .lines()
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    };

    // 添加新版本（如果不存在）
    if !versions.contains(&version.to_string()) {
        versions.push(version.to_string());
        fs::write(&history_path, versions.join("\n"))?;
    }

    Ok(())
}

/// 更新包索引
fn update_package_index(
    metadata: &PackageMetadata,
    package_dir: &Path,
    index_path: &Path,
) -> Result<()> {
    let mut index: RepositoryIndex = if index_path.exists() {
        load_json(index_path)?
    } else {
        RepositoryIndex {
            packages: Vec::new(),
            source: Vec::new(),
        }
    };

    // 创建包信息
    let package_info = PackageInfo {
        id: metadata.id.clone(),
        name: metadata.name.clone(),
        icon: metadata.icon.clone(),
        author: metadata.author.clone(),
        latest_version: metadata.version.clone(),
        description: metadata.description.clone(),
        location: format!("./packages/{}/{}", metadata.id, metadata.version),
    };

    // 更新或添加包信息
    if let Some(pos) = index.packages.iter().position(|p| p.id == metadata.id) {
        index.packages[pos] = package_info;
    } else {
        index.packages.push(package_info);
    }

    save_json(&index, index_path)?;
    Ok(())
}

/// 从索引中移除包
fn remove_package_from_index(
    package_id: &str,
    version: Option<&str>,
    index_path: &Path,
) -> Result<()> {
    let mut index: RepositoryIndex = load_json(index_path)?;

    if let Some(_version) = version {
        // 移除特定版本（从版本历史中移除，但保留包记录）
        if let Some(package) = index.packages.iter_mut().find(|p| p.id == package_id) {
            // 更新最新版本为剩余版本中的最新版
            let history_path = Path::new(&package.location)
                .parent()
                .unwrap()
                .join("versions.txt");

            if let Ok(versions) = read_version_history(&history_path) {
                if let Some(latest) = versions.last() {
                    package.latest_version = latest.clone();
                }
            }
        }
    } else {
        // 移除整个包
        index.packages.retain(|p| p.id != package_id);
    }

    save_json(&index, index_path)?;
    Ok(())
}

/// 读取版本历史
fn read_version_history(path: &Path) -> Result<Vec<String>> {
    if path.exists() {
        Ok(fs::read_to_string(path)?
            .lines()
            .map(|s| s.to_string())
            .collect())
    } else {
        Ok(Vec::new())
    }
}

/// 从版本历史中移除特定版本
fn remove_version_from_history(package_id: &str, version: &str, repo_path: &Path) -> Result<()> {
    let history_path = repo_path
        .join("packages")
        .join(package_id)
        .join("versions.txt");

    if history_path.exists() {
        let mut versions: Vec<String> = fs::read_to_string(&history_path)?
            .lines()
            .map(|s| s.to_string())
            .collect();

        versions.retain(|v| v != version);

        if versions.is_empty() {
            fs::remove_file(&history_path)?;
        } else {
            fs::write(&history_path, versions.join("\n"))?;
        }
    }

    Ok(())
}

/// 移除整个包的历史记录
fn remove_package_history(package_id: &str, repo_path: &Path) -> Result<()> {
    let history_path = repo_path
        .join("packages")
        .join(package_id)
        .join("versions.txt");

    if history_path.exists() {
        fs::remove_file(history_path)?;
    }

    Ok(())
}
