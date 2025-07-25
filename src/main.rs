// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use clap::{Parser, Subcommand};
use sha2::Digest;
use toml;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// 跳过确认提示
    #[arg(short, long)]
    yes: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum SourceCommand {
    /// 创建新的在线软件源仓库
    New {
        /// 软件源仓库路径
        path: String,
    },
    /// 添加新软件（或升级软件版本）到仓库
    Add {
        /// 安装包路径
        package_path: String,
    },
    /// 从仓库删除软件（版本）
    Remove {
        /// 软件包名称和版本，格式为 "包名:版本"
        package_spec: String,
    },
    /// 更新仓库索引
    Update,
    /// 从其它软件源增量同步
    Sync {
        /// 远程软件源 URL
        remote_url: String,
        /// 是否镜像同步（完全覆盖本地）
        #[arg(short, long)]
        mirror: bool,
    },
}

#[derive(Subcommand)]
enum LocalCommand {
    /// 创建新的本地软件仓库
    New {
        /// 本地仓库路径
        path: String,
    },
    /// 更新本地索引
    Update,
    /// 升级已安装软件包
    Upgrade,
    /// 安装软件包
    Install {
        /// 软件包名称
        package_name: String,
    },
    /// 卸载软件包
    Remove {
        /// 软件包名称
        package_name: String,
    },
}

#[derive(Subcommand)]
enum AppCommand {
    /// 创建新的应用包
    New {
        /// 应用包名称 (user/repo)
        name: String,
    },
    /// 在当前目录初始化应用包
    Init,
    /// 添加文件或目录到应用包
    Add {
        /// 要添加的文件或目录路径
        path: String,
    },
}

