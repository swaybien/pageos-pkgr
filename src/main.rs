// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

// 引入模块
mod app;
mod config;
mod crypto;
mod fsxg;
mod index;
mod metadata;
mod net;
mod path;
mod repo;
mod serde_utils;
mod transaction;
mod version;

// 定义命令行参数结构
#[derive(Parser)]
#[command(name = "pageos-pkgr")]
#[command(about = "PageOS 系统的网页应用仓库管理工具", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 应用包管理
    #[command(subcommand)]
    App(AppCommands),

    /// 仓库管理
    #[command(subcommand)]
    Repo(RepoCommands),
}

#[derive(Subcommand)]
enum AppCommands {
    /// 在指定目录初始化软件包
    #[command(arg_required_else_help = true)]
    Init {
        /// 软件包路径
        #[arg(default_value = ".")]
        package_path: PathBuf,
    },

    /// 创建新的软件包
    #[command(arg_required_else_help = true)]
    New {
        /// 软件包ID
        package_id: String,
        /// 基础目录
        #[arg(default_value = ".")]
        base_dir: PathBuf,
    },

    /// 添加文件或目录到软件包清单
    #[command(arg_required_else_help = true)]
    Add {
        /// 要添加的文件或目录路径
        path: PathBuf,
        /// 软件包路径
        #[arg(short, long, default_value = ".")]
        package: PathBuf,
    },

    /// 从软件包清单移除文件或目录
    #[command(arg_required_else_help = true)]
    Remove {
        /// 要移除的文件或目录路径
        path: PathBuf,
        /// 软件包路径
        #[arg(short, long, default_value = ".")]
        package: PathBuf,
    },
}

#[derive(Subcommand)]
enum RepoCommands {
    /// 在指定目录初始化应用仓库
    #[command(arg_required_else_help = true)]
    Init {
        /// 仓库路径
        repo_path: PathBuf,
    },

    /// 创建新的应用仓库
    #[command(arg_required_else_help = true)]
    New {
        /// 仓库名称
        repo_name: String,
        /// 基础目录
        #[arg(default_value = ".")]
        base_dir: PathBuf,
    },

    /// 清理仓库
    #[command(arg_required_else_help = true)]
    Clean {
        /// 仓库路径
        #[arg(short, long, default_value = "~/.local/share/pageos/")]
        repo: PathBuf,
    },

    /// 更新仓库索引
    #[command(arg_required_else_help = true)]
    Update {
        /// 仓库路径
        #[arg(short, long, default_value = "~/.local/share/pageos/")]
        repo: PathBuf,
        /// 本地更新模式
        #[arg(long)]
        local: bool,
    },

    /// 添加软件包到仓库
    #[command(arg_required_else_help = true)]
    Add {
        /// 软件包路径
        package_path: PathBuf,
        /// 仓库路径
        #[arg(short, long, default_value = "~/.local/share/pageos/")]
        repo: PathBuf,
    },

    /// 安装软件包
    #[command(arg_required_else_help = true)]
    Install {
        /// 软件源ID:软件包ID:版本
        source_package_version: String,
        /// 仓库路径
        #[arg(short, long, default_value = "~/.local/share/pageos/")]
        repo: PathBuf,
    },

    /// 卸载软件包
    #[command(arg_required_else_help = true)]
    Remove {
        /// 软件包ID:版本
        package_version: String,
        /// 仓库路径
        #[arg(short, long, default_value = "~/.local/share/pageos/")]
        repo: PathBuf,
    },

    /// 升级软件包
    #[command(arg_required_else_help = true)]
    Upgrade {
        /// 软件包ID
        package_id: String,
        /// 仓库路径
        #[arg(short, long, default_value = "~/.local/share/pageos/")]
        repo: PathBuf,
    },

    /// 同步仓库
    #[command(arg_required_else_help = true)]
    Sync {
        /// 软件源ID
        source_id: Option<String>,
        /// 镜像同步模式
        #[arg(long)]
        mirror: bool,
        /// 仓库路径
        #[arg(short, long, default_value = "~/.local/share/pageos/")]
        repo: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::App(app_cmd) => match app_cmd {
            AppCommands::Init { package_path } => {
                app::init(package_path)?;
                println!("已成功在 {} 初始化应用包", package_path.display());
            }
            AppCommands::New {
                package_id,
                base_dir,
            } => {
                let package_path = app::new(package_id, base_dir)?;
                println!("已成功创建新应用包: {}", package_path.display());
            }
            AppCommands::Add { path, package } => {
                app::add_file(path, package)?;
                println!("已成功添加 {} 到软件包清单", path.display());
            }
            AppCommands::Remove { path, package } => {
                app::remove_file(path, package)?;
                println!("已成功从软件包清单移除 {}", path.display());
            }
        },
        Commands::Repo(repo_cmd) => {
            match repo_cmd {
                RepoCommands::Init { repo_path } => {
                    repo::RepoManager::init(repo_path)?;
                    println!("已成功在 {} 初始化应用仓库", repo_path.display());
                }
                RepoCommands::New {
                    repo_name,
                    base_dir,
                } => {
                    let _repo_manager = repo::RepoManager::new(repo_name, base_dir)?;
                    println!("已成功创建新应用仓库");
                }
                RepoCommands::Clean { repo } => {
                    let mut repo_manager = repo::RepoManager::open(repo.clone())?;
                    repo_manager.clean()?;
                    println!("已成功清理仓库 {}", repo.display());
                }
                RepoCommands::Update { repo, local } => {
                    let mut repo_manager = repo::RepoManager::open(repo.clone())?;
                    if *local {
                        // 更新本地索引
                        repo_manager.update_local_index()?;
                        println!("已成功更新本地索引");
                    } else {
                        // 更新索引 source 部分
                        repo_manager.update_source_index().await?;
                        println!("已成功更新源索引");
                    }
                }
                RepoCommands::Add { package_path, repo } => {
                    let mut repo_manager = repo::RepoManager::open(repo.clone())?;
                    repo_manager.add_package(package_path)?;
                    println!("已成功添加软件包到仓库");
                }
                RepoCommands::Install {
                    source_package_version,
                    repo,
                } => {
                    let mut repo_manager = repo::RepoManager::open(repo.clone())?;
                    repo_manager
                        .install_package(source_package_version, None)
                        .await?;
                    println!("已成功安装软件包 {source_package_version}");
                }
                RepoCommands::Remove {
                    package_version,
                    repo,
                } => {
                    // 解析 package:version
                    let parts: Vec<&str> = package_version.split(':').collect();
                    let package_id = parts[0];
                    let version = if parts.len() > 1 {
                        Some(parts[1])
                    } else {
                        None
                    };

                    let mut repo_manager = repo::RepoManager::open(repo.clone())?;
                    repo_manager.remove_package(package_id, version)?;
                    println!("已成功卸载软件包 {package_id}");
                }
                RepoCommands::Upgrade { package_id, repo } => {
                    let mut repo_manager = repo::RepoManager::open(repo.clone())?;
                    repo_manager.upgrade_package(package_id).await?;
                    println!("已成功升级软件包 {package_id}");
                }
                RepoCommands::Sync {
                    source_id,
                    mirror,
                    repo,
                } => {
                    let source_id = source_id.as_deref().unwrap_or("default");
                    let mut repo_manager = repo::RepoManager::open(repo.clone())?;
                    repo_manager.sync_repository(source_id, *mirror).await?;
                    println!("已成功同步仓库");
                }
            }
        }
    }

    Ok(())
}
