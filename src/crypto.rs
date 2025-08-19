// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;

/// 计算文件的 SHA256 哈希值
///
/// # 参数
/// * `file_path` - 要计算哈希的文件路径
///
/// # 返回
/// 返回包含 64 个字符的十六进制字符串的 Result
///
/// # 示例
/// ```
/// let hash = file_hash("path/to/file.txt")?;
/// println!("文件哈希: {}", hash);
/// ```
pub fn file_hash(file_path: &str) -> Result<String> {
    let mut file = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("{result:x}"))
}

/// 验证文件的完整性
///
/// # 参数
/// * `file_path` - 要验证的文件路径
/// * `expected_hash` - 期望的 SHA256 哈希值（十六进制字符串）
///
/// # 返回
/// 返回布尔值，true 表示验证通过，false 表示验证失败
///
/// # 示例
/// ```
/// let is_valid = verify_file("path/to/file.txt", "expected_hash_value")?;
/// if is_valid {
///     println!("文件验证通过");
/// } else {
///     println!("文件验证失败");
/// }
/// ```
pub fn verify_file(file_path: &str, expected_hash: &str) -> Result<bool> {
    let actual_hash = file_hash(file_path)?;
    Ok(actual_hash.eq_ignore_ascii_case(expected_hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_file_hash() -> Result<()> {
        // 创建临时文件
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(b"Hello, world!")?;
        temp_file.flush()?;

        // 计算哈希
        let hash = file_hash(temp_file.path().to_str().unwrap())?;

        // 验证哈希值（"Hello, world!" 的 SHA256）
        assert_eq!(
            hash,
            "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3"
        );

        Ok(())
    }

    #[test]
    fn test_verify_file() -> Result<()> {
        // 创建临时文件
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(b"Hello, world!")?;
        temp_file.flush()?;

        // 验证正确的哈希
        let is_valid = verify_file(
            temp_file.path().to_str().unwrap(),
            "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3",
        )?;
        assert!(is_valid);

        // 验证错误的哈希
        let is_invalid = verify_file(temp_file.path().to_str().unwrap(), "invalid_hash_value")?;
        assert!(!is_invalid);

        Ok(())
    }

    #[test]
    fn test_verify_file_case_insensitive() -> Result<()> {
        // 创建临时文件
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(b"Hello, world!")?;
        temp_file.flush()?;

        // 测试大小写不敏感的哈希验证
        let is_valid_upper = verify_file(
            temp_file.path().to_str().unwrap(),
            "315F5BDB76D078C43B8AC0064E4A0164612B1FCE77C869345BFC94C75894EDD3",
        )?;
        assert!(is_valid_upper);

        Ok(())
    }
}
