// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/// 版本比较
/// 
/// 比较两个版本在版本清单中的行号（越高越新）
/// 
/// # Arguments
/// 
/// * `version1` - 第一个版本号
/// * `version2` - 第二个版本号
/// * `versions` - 版本清单，按从旧到新顺序排列
/// 
/// # Returns
/// 
/// * `1` 如果 version1 更新
/// * `-1` 如果 version2 更新
/// * `0` 如果版本相同
/// 
/// # Examples
/// 
/// ```
/// let versions = vec!["1.0.0".to_string(), "1.1.0".to_string(), "2.0.0".to_string()];
/// assert_eq!(compare("2.0.0", "1.1.0", &versions), 1);
/// assert_eq!(compare("1.0.0", "2.0.0", &versions), -1);
/// assert_eq!(compare("1.1.0", "1.1.0", &versions), 0);
/// ```
pub fn compare(version1: &str, version2: &str, versions: &[String]) -> i32 {
    // 如果版本号相同，返回0
    if version1 == version2 {
        return 0;
    }
    
    // 查找版本在清单中的位置（索引）
    let pos1 = versions.iter().position(|v| v == version1);
    let pos2 = versions.iter().position(|v| v == version2);
    
    match (pos1, pos2) {
        // 两个版本都存在，比较位置
        (Some(p1), Some(p2)) => {
            if p1 > p2 {
                1 // version1 更新
            } else {
                -1 // version2 更新
            }
        },
        // 只有一个版本存在
        (Some(_), None) => 1,  // version1 存在，认为它更新
        (None, Some(_)) => -1, // version2 存在，认为它更新
        // 两个版本都不存在
        (None, None) => 0,     // 无法比较，认为相同
    }
}

/// 获取最新版本
/// 
/// 从版本清单中获取最新版本（最后一个）
/// 
/// # Arguments
/// 
/// * `versions` - 版本清单，按从旧到新顺序排列
/// 
/// # Returns
/// 
/// * `Some(&str)` 最新版本号的引用
/// * `None` 如果版本清单为空
/// 
/// # Examples
/// 
/// ```
/// let versions = vec!["1.0.0".to_string(), "1.1.0".to_string(), "2.0.0".to_string()];
/// assert_eq!(get_latest(&versions), Some("2.0.0"));
/// ```
pub fn get_latest(versions: &[String]) -> Option<&str> {
    versions.last().map(|s| s.as_str())
}

/// 版本解析
/// 
/// 解析版本字符串，提取主要版本信息
/// 目前直接返回原版本字符串
/// 
/// # Arguments
/// 
/// * `version` - 版本字符串
/// 
/// # Returns
/// 
/// * 解析后的版本字符串
/// 
/// # Examples
/// 
/// ```
/// assert_eq!(parse("1.2.3"), "1.2.3");
/// assert_eq!(parse("139402853dw3d3"), "139402853dw3d3");
/// ```
pub fn parse(version: &str) -> &str {
    // 目前直接返回原版本字符串
    // 未来可以添加更复杂的解析逻辑
    version
}