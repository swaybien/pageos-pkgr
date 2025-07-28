// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::crypto;
use crate::fsxg;
use crate::metadata::PackageMetadata;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// 初始化应用包
///
/// 在指定目录初始化新的应用包，创建必要的文件结构和 metadata.json 文件
///
/// # 参数
/// - `package_path`: 应用包的根目录路径
///
/// # 返回值
/// 返回 Result<(), anyhow::Error>，成功时返回 Ok(())，失败时返回错误
///
/// # 流程
/// 1. 创建包目录（如果不存在）
/// 2. 创建 metadata.json 文件，包含默认的包配置
/// 3. 创建 .gitignore 文件，忽略 target 目录
pub fn init<P: AsRef<Path>>(package_path: P) -> Result<()> {
    let package_path = package_path.as_ref();

    // 创建包目录
    fsxg::create_directory(package_path)
        .with_context(|| format!("无法创建包目录: {}", package_path.display()))?;

    // 创建 metadata.json 文件
    let metadata_path = package_path.join("metadata.json");
    if !metadata_path.exists() {
        let mut metadata = PackageMetadata::new();
        metadata.id = package_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        metadata.name = metadata.id.clone();
        metadata.version = "0.0.0".to_string();
        metadata.description = "A PageOS web application".to_string();
        metadata.author = "Unknown".to_string();
        metadata.r#type = "webapp".to_string();
        metadata.category = "utility".to_string();
        metadata.entry = "index.html".to_string();

        let metadata_json =
            serde_json::to_string_pretty(&metadata).with_context(|| "无法序列化元数据")?;
        fs::write(&metadata_path, metadata_json)
            .with_context(|| format!("无法写入元数据文件: {}", metadata_path.display()))?;
    }

    // 创建 .gitignore 文件
    let gitignore_path = package_path.join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(&gitignore_path, "/target/")
            .with_context(|| format!("无法创建 .gitignore 文件: {}", gitignore_path.display()))?;
    }

    Ok(())
}

/// 创建新应用包
///
/// 创建以 package-id 命名的目录，并在目录内初始化新的应用包
///
/// # 参数
/// - `package_id`: 应用包的唯一标识符
/// - `base_dir`: 基础目录路径，新包将创建在此目录下
///
/// # 返回值
/// 返回 Result<PathBuf, anyhow::Error>，成功时返回新创建的包路径，失败时返回错误
///
/// # 流程
/// 1. 构建新包的完整路径
/// 2. 调用 init() 在新目录中初始化应用包
pub fn new<S: AsRef<str>, P: AsRef<Path>>(package_id: S, base_dir: P) -> Result<PathBuf> {
    let package_id = package_id.as_ref();
    let base_dir = base_dir.as_ref();
    let package_path = base_dir.join(package_id);

    init(&package_path).with_context(|| format!("无法初始化新应用包: {}", package_id))?;

    Ok(package_path)
}