#[derive(Subcommand)]
enum Commands {
    /// 应用包管理
    App {
        #[command(subcommand)]
        command: AppCommand,
    },
    /// 软件源仓库管理
    Source {
        #[command(subcommand)]
        command: SourceCommand,
    },
    /// 本地仓库管理
    Local {
        #[command(subcommand)]
        command: LocalCommand,
    },
}

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("错误: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    // 在执行命令前进行确认检查
    if !cli.yes && !confirm_action(&cli.command)? {
        println!("操作已取消");
        return Ok(());
    }

    match cli.command {
        Commands::App { command } => match command {
            AppCommand::New { name } => {
                println!("创建新应用包: {}", name);
                // 创建包目录
                let dir_path = Path::new(&name);
                if dir_path.exists() {
                    anyhow::bail!("目录 '{}' 已存在", name);
                }

                fs::create_dir(&dir_path).with_context(|| format!("创建目录 '{}' 失败", name))?;

                // 在新目录中初始化包
                initialize_package(&dir_path).context("初始化软件包失败")?;

                println!("应用包 '{}' 创建成功", name);
                Ok(())
            }
            AppCommand::Init => {
                let current_dir = std::env::current_dir()?;
                initialize_package(&current_dir).context("初始化软件包失败")?;
                println!("软件包初始化完成");
                Ok(())
            }
            AppCommand::Add { path } => {
                handle_app_add(&path)?;
                Ok(())
            }
        },
        Commands::Source { command } => match command {
            SourceCommand::New { path } => {
                create_source_repo(&path)?;
                println!("成功创建软件源仓库于: {}", path);
                Ok(())
            }
            SourceCommand::Add { package_path } => {
                add_package_to_repo(&package_path)?;
                println!("成功添加软件包: {}", package_path);
                Ok(())
            }
            SourceCommand::Remove { package_spec } => {
                remove_package_from_repo(&package_spec)?;
                println!("成功删除软件包: {}", package_spec);
                Ok(())
            }
            SourceCommand::Update => {
                update_source_index()?;
                println!("成功更新仓库索引");
                Ok(())
            }
            SourceCommand::Sync { remote_url, mirror } => {
                sync_from_remote(&remote_url, mirror).await?;
                println!("成功从 {} 同步软件源", remote_url);
                Ok(())
            }
        },
        Commands::Local { command } => match command {
            LocalCommand::New { path } => {
                create_local_repo(&path)?;
                println!("成功创建本地软件仓库于: {}", path);
                Ok(())
            }
            LocalCommand::Update => {
                update_local_index().await?;
                println!("成功更新本地索引");
                Ok(())
            }
            LocalCommand::Upgrade => {
                // 先更新本地索引
                update_local_index().await?;

                // 获取本地索引（GlobalIndex）的 local 和 remote 部分
                let repo_root = std::env::current_dir()?;
                let index_path = repo_root.join("index.json");
                if !index_path.exists() {
                    anyhow::bail!("索引文件不存在，请先运行 local update");
                }
                let content = fs::read_to_string(&index_path)?;
                let global_index: GlobalIndex = serde_json::from_str(&content)?;
            
                let local_index = global_index.local;
                let remote_index = global_index.remote;

                // 找出需要更新的包
                let updates = compare_versions(&local_index, &remote_index);
                if updates.is_empty() {
                    println!("所有软件包已是最新版本");
                    return Ok(());
                }

                println!("以下软件包可升级:");
                for update in &updates {
                    println!(
                        "- {}: {} -> {}",
                        update.id, update.current_version, update.latest_version
                    );
                }

                // 确认升级
                if !cli.yes {
                    println!("确定要升级这些软件包吗？(y/N)");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    if !input.trim().eq_ignore_ascii_case("y") {
                        println!("升级已取消");
                        return Ok(());
                    }
                }

                // 从配置获取远程仓库URL
                let repo_root = std::env::current_dir()?;
                let config_path = repo_root.join("config.toml");
                let config_content = fs::read_to_string(&config_path)?;
                let config: toml::Table = toml::from_str(&config_content)?;
                let remote_url = config["remote"]["url"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("配置文件中缺少远程仓库 URL"))?;

                // 执行升级
                for update in &updates {
                    println!("正在升级 {}...", update.id);

                    // 下载新版本
                    let package_url = format!(
                        "{}/applications/{}/{}/",
                        remote_url.trim_end_matches('/'),
                        update.id,
                        update.latest_version
                    );

                    let target_dir = repo_root
                        .join("local")
                        .join("applications")
                        .join(&update.id)
                        .join(&update.latest_version);

                    fs::create_dir_all(&target_dir)?;

                    // 下载元数据
                    let metadata_url = format!("{}metadata.json", package_url);
                    let metadata_path = target_dir.join("metadata.json");
                    download_file(&metadata_url, &metadata_path).await?;

                    // 下载应用文件
                    let metadata: Metadata =
                        serde_json::from_str(&fs::read_to_string(&metadata_path)?)?;
                    for (file_path, _) in metadata.all_files {
                        let file_url = format!("{}{}", package_url, file_path);
                        let dest_path = target_dir.join(file_path);

                        if let Some(parent) = dest_path.parent() {
                            fs::create_dir_all(parent)?;
                        }

                        download_file(&file_url, &dest_path).await?;
                    }

                    // 更新 versions.txt
                    let versions_file = target_dir.parent().unwrap().join("versions.txt");
                    let mut versions = get_versions_from_file(&versions_file).unwrap_or_default();
                    if !versions.contains(&update.latest_version) {
                        versions.push(update.latest_version.clone());
                        write_versions_to_file(&versions_file, &versions)?;
                    }

                    println!("成功升级 {} 到 {}", update.id, update.latest_version);
                }

                // 更新本地索引
                let index_path = repo_root.join("index.json");
                let mut global_index: GlobalIndex = if index_path.exists() {
                    let content = fs::read_to_string(&index_path)?;
                    serde_json::from_str(&content)?
                } else {
                    GlobalIndex {
                        local: Vec::new(),
                        remote: Vec::new(),
                    }
                };

                // 更新每个升级包的版本信息
                for update in &updates {
                    if let Some(app) = global_index.local.iter_mut().find(|app| app.id == update.id) {
                        app.latest_version = update.latest_version.clone();
                    }
                }

                // 写入更新后的索引
                fs::write(&index_path, serde_json::to_string_pretty(&global_index)?)?;

                println!("所有软件包升级完成");
                Ok(())
            }
            LocalCommand::Install { package_name } => {
                // 先更新本地索引
                update_local_index().await?;

                // 获取远程索引
                let repo_root = std::env::current_dir()?;
                let remote_index_path = repo_root.join("remote").join("index.json");
                if !remote_index_path.exists() {
                    anyhow::bail!("远程索引不存在，请先运行 local update");
                }
                let content = fs::read_to_string(&remote_index_path)?;
                let remote_index: Vec<AppIndex> = serde_json::from_str(&content)?;

                // 查找要安装的包
                let package = remote_index
                    .iter()
                    .find(|app| app.id == package_name)
                    .ok_or_else(|| anyhow::anyhow!("找不到软件包: {}", package_name))?;

                // 从配置获取远程仓库URL
                let config_path = repo_root.join("config.toml");
                let config_content = fs::read_to_string(&config_path)?;
                let config: toml::Table = toml::from_str(&config_content)?;
                let remote_url = config["remote"]["url"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("配置文件中缺少远程仓库 URL"))?;

                println!("正在安装 {}...", package_name);

                // 下载软件包
                let package_url = format!(
                    "{}/applications/{}/{}/",
                    remote_url.trim_end_matches('/'),
                    package.id,
                    package.latest_version
                );

                let target_dir = repo_root
                    .join("local")
                    .join("applications")
                    .join(&package.id)
                    .join(&package.latest_version);

                fs::create_dir_all(&target_dir)?;

                // 下载元数据
                let metadata_url = format!("{}metadata.json", package_url);
                let metadata_path = target_dir.join("metadata.json");
                download_file(&metadata_url, &metadata_path).await?;

                // 下载应用文件
                let metadata: Metadata =
                    serde_json::from_str(&fs::read_to_string(&metadata_path)?)?;
                for (file_path, _) in metadata.all_files {
                    let file_url = format!("{}{}", package_url, file_path);
                    let dest_path = target_dir.join(file_path);

                    if let Some(parent) = dest_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    download_file(&file_url, &dest_path).await?;
                }

                // 更新 versions.txt
                let versions_file = target_dir.parent().unwrap().join("versions.txt");
                let mut versions = get_versions_from_file(&versions_file).unwrap_or_default();
                if !versions.contains(&package.latest_version) {
                    versions.push(package.latest_version.clone());
                    write_versions_to_file(&versions_file, &versions)?;
                }

                // 更新本地索引（local部分）
                let local_index_path = repo_root.join("index.json");
                let mut global_index: GlobalIndex = if local_index_path.exists() {
                    let content = fs::read_to_string(&local_index_path)?;
                    serde_json::from_str(&content)?
                } else {
                    GlobalIndex {
                        local: Vec::new(),
                        remote: Vec::new(),
                    }
                };
            
                // 检查该包是否已在本地索引中，如果不在则添加
                if !global_index.local.iter().any(|app| app.id == package.id) {
                    global_index.local.push(AppIndex {
                        id: package.id.clone(),
                        name: package.name.clone(),
                        author: package.author.clone(),
                        latest_version: package.latest_version.clone(),
                        description: package.description.clone(),
                        location: format!("applications/{}/{}", package.id, package.latest_version),
                    });
                    fs::write(
                        &local_index_path,
                        serde_json::to_string_pretty(&global_index)?,
                    )?;
                }

                println!("成功安装 {} 版本 {}", package_name, package.latest_version);
                Ok(())
            }
            LocalCommand::Remove { package_name } => {
                // 获取仓库根目录
                let repo_root = std::env::current_dir()?;
                let local_dir = repo_root.join("local");

                // 构建应用目录路径
                let app_dir = local_dir.join("applications").join(&package_name);
                if !app_dir.exists() {
                    anyhow::bail!("找不到软件包: {}", package_name);
                }

                // 删除应用目录
                fs::remove_dir_all(&app_dir)?;
                println!("已卸载软件包: {}", package_name);

                // 更新本地索引
                let local_index_path = local_dir.join("index.json");
                if local_index_path.exists() {
                    let mut index: Vec<AppIndex> =
                        serde_json::from_str(&fs::read_to_string(&local_index_path)?)?;
                    index.retain(|app| app.id != package_name);
                    fs::write(&local_index_path, serde_json::to_string_pretty(&index)?)?;
                }

                Ok(())
            }
        },
    }
}

