// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::{Context, Result};
use std::path::PathBuf;

/// 表示文件系统操作的类型
#[derive(Debug, Clone)]
pub enum Operation {
    /// 创建文件操作
    Create { path: PathBuf },
    /// 删除文件操作
    Remove {
        path: PathBuf,
        /// 被删除文件的原始内容（用于回滚）
        content: Vec<u8>,
    },
    /// 移动/重命名文件操作
    Move {
        from: PathBuf,
        to: PathBuf,
        /// 目标路径的原始内容（如果存在，用于回滚）
        original_dest_content: Option<Vec<u8>>,
    },
}

/// 事务管理器
pub struct Transaction {
    /// 操作日志，记录事务中执行的所有操作
    log: Vec<Operation>,
    /// 事务是否已提交
    committed: bool,
}

impl Transaction {
    /// 创建一个新的事务
    pub fn new() -> Self {
        Self {
            log: Vec::new(),
            committed: false,
        }
    }

    /// 开始新的事务
    pub fn begin() -> Self {
        Self::new()
    }

    /// 提交事务，清空操作日志
    pub fn commit(mut self) -> Result<()> {
        self.committed = true;
        // 清空日志
        self.log.clear();
        Ok(())
    }

    /// 回滚事务，撤销所有已执行的操作
    pub fn rollback(mut self) -> Result<()> {
        // 从后往前执行回滚操作
        while let Some(op) = self.log.pop() {
            match op {
                Operation::Create { path } => {
                    // 回滚创建：删除已创建的文件
                    if path.exists() {
                        std::fs::remove_file(&path).with_context(|| {
                            format!("无法回滚创建操作: 删除文件失败 {}", path.display())
                        })?;
                    }
                }
                Operation::Remove { path, content } => {
                    // 回滚删除：重新创建文件并写入原始内容
                    if let Some(parent) = path.parent() {
                        crate::fsxg::create_directory(parent)
                            .with_context(|| format!("无法创建父目录: {}", parent.display()))?;
                    }
                    std::fs::write(&path, content).with_context(|| {
                        format!("无法回滚删除操作: 写入文件失败 {}", path.display())
                    })?;
                }
                Operation::Move {
                    from,
                    to,
                    original_dest_content,
                } => {
                    // 回滚移动：将文件移回原位置
                    if to.exists() {
                        std::fs::rename(&to, &from).with_context(|| {
                            format!(
                                "无法回滚移动操作: 重命名失败 {} -> {}",
                                to.display(),
                                from.display()
                            )
                        })?;

                        // 如果目标位置原来有文件，需要恢复
                        if let Some(content) = original_dest_content {
                            std::fs::write(&to, content).with_context(|| {
                                format!("无法恢复目标位置的原始文件: {}", to.display())
                            })?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 在事务中安全地创建文件
    pub fn safe_create(&mut self, path: &std::path::Path, content: &[u8]) -> Result<()> {
        if path.exists() {
            return Err(anyhow::anyhow!("文件已存在: {}", path.display()));
        }

        // 确保父目录存在
        if let Some(parent) = path.parent() {
            crate::fsxg::create_directory(parent)
                .with_context(|| format!("无法创建父目录: {}", parent.display()))?;
        }

        // 执行创建操作
        std::fs::write(path, content)
            .with_context(|| format!("创建文件失败: {}", path.display()))?;

        // 记录操作到日志
        self.log.push(Operation::Create {
            path: path.to_path_buf(),
        });

        Ok(())
    }

    /// 在事务中安全地删除文件
    pub fn safe_remove(&mut self, path: &std::path::Path) -> Result<()> {
        if !path.exists() {
            return Err(anyhow::anyhow!("文件不存在: {}", path.display()));
        }

        if path.is_dir() {
            return Err(anyhow::anyhow!("路径是目录，不能删除: {}", path.display()));
        }

        // 读取文件内容用于回滚
        let content =
            std::fs::read(path).with_context(|| format!("无法读取文件内容: {}", path.display()))?;

        // 执行删除操作
        std::fs::remove_file(path).with_context(|| format!("删除文件失败: {}", path.display()))?;

        // 记录操作到日志
        self.log.push(Operation::Remove {
            path: path.to_path_buf(),
            content,
        });

        Ok(())
    }

    /// 在事务中安全地移动文件
    pub fn safe_move(&mut self, from: &std::path::Path, to: &std::path::Path) -> Result<()> {
        if !from.exists() {
            return Err(anyhow::anyhow!("源文件不存在: {}", from.display()));
        }

        if from.is_dir() {
            return Err(anyhow::anyhow!(
                "源路径是目录，不能移动: {}",
                from.display()
            ));
        }

        // 读取目标位置的原始内容（如果存在）
        let original_dest_content = if to.exists() {
            if to.is_dir() {
                return Err(anyhow::anyhow!("目标路径是目录: {}", to.display()));
            }
            Some(
                std::fs::read(to)
                    .with_context(|| format!("无法读取目标文件内容: {}", to.display()))?,
            )
        } else {
            None
        };

        // 确保目标父目录存在
        if let Some(parent) = to.parent() {
            crate::fsxg::create_directory(parent)
                .with_context(|| format!("无法创建目标父目录: {}", parent.display()))?;
        }

        // 执行移动操作
        std::fs::rename(from, to)
            .with_context(|| format!("移动文件失败: {} -> {}", from.display(), to.display()))?;

        // 记录操作到日志
        self.log.push(Operation::Move {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            original_dest_content,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_transaction_commit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        let content = b"Hello, world!";

        // 开始事务
        let mut tx = Transaction::begin();

        // 在事务中创建文件
        tx.safe_create(&file_path, content)?;

        // 提交事务
        tx.commit()?;

        // 验证文件存在且内容正确
        assert!(file_path.exists());
        assert_eq!(fs::read(&file_path)?, content);

        Ok(())
    }

    #[test]
    fn test_transaction_rollback_create() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        let content = b"Hello, world!";

        // 开始事务
        let mut tx = Transaction::begin();

        // 在事务中创建文件
        tx.safe_create(&file_path, content)?;

        // 验证文件已创建
        assert!(file_path.exists());

        // 回滚事务
        tx.rollback()?;

        // 验证文件已被删除
        assert!(!file_path.exists());

        Ok(())
    }

    #[test]
    fn test_transaction_rollback_remove() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.txt");
        let content = b"Hello, world!";

        // 创建文件
        fs::write(&file_path, content)?;

        // 开始事务
        let mut tx = Transaction::begin();

        // 在事务中删除文件
        tx.safe_remove(&file_path)?;

        // 验证文件已被删除
        assert!(!file_path.exists());

        // 回滚事务
        tx.rollback()?;

        // 验证文件已恢复
        assert!(file_path.exists());
        assert_eq!(fs::read(&file_path)?, content);

        Ok(())
    }

    #[test]
    fn test_transaction_rollback_move() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let from_path = temp_dir.path().join("from.txt");
        let to_path = temp_dir.path().join("to.txt");
        let content = b"Hello, world!";

        // 创建源文件
        fs::write(&from_path, content)?;

        // 开始事务
        let mut tx = Transaction::begin();

        // 在事务中移动文件
        tx.safe_move(&from_path, &to_path)?;

        // 验证移动已发生
        assert!(!from_path.exists());
        assert!(to_path.exists());

        // 回滚事务
        tx.rollback()?;

        // 验证文件已移回原位置
        assert!(from_path.exists());
        assert!(!to_path.exists());
        assert_eq!(fs::read(&from_path)?, content);

        Ok(())
    }

    #[test]
    fn test_transaction_rollback_move_overwrite() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let from_path = temp_dir.path().join("from.txt");
        let to_path = temp_dir.path().join("to.txt");
        let from_content = b"From content";
        let to_content = b"To content";

        // 创建源文件和目标文件
        fs::write(&from_path, from_content)?;
        fs::write(&to_path, to_content)?;

        // 开始事务
        let mut tx = Transaction::begin();

        // 在事务中移动文件（会覆盖目标文件）
        tx.safe_move(&from_path, &to_path)?;

        // 验证移动已发生
        assert!(!from_path.exists());
        assert!(to_path.exists());
        assert_eq!(fs::read(&to_path)?, from_content); // 内容被覆盖

        // 回滚事务
        tx.rollback()?;

        // 验证文件已移回原位置且目标文件内容被恢复
        assert!(from_path.exists());
        assert!(to_path.exists());
        assert_eq!(fs::read(&from_path)?, from_content);
        assert_eq!(fs::read(&to_path)?, to_content); // 原始内容被恢复

        Ok(())
    }
}
