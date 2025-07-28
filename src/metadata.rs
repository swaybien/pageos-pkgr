// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 包元数据
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackageMetadata {
    /// 应用名称
    pub name: String,
    /// 应用唯一标识
    pub id: String,
    /// 版本号
    pub version: String,
    /// 详细描述
    pub description: String,
    /// 图标路径（相对于软件包）
    pub icon: String,
    /// 作者
    pub author: String,
    /// 应用类型
    pub r#type: String,
    /// 分类
    pub category: String,
    /// 权限列表
    pub permissions: Vec<String>,
    /// 入口文件
    pub entry: String,
    /// 文件清单
    pub all_files: HashMap<String, String>,
}

/// 文件清单
///
/// 用于表示单个文件的路径和其对应的 SHA256 哈希值。
/// 在 `PackageMetadata` 中，`all_files` 字段使用 `HashMap<String, String>` 来存储多个文件。
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileManifest {
    /// 文件相对路径
    pub path: String,
    /// SHA256 哈希值
    pub hash: String,
}

/// 版本历史
///
/// 用于管理软件包的版本信息。
/// 通常以递增的方式记录版本号，用于版本比较和更新检查。
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionHistory {
    /// 版本号列表，按时间顺序（从旧到新）存储
    pub versions: Vec<String>,
}

impl VersionHistory {
    /// 创建一个新的版本历史记录
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
        }
    }

    /// 添加一个新版本
    pub fn add_version(&mut self, version: String) {
        self.versions.push(version);
    }

    /// 获取最新版本
    pub fn get_latest(&self) -> Option<&String> {
        self.versions.last()
    }

    /// 检查是否存在特定版本
    pub fn has_version(&self, version: &str) -> bool {
        self.versions.contains(&version.to_string())
    }
}

impl Default for PackageMetadata {
    fn default() -> Self {
        Self {
            name: String::new(),
            id: String::new(),
            version: String::new(),
            description: String::new(),
            icon: String::new(),
            author: String::new(),
            r#type: String::new(),
            category: String::new(),
            permissions: Vec::new(),
            entry: String::new(),
            all_files: HashMap::new(),
        }
    }
}

impl PackageMetadata {
    /// 创建一个新的包元数据实例
    pub fn new() -> Self {
        Self::default()
    }

    /// 将文件添加到清单中
    pub fn add_file(&mut self, path: String, hash: String) {
        self.all_files.insert(path, hash);
    }

    /// 从清单中移除文件
    pub fn remove_file(&mut self, path: &str) -> Option<String> {
        self.all_files.remove(path)
    }

    /// 检查文件是否在清单中
    pub fn has_file(&self, path: &str) -> bool {
        self.all_files.contains_key(path)
    }

    /// 获取文件的哈希值
    pub fn get_file_hash(&self, path: &str) -> Option<&String> {
        self.all_files.get(path)
    }
}

impl Default for FileManifest {
    fn default() -> Self {
        Self {
            path: String::new(),
            hash: String::new(),
        }
    }
}

impl FileManifest {
    /// 创建一个新的文件清单项
    pub fn new(path: String, hash: String) -> Self {
        Self { path, hash }
    }
}

impl Default for VersionHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_metadata_creation() {
        let metadata = PackageMetadata::new();
        assert_eq!(metadata.name, "");
        assert_eq!(metadata.id, "");
        assert_eq!(metadata.version, "");
        assert!(metadata.all_files.is_empty());
    }

    #[test]
    fn test_package_metadata_add_and_remove_file() {
        let mut metadata = PackageMetadata::new();
        let path = "test.txt".to_string();
        let hash = "abc123".to_string();

        metadata.add_file(path.clone(), hash.clone());
        assert!(metadata.has_file(&path));
        assert_eq!(metadata.get_file_hash(&path), Some(&hash));

        let removed_hash = metadata.remove_file(&path);
        assert_eq!(removed_hash, Some(hash));
        assert!(!metadata.has_file(&path));
    }

    #[test]
    fn test_version_history_operations() {
        let mut history = VersionHistory::new();
        assert!(history.get_latest().is_none());

        history.add_version("1.0.0".to_string());
        history.add_version("1.1.0".to_string());

        assert_eq!(history.get_latest(), Some(&"1.1.0".to_string()));
        assert!(history.has_version("1.0.0"));
        assert!(history.has_version("1.1.0"));
        assert!(!history.has_version("2.0.0"));
    }

    #[test]
    fn test_file_manifest_creation() {
        let path = "icon.png".to_string();
        let hash = "def456".to_string();
        let manifest = FileManifest::new(path.clone(), hash.clone());

        assert_eq!(manifest.path, path);
        assert_eq!(manifest.hash, hash);
    }
}