/// 确认操作
fn confirm_action(command: &Commands) -> Result<bool> {
    let prompt = match command {
        Commands::App { command } => match command {
            AppCommand::New { name } => format!("确定要创建新应用包 '{}' 吗？(y/N)", name),
            AppCommand::Init => "确定要在当前目录初始化应用包吗？(y/N)".to_string(),
            AppCommand::Add { path } => format!("确定要添加路径 '{}' 到应用包吗？(y/N)", path),
        },
        Commands::Source { command } => match command {
            SourceCommand::New { path } => format!("确定要创建新软件源仓库于 '{}' 吗？(y/N)", path),
            SourceCommand::Add { package_path } => {
                format!("确定要添加软件包 '{}' 到仓库吗？(y/N)", package_path)
            }
            SourceCommand::Remove { package_spec } => {
                format!("确定要从仓库删除软件包 '{}' 吗？(y/N)", package_spec)
            }
            SourceCommand::Update => "确定要更新仓库索引吗？(y/N)".to_string(),
            SourceCommand::Sync { remote_url, mirror } => format!(
                "确定要从 {} {}同步软件源吗？(y/N)",
                remote_url,
                if *mirror { "镜像" } else { "" }
            ),
        },
        Commands::Local { command } => match command {
            LocalCommand::New { path } => format!("确定要创建新本地仓库于 '{}' 吗？(y/N)", path),
            LocalCommand::Update => "确定要更新本地索引吗？(y/N)".to_string(),
            LocalCommand::Upgrade => "确定要升级已安装软件包吗？(y/N)".to_string(),
            LocalCommand::Install { package_name } => {
                format!("确定要安装软件包 '{}' 吗？(y/N)", package_name)
            }
            LocalCommand::Remove { package_name } => {
                format!("确定要卸载软件包 '{}' 吗？(y/N)", package_name)
            }
        },
    };

    println!("{}", prompt);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}

