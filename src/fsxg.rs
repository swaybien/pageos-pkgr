// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// 创建目录
///
/// # 参数
/// - `path`: 要创建的目录路径
///
/// # 返回值
/// 返回 Result<(), anyhow::Error>，成功时返回 Ok(())，失败时返回错误
///
/// # 示例
/// ```
/// create_directory("/tmp/test")?;
/// ```
pub fn create_directory<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    fs::create_dir_all(path).with_context(|| format!("无法创建目录: {}", path.display()))
}

/// 复制目录
///
/// # 参数
/// - `from`: 源目录路径
/// - `to`: 目标目录路径
///
/// # 返回值
/// 返回 Result<(), anyhow::Error>，成功时返回 Ok(())，失败时返回错误
///
/// # 示例
/// ```
/// copy_directory("/tmp/source", "/tmp/destination")?;
/// ```
// pub fn copy_directory<P: AsRef<Path>>(from: P, to: P) -> Result<()> {
//     let from = from.as_ref();
//     let to = to.as_ref();

//     // 确保源目录存在且为目录
//     if !from.exists() {
//         return Err(anyhow::anyhow!("源目录不存在: {}", from.display()));
//     }
//     if !from.is_dir() {
//         return Err(anyhow::anyhow!("源路径不是目录: {}", from.display()));
//     }

//     // 创建目标目录
//     create_directory(to)?;

//     // 遍历源目录中的所有条目
//     for entry in
//         fs::read_dir(from).with_context(|| format!("无法读取源目录: {}", from.display()))?
//     {
//         let entry = entry.with_context(|| format!("无法读取目录条目: {}", from.display()))?;
//         let path = entry.path();
//         let file_name = entry.file_name();
//         let dest_path = to.join(&file_name);

//         if path.is_dir() {
//             // 递归复制子目录
//             copy_directory(&path, &dest_path)?;
//         } else {
//             // 复制文件
//             fs::copy(&path, &dest_path).with_context(|| {
//                 format!(
//                     "无法复制文件: {} -> {}",
//                     path.display(),
//                     dest_path.display()
//                 )
//             })?;
//         }
//     }

//     Ok(())
// }

/// 移除目录
///
/// # 参数
/// - `path`: 要移除的目录路径
///
/// # 返回值
/// 返回 Result<(), anyhow::Error>，成功时返回 Ok(())，失败时返回错误
///
/// # 示例
/// ```
/// remove_directory("/tmp/test")?;
/// ```
pub fn remove_directory<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();

    if !path.exists() {
        return Ok(());
    }

    if !path.is_dir() {
        return Err(anyhow::anyhow!("路径不是目录: {}", path.display()));
    }

    fs::remove_dir_all(path).with_context(|| format!("无法移除目录: {}", path.display()))
}

/// 获取目录文件列表
///
/// # 参数
/// - `path`: 目录路径
/// - `recursive`: 是否递归遍历子目录
///
/// # 返回值
/// 返回 Result<Vec<PathBuf>, anyhow::Error>，成功时返回文件路径列表，失败时返回错误
///
/// # 示例
/// ```
/// let files = get_directory_files("/tmp", true)?;
/// ```
pub fn get_directory_files<P: AsRef<Path>>(path: P, recursive: bool) -> Result<Vec<PathBuf>> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(anyhow::anyhow!("目录不存在: {}", path.display()));
    }

    if !path.is_dir() {
        return Err(anyhow::anyhow!("路径不是目录: {}", path.display()));
    }

    let mut files = Vec::new();

    if recursive {
        // 递归遍历目录
        for entry in walkdir::WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let file_path = entry.into_path();
                // 确保返回绝对路径
                let abs_path = if file_path.is_absolute() {
                    file_path
                } else {
                    // 将相对路径转换为绝对路径
                    std::fs::canonicalize(&file_path)
                        .with_context(|| format!("无法解析路径: {:?}", file_path))?
                };
                files.push(abs_path);
            }
        }
    } else {
        // 只遍历当前目录
        for entry in
            fs::read_dir(path).with_context(|| format!("无法读取目录: {}", path.display()))?
        {
            let entry = entry.with_context(|| format!("无法读取目录条目: {}", path.display()))?;
            let entry_path = entry.path();
            // 确保返回绝对路径
            let abs_path = if entry_path.is_absolute() {
                entry_path
            } else {
                let full_path = path.join(&entry_path);
                // 将相对路径转换为绝对路径
                std::fs::canonicalize(&full_path)
                    .with_context(|| format!("无法解析路径: {:?}", full_path))?
            };

            if abs_path.is_file() {
                files.push(abs_path);
            }
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    fn create_test_file<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
        let mut file = File::create(path).with_context(|| "无法创建测试文件")?;
        use std::io::Write;
        file.write_all(content.as_bytes())
            .with_context(|| "无法写入测试文件")?;
        Ok(())
    }

    #[test]
    fn test_create_and_remove_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_path = temp_dir.path().join("test_dir");

        // 测试创建目录
        create_directory(&test_path)?;
        assert!(test_path.exists());
        assert!(test_path.is_dir());

        // 测试移除目录
        remove_directory(&test_path)?;
        assert!(!test_path.exists());

        // 测试移除不存在的目录（应该成功）
        remove_directory(&test_path)?;

        Ok(())
    }

    // #[test]
    // fn test_copy_directory() -> Result<()> {
    //     let temp_dir = TempDir::new()?;
    //     let src_dir = temp_dir.path().join("src");
    //     let dst_dir = temp_dir.path().join("dst");

    //     // 创建源目录结构
    //     create_directory(&src_dir)?;
    //     create_test_file(src_dir.join("file1.txt"), "content1")?;
    //     create_test_file(src_dir.join("file2.txt"), "content2")?;

    //     let sub_dir = src_dir.join("subdir");
    //     create_directory(&sub_dir)?;
    //     create_test_file(sub_dir.join("file3.txt"), "content3")?;

    //     // 复制目录
    //     copy_directory(&src_dir, &dst_dir)?;

    //     // 验证复制结果
    //     assert!(dst_dir.exists());
    //     assert!(dst_dir.join("file1.txt").exists());
    //     assert!(dst_dir.join("file2.txt").exists());
    //     assert!(dst_dir.join("subdir").exists());
    //     assert!(dst_dir.join("subdir").join("file3.txt").exists());

    //     Ok(())
    // }

    #[test]
    fn test_get_directory_files() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_dir = temp_dir.path().join("test");

        // 创建测试目录结构
        create_directory(&test_dir)?;
        create_test_file(test_dir.join("file1.txt"), "content1")?;
        create_test_file(test_dir.join("file2.txt"), "content2")?;

        let sub_dir = test_dir.join("subdir");
        create_directory(&sub_dir)?;
        create_test_file(sub_dir.join("file3.txt"), "content3")?;

        // 测试非递归获取文件
        let files = get_directory_files(&test_dir, false)?;
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.file_name().unwrap() == "file1.txt"));
        assert!(files.iter().any(|f| f.file_name().unwrap() == "file2.txt"));
        assert!(!files.iter().any(|f| f.file_name().unwrap() == "file3.txt"));

        // 测试递归获取文件
        let files = get_directory_files(&test_dir, true)?;
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.file_name().unwrap() == "file1.txt"));
        assert!(files.iter().any(|f| f.file_name().unwrap() == "file2.txt"));
        assert!(files.iter().any(|f| f.file_name().unwrap() == "file3.txt"));

        Ok(())
    }
}