/// 添加文件到包清单
///
/// 将指定文件或目录（递归）添加到包的 metadata.json 文件的 all_files 字段中
///
/// # 参数
/// - `path`: 要添加的文件或目录路径
/// - `package_path`: 应用包的根目录路径
///
/// # 返回值
/// 返回 Result<(), anyhow::Error>，成功时返回 Ok(())，失败时返回错误
///
/// # 流程
/// 1. 读取现有的 metadata.json 文件
/// 2. 对于文件：计算 SHA256 哈希值，添加到 all_files 映射中
/// 3. 对于目录：递归遍历所有文件，计算每个文件的哈希值并添加
/// 4. 保存更新后的 metadata.json 文件
pub fn add_file<P: AsRef<Path>>(path: P, package_path: P) -> Result<()> {
    let path = path.as_ref();
    let package_path = package_path.as_ref();
    let abs_path =
        fs::canonicalize(path).with_context(|| format!("无法解析路径: {}", path.display()))?;

    // 读取现有的元数据
    let metadata_path = package_path.join("metadata.json");
    let metadata_content = fs::read_to_string(&metadata_path)
        .with_context(|| format!("无法读取元数据文件: {}", metadata_path.display()))?;
    let mut metadata: PackageMetadata =
        serde_json::from_str(&metadata_content).with_context(|| "无法解析元数据 JSON")?;

    // 获取包的根目录的绝对路径
    let package_abs_path = fs::canonicalize(package_path)
        .with_context(|| format!("无法解析包路径: {}", package_path.display()))?;

    // 确保路径在包目录内
    if !abs_path.starts_with(&package_abs_path) {
        return Err(anyhow::anyhow!(
            "文件路径 {} 不在包目录 {} 内",
            abs_path.display(),
            package_abs_path.display()
        ));
    }

    // 计算相对于包目录的路径
    let relative_path = abs_path
        .strip_prefix(&package_abs_path)
        .with_context(|| "无法计算相对于包目录的路径")?
        .to_path_buf();

    if path.is_file() {
        // 处理单个文件
        let hash = crypto::file_hash(path.to_str().unwrap())
            .with_context(|| format!("无法计算文件哈希: {}", path.display()))?;
        let relative_path_str = relative_path.to_string_lossy().replace("\\", "/");
        metadata.add_file(relative_path_str.to_string(), hash);
    } else if path.is_dir() {
        // 处理目录，递归添加所有文件
        let files = fsxg::get_directory_files(path, true)
            .with_context(|| format!("无法获取目录文件: {}", path.display()))?;

        for file_path in files {
            let hash = crypto::file_hash(file_path.to_str().unwrap())
                .with_context(|| format!("无法计算文件哈希: {}", file_path.display()))?;
            let file_relative_path = file_path
                .strip_prefix(&package_abs_path)
                .with_context(|| "无法计算相对于包目录的路径")?;
            let relative_path_str = file_relative_path.to_string_lossy().replace("\\", "/");
            metadata.add_file(relative_path_str.to_string(), hash);
        }
    } else {
        return Err(anyhow::anyhow!(
            "路径既不是文件也不是目录: {}",
            path.display()
        ));
    }

    // 保存更新后的元数据
    let metadata_json =
        serde_json::to_string_pretty(&metadata).with_context(|| "无法序列化元数据")?;
    fs::write(&metadata_path, metadata_json)
        .with_context(|| format!("无法写入元数据文件: {}", metadata_path.display()))?;

    Ok(())
}