/// 检查目录项是否是 dot 文件或目录（以点开头）
fn is_dot_file_or_dir(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn initialize_package(base_path: &Path) -> Result<()> {
    let metadata_path = base_path.join("metadata.json");

    if metadata_path.exists() {
        anyhow::bail!("metadata.json 已存在");
    }

    // 遍历目录生成文件哈希
    let mut all_files = HashMap::new();
    for entry in WalkDir::new(base_path)
        .into_iter()
        .filter_entry(|e| !is_dot_file_or_dir(e)) // 跳过 dot 文件（夹）
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel_path = entry
            .path()
            .strip_prefix(base_path)?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("无效的文件路径"))?
            .replace('\\', "/");

        // 跳过metadata.json本身
        if rel_path == "metadata.json" {
            continue;
        }

        let content = fs::read(entry.path())?;
        let hash = format!("{:x}", sha2::Sha256::digest(&content));
        all_files.insert(rel_path, hash);
    }

    let mut default_metadata = serde_json::json!({
        "name": "",
        "id": "",
        "version": "0.1.0",
        "description": "",
        "author": "",
        "type": "",
        "category": "",
        "permissions": [],
        "entry": "index.html",
        "all_files": all_files
    });

    // 保留用户可能已填写的基础字段
    if let Ok(existing_content) = fs::read_to_string(&metadata_path) {
        if let Ok(existing) = serde_json::from_str::<serde_json::Value>(&existing_content) {
            for field in [
                "name",
                "id",
                "version",
                "description",
                "author",
                "type",
                "category",
            ] {
                if existing.get(field).is_some() {
                    default_metadata[field] = existing[field].clone();
                }
            }
        }
    }

    fs::write(
        &metadata_path,
        serde_json::to_string_pretty(&default_metadata)?,
    )
    .context("写入 metadata.json 失败")?;

    Ok(())
}

/// 添加文件或目录到应用包的 metadata.json
fn handle_app_add(path: &str) -> Result<()> {
    let base_path = std::env::current_dir()?;
    let target_path = base_path.join(path);
    let metadata_path = base_path.join("metadata.json");

    // 检查 metadata.json 是否存在
    if !metadata_path.exists() {
        anyhow::bail!("当前目录下没有 metadata.json 文件，请先运行 app init");
    }

    // 读取并解析 metadata.json
    let metadata_content = fs::read_to_string(&metadata_path)?;
    let mut metadata: Metadata = serde_json::from_str(&metadata_content)?;

    // 计算文件/目录的哈希
    let mut new_files = HashMap::new();
    if target_path.is_file() {
        let rel_path = target_path
            .strip_prefix(&base_path)?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("路径包含无效字符"))?
            .replace('\\', "/");
        let content = fs::read(&target_path)?;
        let hash = format!("{:x}", sha2::Sha256::digest(&content));
        new_files.insert(rel_path, hash);
    } else if target_path.is_dir() {
        for entry in WalkDir::new(&target_path)
            .into_iter()
            .filter_entry(|e| !is_dot_file_or_dir(e))
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let entry_path = entry.path();
            let rel_path = entry_path
                .strip_prefix(&base_path)?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("路径包含无效字符"))?
                .replace('\\', "/");
            let content = fs::read(entry_path)?;
            let hash = format!("{:x}", sha2::Sha256::digest(&content));
            new_files.insert(rel_path, hash);
        }
    } else {
        anyhow::bail!("路径不存在: {}", path);
    }

    // 更新 metadata 的 all_files
    let num_files = new_files.len(); // 先保存长度
    for (file, hash) in new_files {
        metadata.all_files.insert(file, hash);
    }

    // 写回 metadata.json
    fs::write(&metadata_path, serde_json::to_string_pretty(&metadata)?)?;
    println!("成功添加 {} 个文件到 metadata.json", num_files); // 使用保存的长度
    Ok(())
}

