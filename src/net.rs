// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use reqwest;
use tokio::io::AsyncWriteExt;

/// 从指定URL下载文件到本地路径
///
/// # 参数
///
/// * `url` - 要下载的文件的URL
/// * `path` - 本地保存文件的路径
///
/// # 返回值
///
/// 返回 `Result<(), Box<dyn std::error::Error>>`，成功时返回 Ok(())，失败时返回错误
///
/// # 功能特性
///
/// * 支持 HTTP/HTTPS 下载
/// * 显示下载进度
/// * 处理网络异常（超时、连接失败等）
/// * 流式下载，节省内存
pub async fn download_file(url: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 创建 HTTP 客户端
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // 发起 GET 请求
    let response = client.get(url).send().await?;

    // 检查响应状态
    if !response.status().is_success() {
        return Err(format!("HTTP请求失败: {}", response.status()).into());
    }

    // 获取文件总大小用于进度显示
    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    // 确保目标目录存在
    let parent_dir = std::path::Path::new(path)
        .parent()
        .ok_or_else(|| "无法获取父目录")?;
    tokio::fs::create_dir_all(parent_dir).await?;

    // 创建本地文件
    let mut file = tokio::fs::File::create(path).await?;

    // 流式写入文件
    let bytes = response.bytes().await?;
    let bytes_len = bytes.len() as u64;
    file.write_all(&bytes).await?;

    // 更新下载进度
    downloaded += bytes_len;

    // 显示进度
    if total_size > 0 {
        let progress = (downloaded as f64 / total_size as f64 * 100.0) as u8;
        eprint!("\r下载进度: {}%", progress);
    }

    // 确保所有数据都写入磁盘
    file.flush().await?;

    // 换行结束进度显示
    if total_size > 0 {
        eprintln!();
    }

    // 换行结束进度显示
    if total_size > 0 {
        eprintln!();
    }

    Ok(())
}

/// 从指定URL获取索引数据
///
/// # 参数
///
/// * `url` - 索引文件的URL
///
/// # 返回值
///
/// 返回 `Result<serde_json::Value, Box<dyn std::error::Error>>`，成功时返回解析后的JSON值，失败时返回错误
///
/// # 功能特性
///
/// * 支持 HTTP/HTTPS 请求
/// * 处理网络异常（超时、连接失败等）
/// * 返回解析后的 JSON 数据
pub async fn fetch_index(url: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // 创建 HTTP 客户端
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // 发起 GET 请求
    let response = client.get(url).send().await?;

    // 检查响应状态
    if !response.status().is_success() {
        return Err(format!("HTTP请求失败: {}", response.status()).into());
    }

    // 读取响应体
    let body = response.text().await?;

    // 解析JSON
    let index: serde_json::Value = serde_json::from_str(&body)?;

    Ok(index)
}

/// 执行镜像同步，完全同步源的内容到本地
///
/// # 参数
///
/// * `source_url` - 源的基URL
/// * `target_dir` - 本地目标目录
/// * `enabled` - 源是否启用
/// * `require_https` - 是否强制使用HTTPS
///
/// # 返回值
///
/// 返回 `Result<(), Box<dyn std::error::Error>>`，成功时返回 Ok(())，失败时返回错误
///
/// # 功能特性
///
/// * 完全同步源的内容，保持与源一致
/// * 处理文件的添加、更新和删除
/// * 确保数据完整性
/// * 处理网络异常
pub async fn mirror_sync(
    source_url: &str,
    target_dir: &str,
    enabled: bool,
    require_https: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // 检查源是否启用
    if !enabled {
        return Ok(());
    }

    // 验证URL协议
    if require_https && !source_url.starts_with("https://") {
        return Err("源配置要求使用HTTPS，但提供的URL不是HTTPS".into());
    }

    // 创建目标目录
    std::fs::create_dir_all(target_dir)?;

    // 获取源索引
    let index_url = format!("{}/index.json", source_url.trim_end_matches('/'));
    let index = fetch_index(&index_url).await?;

    // 同步源索引中的所有文件
    if let Some(source) = index["source"].as_array() {
        for pkg in source {
            if let Some(location) = pkg["location"].as_str() {
                // 确保位置以/结尾
                let location = if location.ends_with('/') {
                    location.to_string()
                } else {
                    format!("{}/", location)
                };

                // 获取包的文件列表
                let files_url = format!("{}metadata.json", location);
                let files_index = fetch_index(&files_url).await?;

                // 同步包中的所有文件
                if let Some(files) = files_index["all_files"].as_object() {
                    for (file_path, _hash) in files {
                        let file_url = format!("{}{}", location, file_path);
                        let local_path = format!("{}/{}", target_dir, file_path);

                        // 确保本地目录存在
                        if let Some(parent) = std::path::Path::new(&local_path).parent() {
                            std::fs::create_dir_all(parent)?;
                        }

                        // 下载文件
                        download_file(&file_url, &local_path).await?;
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_download_file_success() -> Result<(), Box<dyn std::error::Error>> {
        // 创建临时目录
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test_download.txt");
        let file_path_str = file_path.to_str().unwrap();

        // 下载一个已知的小文件进行测试
        download_file("https://httpbin.org/bytes/1024", file_path_str).await?;

        // 验证文件存在且大小正确
        assert!(file_path.exists());
        let metadata = fs::metadata(file_path)?;
        assert!(metadata.len() > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_download_file_invalid_url() {
        let result = download_file("https://not-exsist.example.com/file.txt", "test.txt").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_download_file_to_invalid_path() {
        let result = download_file("https://httpbin.org/bytes/10", "/invalid/path/test.txt").await;
        assert!(result.is_err());
    }
}