/// 从包清单移除文件
///
/// 从 metadata.json 的 all_files 字段中移除指定文件或目录（内所有文件）的条目
///
/// # 参数
/// - `path`: 要移除的文件或目录路径
/// - `package_path`: 应用包的根目录路径
///
/// # 返回值
/// 返回 Result<(), anyhow::Error>，成功时返回 Ok(())，失败时返回错误
///
/// # 流程
/// 1. 读取现有的 metadata.json 文件
/// 2. 对于文件：从 all_files 映射中移除对应的条目
/// 3. 对于目录：递归移除目录内所有文件的条目
/// 4. 保存更新后的 metadata.json 文件
pub fn remove_file<P: AsRef<Path>>(path: P, package_path: P) -> Result<()> {
    let path = path.as_ref();
    let package_path = package_path.as_ref();
    let abs_path =
        fs::canonicalize(path).with_context(|| format!("无法解析路径: {}", path.display()))?;

    // 读取现有的元数据
    let metadata_path = package_path.join("metadata.json");
    let metadata_content = fs::read_to_string(&metadata_path)
        .with_context(|| format!("无法读取元数据文件: {}", metadata_path.display()))?;
    let mut metadata: PackageMetadata =
        serde_json::from_str(&metadata_content).with_context(|| "无法解析元数据 JSON")?;

    // 获取包的根目录的绝对路径
    let package_abs_path = fs::canonicalize(package_path)
        .with_context(|| format!("无法解析包路径: {}", package_path.display()))?;

    // 确保路径在包目录内
    if !abs_path.starts_with(&package_abs_path) {
        return Err(anyhow::anyhow!(
            "文件路径 {} 不在包目录 {} 内",
            abs_path.display(),
            package_abs_path.display()
        ));
    }

    // 计算相对于包目录的路径
    let relative_path = abs_path
        .strip_prefix(&package_abs_path)
        .with_context(|| "无法计算相对于包目录的路径")?
        .to_path_buf();

    if path.is_file() {
        // 处理单个文件
        let relative_path_str = relative_path.to_string_lossy().replace("\\", "/");
        metadata.remove_file(&relative_path_str);
    } else if path.is_dir() {
        // 处理目录，递归移除所有文件的条目
        let files = fsxg::get_directory_files(path, true)
            .with_context(|| format!("无法获取目录文件: {}", path.display()))?;

        for file_path in files {
            let file_relative_path = file_path
                .strip_prefix(&package_abs_path)
                .with_context(|| "无法计算相对于包目录的路径")?;
            let relative_path_str = file_relative_path.to_string_lossy().replace("\\", "/");
            metadata.remove_file(&relative_path_str);
        }
    } else {
        return Err(anyhow::anyhow!(
            "路径既不是文件也不是目录: {}",
            path.display()
        ));
    }

    // 保存更新后的元数据
    let metadata_json =
        serde_json::to_string_pretty(&metadata).with_context(|| "无法序列化元数据")?;
    fs::write(&metadata_path, metadata_json)
        .with_context(|| format!("无法写入元数据文件: {}", metadata_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_file<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
        let mut file = File::create(path).with_context(|| "无法创建测试文件")?;
        file.write_all(content.as_bytes())
            .with_context(|| "无法写入测试文件")?;
        Ok(())
    }

    #[test]
    fn test_init_and_new() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let package_path = temp_dir.path().join("test-app");

        // 测试 init
        init(&package_path)?;
        assert!(package_path.exists());
        assert!(package_path.join("metadata.json").exists());
        assert!(package_path.join(".gitignore").exists());

        // 验证 metadata.json 内容
        let metadata_content = std::fs::read_to_string(package_path.join("metadata.json"))?;
        let metadata: PackageMetadata = serde_json::from_str(&metadata_content)?;
        assert_eq!(metadata.id, "test-app");
        assert_eq!(metadata.name, "test-app");
        assert_eq!(metadata.version, "0.0.0");

        Ok(())
    }

    #[test]
    fn test_add_and_remove_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let package_path = temp_dir.path().join("test-app");

        // 初始化包
        init(&package_path)?;

        // 创建测试文件
        let test_file = package_path.join("test.txt");
        create_test_file(&test_file, "Hello, world!")?;

        // 添加文件到清单
        add_file(&test_file, &package_path)?;

        // 验证文件已添加
        let metadata_content = std::fs::read_to_string(package_path.join("metadata.json"))?;
        let metadata: PackageMetadata = serde_json::from_str(&metadata_content)?;
        assert!(metadata.has_file("test.txt"));

        // 移除文件
        remove_file(&test_file, &package_path)?;

        // 验证文件已移除
        let metadata_content = std::fs::read_to_string(package_path.join("metadata.json"))?;
        let metadata: PackageMetadata = serde_json::from_str(&metadata_content)?;
        assert!(!metadata.has_file("test.txt"));

        Ok(())
    }

    #[test]
    fn test_add_and_remove_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let package_path = temp_dir.path().join("test-app");

        // 初始化包
        init(&package_path)?;

        // 创建测试目录和文件
        let test_dir = package_path.join("test-dir");
        fsxg::create_directory(&test_dir)?;
        create_test_file(test_dir.join("file1.txt"), "content1")?;
        create_test_file(test_dir.join("file2.txt"), "content2")?;

        // 添加目录到清单
        add_file(&test_dir, &package_path)?;

        // 验证文件已添加
        let metadata_content = std::fs::read_to_string(package_path.join("metadata.json"))?;
        let metadata: PackageMetadata = serde_json::from_str(&metadata_content)?;
        assert!(metadata.has_file("test-dir/file1.txt"));
        assert!(metadata.has_file("test-dir/file2.txt"));

        // 移除目录
        remove_file(&test_dir, &package_path)?;

        // 验证文件已移除
        let metadata_content = std::fs::read_to_string(package_path.join("metadata.json"))?;
        let metadata: PackageMetadata = serde_json::from_str(&metadata_content)?;
        assert!(!metadata.has_file("test-dir/file1.txt"));
        assert!(!metadata.has_file("test-dir/file2.txt"));

        Ok(())
    }
}