fn create_source_repo(path: &str) -> Result<()> {
    let repo_path = Path::new(path);
    if repo_path.exists() {
        anyhow::bail!("目录 '{}' 已存在", path);
    }

    fs::create_dir(repo_path).with_context(|| format!("创建目录 '{}' 失败", path))?;

    // 创建 applications 目录
    let applications_path = repo_path.join("applications");
    fs::create_dir(&applications_path).context("创建 applications 目录失败")?;

    // 创建 config.toml 文件
    let config_path = repo_path.join("config.toml");
    fs::write(&config_path, "# 软件源配置\n").context("写入 config.toml 失败")?;

    // 创建 index.json 文件
    let index_path = repo_path.join("index.json");
    fs::write(&index_path, "[]").context("写入 index.json 失败")?;

    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Metadata {
    id: String,
    version: String,
    name: String,
    author: String,
    description: Option<String>,
    #[serde(rename = "type")]
    app_type: Option<String>,
    category: Option<String>,
    permissions: Vec<String>,
    entry: String,
    all_files: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct AppIndex {
    id: String,
    name: String,
    author: String,
    latest_version: String,
    description: Option<String>,
    location: String,
}

#[derive(Serialize, Deserialize)]
struct GlobalIndex {
    local: Vec<AppIndex>,
    remote: Vec<AppIndex>,
}

fn add_package_to_repo(package_path: &str) -> Result<()> {
    let package_path = Path::new(package_path);
    let metadata_path = package_path.join("metadata.json");

    // 读取并解析元数据
    let metadata_content = fs::read_to_string(&metadata_path)
        .with_context(|| format!("无法读取文件: {}", metadata_path.display()))?;
    let metadata: Metadata = serde_json::from_str(&metadata_content)
        .with_context(|| format!("解析 metadata.json 失败: {}", metadata_path.display()))?;

    // 获取当前工作目录（仓库根目录）
    let repo_root = std::env::current_dir()?;
    let applications_dir = repo_root.join("applications");
    let app_dir = applications_dir.join(&metadata.id);
    let version_dir = app_dir.join(&metadata.version);

    // 创建应用目录和版本目录
    fs::create_dir_all(&version_dir)
        .with_context(|| format!("创建目录失败: {}", version_dir.display()))?;

    // 复制整个软件包
    copy_dir_all(package_path, &version_dir).with_context(|| "复制软件包失败".to_string())?;

    // 更新 versions.txt
    let versions_file = app_dir.join("versions.txt");
    let mut versions = get_versions_from_file(&versions_file)?;

    if !versions.contains(&metadata.version) {
        versions.push(metadata.version.clone());
        write_versions_to_file(&versions_file, &versions)?;
    }

    // 更新索引文件
    let index_file = repo_root.join("index.json");
    let mut global_index: GlobalIndex = if index_file.exists() {
        let content = fs::read_to_string(&index_file)?;
        serde_json::from_str(&content)?
    } else {
        GlobalIndex {
            local: Vec::new(),
            remote: Vec::new(),
        }
    };

    // 更新或添加索引条目到 remote 数组
    if let Some(existing) = global_index.remote.iter_mut().find(|app| app.id == metadata.id) {
        existing.latest_version = metadata.version.clone();
        existing.name = metadata.name.clone();
        existing.author = metadata.author.clone();
        existing.description = metadata.description.clone();
    } else {
        global_index.remote.push(AppIndex {
            id: metadata.id.clone(),
            name: metadata.name.clone(),
            author: metadata.author.clone(),
            latest_version: metadata.version.clone(),
            description: metadata.description.clone(),
            location: format!("applications/{}/{}", metadata.id, metadata.version),
        });
    }

    // 写入更新后的索引
    fs::write(&index_file, serde_json::to_string_pretty(&global_index)?)
        .with_context(|| format!("写入索引文件失败: {}", index_file.display()))?;

    Ok(())
}

/// 递归复制目录及其内容
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // 跳过 dot 文件（夹）
        if file_name_str.starts_with('.') {
            continue;
        }

        let target = dst.as_ref().join(&file_name);

        if ty.is_dir() {
            fs::create_dir_all(&target)?; // 只在需要时创建目录
            copy_dir_all(entry.path(), target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?; // 确保父目录存在
            }
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// 从仓库删除软件（版本）
fn remove_package_from_repo(package_spec: &str) -> Result<()> {
    // 分割包名和版本
    let parts: Vec<&str> = package_spec.split(':').collect();
    if parts.len() != 2 {
        anyhow::bail!("无效的软件包格式，应为 '包名:版本'");
    }
    let package_name = parts[0];
    let version = parts[1];

    // 获取当前工作目录（仓库根目录）
    let repo_root = std::env::current_dir()?;
    let applications_dir = repo_root.join("applications");
    let package_dir = applications_dir.join(package_name);
    let version_dir = package_dir.join(version);

    // 检查版本目录是否存在
    if !version_dir.exists() {
        anyhow::bail!("版本目录不存在: {}", version_dir.display());
    }

    // 删除版本目录
    fs::remove_dir_all(&version_dir)?;

    // 更新 versions.txt
    let versions_file = package_dir.join("versions.txt");
    let versions = get_versions_from_file(&versions_file)?;
    let new_versions: Vec<String> = versions.iter().filter(|v| *v != version).cloned().collect();

    if new_versions.is_empty() {
        // 如果没有其他版本，删除整个包目录
        fs::remove_dir_all(&package_dir)?;
    } else {
        // 更新 versions.txt 文件
        write_versions_to_file(&versions_file, &new_versions)?;
    }

    // 更新索引文件 index.json
    let index_file = repo_root.join("index.json");
    if index_file.exists() {
        let index_content = fs::read_to_string(&index_file)?;
        let mut global_index: GlobalIndex = serde_json::from_str(&index_content)?;

        // 更新 remote 数组：移除已删除的包（如果包目录不存在）或更新最新版本
        global_index.remote.retain(|app| app.id != package_name || package_dir.exists());

        // 如果包还存在，更新 remote 数组中该包的最新版本
        if package_dir.exists() {
            if let Some(app) = global_index.remote.iter_mut().find(|app| app.id == package_name) {
                let versions_content = fs::read_to_string(&package_dir.join("versions.txt"))?;
                let versions: Vec<&str> = versions_content.lines().collect();
                if let Some(latest) = versions.last() {
                    app.latest_version = latest.to_string();
                }
            }
        }

        // 写入更新后的索引
        fs::write(&index_file, serde_json::to_string_pretty(&global_index)?)?;
    }

    Ok(())
}

fn get_versions_from_file(path: &Path) -> Result<Vec<String>> {
    if path.exists() {
        let content = fs::read_to_string(path)?;
        Ok(content.lines().map(|s| s.to_string()).collect())
    } else {
        Ok(Vec::new())
    }
}

#[derive(Debug)]
struct PackageUpdate {
    id: String,
    current_version: String,
    latest_version: String,
}

/// 获取本地索引
fn get_local_index() -> Result<Vec<AppIndex>> {
    let index_file = std::env::current_dir()?.join("index.json");
    if !index_file.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&index_file)?;
    serde_json::from_str(&content).context("解析本地索引失败")
}

/// 比较本地和远程版本，返回需要更新的包
fn compare_versions(local: &[AppIndex], remote: &[AppIndex]) -> Vec<PackageUpdate> {
    let mut updates = Vec::new();

    for remote_app in remote {
        if let Some(local_app) = local.iter().find(|a| a.id == remote_app.id) {
            if local_app.latest_version != remote_app.latest_version {
                updates.push(PackageUpdate {
                    id: remote_app.id.clone(),
                    current_version: local_app.latest_version.clone(),
                    latest_version: remote_app.latest_version.clone(),
                });
            }
        } else {
            // 新应用
            updates.push(PackageUpdate {
                id: remote_app.id.clone(),
                current_version: "无".to_string(),
                latest_version: remote_app.latest_version.clone(),
            });
        }
    }

    updates
}

/// 下载文件并保存到本地
async fn download_file(url: &str, path: &Path) -> Result<()> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("下载文件失败: {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("下载文件失败: {} - {}", url, response.status());
    }

    let content = response
        .bytes()
        .await
        .with_context(|| format!("读取文件内容失败: {}", url))?;

    fs::write(path, content).with_context(|| format!("保存文件失败: {}", path.display()))?;

    Ok(())
}

fn write_versions_to_file(path: &Path, versions: &[String]) -> Result<()> {
    fs::write(path, versions.join("\n"))?;
    Ok(())
}

/// 从远程仓库同步应用包
async fn sync_from_remote(remote_url: &str, mirror: bool) -> Result<()> {
    println!("正在从 {} 同步……", remote_url);
    println!("镜像模式: {}", mirror);

    // 获取远程索引
    let remote_index_url = format!("{}/index.json", remote_url.trim_end_matches('/'));
    println!("正在获取远程索引: {}", remote_index_url);

    let response = reqwest::get(&remote_index_url)
        .await
        .with_context(|| format!("请求远程索引失败: {}", remote_index_url))?;

    if !response.status().is_success() {
        anyhow::bail!("远程服务器返回错误状态: {}", response.status());
    }

    let remote_index: Vec<AppIndex> = response
        .json()
        .await
        .with_context(|| "解析远程索引JSON失败".to_string())?;

    println!("成功获取到 {} 个应用的索引", remote_index.len());

    // 获取本地索引
    let local_index = get_local_index()?;

    // 比较版本并找出需要更新的应用
    let updates = compare_versions(&local_index, &remote_index);
    println!("需要更新 {} 个应用包", updates.len());

    if !updates.is_empty() {
        println!("需要更新的应用包:");
        for update in &updates {
            println!(
                "- {} (当前: {}, 最新: {})",
                update.id, update.current_version, update.latest_version
            );
        }

        // 镜像模式：删除所有本地应用，只保留远程应用
        if mirror {
            println!("镜像模式 - 清空本地仓库……");
            let applications_dir = std::env::current_dir()?.join("applications");
            if applications_dir.exists() {
                fs::remove_dir_all(&applications_dir)?;
                fs::create_dir(&applications_dir)?;
            }
        }

        // 下载新增/更新的应用包
        for update in updates {
            let package_url = format!(
                "{}/applications/{}/{}/",
                remote_url.trim_end_matches('/'),
                update.id,
                update.latest_version
            );

            println!("正在下载: {}", package_url);

            // 创建目标目录
            let target_dir = std::env::current_dir()?
                .join("applications")
                .join(&update.id)
                .join(&update.latest_version);

            fs::create_dir_all(&target_dir)
                .with_context(|| format!("创建目录失败: {}", target_dir.display()))?;

            // 下载元数据文件
            let metadata_url = format!("{}metadata.json", package_url);
            let metadata_path = target_dir.join("metadata.json");
            download_file(&metadata_url, &metadata_path).await?;

            // 解析元数据获取文件列表
            let metadata: Metadata = serde_json::from_str(&fs::read_to_string(&metadata_path)?)
                .context("解析元数据失败")?;

            // 下载所有文件
            for (file_path, _) in metadata.all_files {
                let file_url = format!("{}{}", package_url, file_path);
                let file_path = target_dir.join(file_path);

                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                download_file(&file_url, &file_path).await?;
            }

            // 更新versions.txt
            let versions_file = target_dir.parent().unwrap().join("versions.txt");
            let mut versions = get_versions_from_file(&versions_file)?;
            if !versions.contains(&update.latest_version) {
                versions.push(update.latest_version.clone());
                write_versions_to_file(&versions_file, &versions)?;
            }

            println!("成功下载: {}", update.id);
        }

        // 更新本地索引
        update_source_index()?;
        println!("本地索引已更新");
    } else {
        println!("所有应用包都是最新版本");
    }

    Ok(())
}

/// 更新仓库索引
fn update_source_index() -> Result<()> {
    let repo_root = std::env::current_dir()?;
    let applications_dir = repo_root.join("applications");
    let index_file = repo_root.join("index.json");

    // 如果 applications 目录不存在，则创建一个空的 GlobalIndex
    if !applications_dir.exists() {
        let empty_index = GlobalIndex {
            local: Vec::new(),
            remote: Vec::new(),
        };
        fs::write(&index_file, serde_json::to_string_pretty(&empty_index)?)?;
        return Ok(());
    }

    // 读取现有的索引文件（如果存在），我们只更新 remote 部分，保留 local 部分
    let mut global_index: GlobalIndex = if index_file.exists() {
        let content = fs::read_to_string(&index_file)?;
        serde_json::from_str(&content)?
    } else {
        GlobalIndex {
            local: Vec::new(),
            remote: Vec::new(),
        }
    };

    let mut remote_index = Vec::new();

    // 遍历所有应用目录，构建 remote 索引
    for app_dir_entry in fs::read_dir(applications_dir)? {
        let app_dir_entry = app_dir_entry?;
        let app_dir_path = app_dir_entry.path();
        if !app_dir_path.is_dir() {
            continue;
        }

        // 获取应用ID
        let app_id = app_dir_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("无效的应用目录名: {:?}", app_dir_path))?
            .to_string();

        // 读取 versions.txt 文件
        let versions_file = app_dir_path.join("versions.txt");
        if !versions_file.exists() {
            eprintln!("警告: 应用 '{}' 没有 versions.txt 文件，跳过", app_id);
            continue;
    }

        let versions_content = fs::read_to_string(&versions_file)?;
        let versions: Vec<&str> = versions_content.lines().collect();

        if versions.is_empty() {
            eprintln!("警告: 应用 '{}' 没有版本，跳过", app_id);
            continue;
        }

        // 获取最新版本（最后一个版本）
        let latest_version = versions.last().unwrap().to_string();

        // 读取最新版本目录下的 metadata.json
        let version_dir = app_dir_path.join(&latest_version);
        let metadata_path = version_dir.join("metadata.json");
        if !metadata_path.exists() {
            eprintln!(
                "警告: 应用 '{}' 的版本 '{}' 没有 metadata.json 文件，跳过",
                app_id, latest_version
            );
            continue;
        }

        let metadata_content = fs::read_to_string(&metadata_path)
            .with_context(|| format!("无法读取元数据文件: {}", metadata_path.display()))?;
        let metadata: Metadata = serde_json::from_str(&metadata_content)
            .with_context(|| format!("解析元数据失败: {}", metadata_path.display()))?;

        // 构建索引项
        remote_index.push(AppIndex {
            id: app_id.clone(),
            name: metadata.name,
            author: metadata.author,
            latest_version: latest_version.clone(),
            description: metadata.description,
            location: format!("applications/{}/{}", app_id, latest_version),
        });
    }

    // 更新 global_index 的 remote 部分
    global_index.remote = remote_index;

    // 写入更新后的索引
    fs::write(&index_file, serde_json::to_string_pretty(&global_index)?)?;

    Ok(())
}

/// 更新本地仓库索引
async fn update_local_index() -> Result<()> {
    // 读取本地仓库配置
    let repo_root = std::env::current_dir()?;
    let config_path = repo_root.join("config.toml");

    if !config_path.exists() {
        anyhow::bail!("找不到 config.toml 文件");
    }

    let config_content = fs::read_to_string(&config_path)?;
    let config: toml::Table = toml::from_str(&config_content)?;

    // 获取远程仓库 URLs (支持单 URL 或多镜像源)
    let remote_urls: Vec<String> = if let Some(url) = config["remote"]["url"].as_str() {
        vec![url.to_string()]
    } else if let Some(urls) = config["remote"]["urls"].as_array() {
        urls.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        anyhow::bail!("配置文件中缺少远程仓库 URL 或 URLs 配置");
    };

    // 尝试每个镜像源直到成功
    let mut last_error = None;
    let mut remote_index: Option<Vec<AppIndex>> = None;
    for remote_url in &remote_urls {
        println!("正在尝试从远程仓库 {} 更新索引……", remote_url);

        let remote_index_url = format!("{}/index.json", remote_url.trim_end_matches('/'));
        let response = match reqwest::get(&remote_index_url).await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                last_error = Some(format!(
                    "镜像源 {} 返回错误状态: {}",
                    remote_url,
                    r.status()
                ));
                continue;
            }
            Err(e) => {
                last_error = Some(format!("镜像源 {} 连接失败: {}", remote_url, e));
                continue;
            }
        };

        match response.json::<Vec<AppIndex>>().await {
            Ok(index) => {
                remote_index = Some(index);
                break;
            }
            Err(e) => {
                last_error = Some(format!("镜像源 {} 解析索引失败: {}", remote_url, e));
                continue;
            }
        }
    }

    let remote_index = remote_index.ok_or_else(|| {
        anyhow::anyhow!(
            "所有镜像源尝试失败:\n{}",
            last_error.unwrap_or_else(|| "未知错误".to_string())
        )
    })?;

    // 获取本地索引（GlobalIndex）
    let local_index_path = repo_root.join("index.json");
    let mut global_index: GlobalIndex = if local_index_path.exists() {
        let content = fs::read_to_string(&local_index_path)?;
        serde_json::from_str(&content)?
    } else {
        GlobalIndex {
            local: Vec::new(),
            remote: Vec::new(),
        }
    };

    // 比较差异：使用 global_index.remote 作为本地当前存储的远程索引
    let updates = compare_versions(&global_index.remote, &remote_index);

    if updates.is_empty() {
        println!("本地索引已是最新");
        return Ok(());
    }

    // 更新 global_index 的 remote 部分
    global_index.remote = remote_index;

    // 写入更新后的索引
    fs::write(
        &local_index_path,
        serde_json::to_string_pretty(&global_index)?,
    )?;

    println!("成功更新本地索引，共更新 {} 个应用", updates.len());
    Ok(())
}

/// 创建本地软件仓库
fn create_local_repo(path: &str) -> Result<()> {
    let repo_path = Path::new(path);
    if repo_path.exists() {
        anyhow::bail!("目录 '{}' 已存在", path);
    }

    fs::create_dir(repo_path).with_context(|| format!("创建目录 '{}' 失败", path))?;

    // 创建本地仓库目录结构
    let local_dir = repo_path.join("local");
    fs::create_dir(&local_dir).context("创建 local 目录失败")?;

    let applications_dir = local_dir.join("applications");
    fs::create_dir(&applications_dir).context("创建 applications 目录失败")?;

    // 创建 remote 目录
    let remote_dir = repo_path.join("remote");
    fs::create_dir(&remote_dir).context("创建 remote 目录失败")?;

    // 创建 config.toml 文件
    let config_path = repo_path.join("config.toml");
    fs::write(&config_path, "# 本地仓库配置\n").context("写入 config.toml 失败")?;

    // 创建 index.json 文件
    let index_path = repo_path.join("index.json");
    fs::write(&index_path, "[]").context("写入 index.json 失败")?;

    Ok(())
}
