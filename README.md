# PageOS 网页应用包管理器 (pageos-pkgr)

[![License: MPL-2.0](https://img.shields.io/badge/License-MPL%202.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)

PageOS-pkgr 是 PageOS 系统的网页应用仓库管理工具，用于管理网页应用的安装、更新和仓库维护。

## 项目背景

PageOS 是一个基于 Arch Linux 的图形化发行版，采用 Wayland 下的 cage 显示全屏的 Firefox 浏览器作为用户界面。所有用户交互都在网页中实现，通过 Rust 双向服务端程序使用 WebSocket 进行系统交互。

## 功能特性

- 管理本地和在线网页应用仓库
- 支持应用包的安装、升级和卸载
- 提供应用包元数据管理 (metadata.json)
- 支持版本控制和增量更新
- 提供命令行界面进行仓库管理

## 仓库结构


```plaintext
$HOME/.local/share/pageos/  # 仓库存储根目录
├── packages/               # 已安装的包
│   ├── pageos.settings-manager/
│   │   ├── 1.0.0/
│   │   │   ├── metadata.json
│   │   │   └── ...         # 应用文件
│   │   ├── 1.1.0/
│   │   └── versions.txt
│   └── %PACKAGE_ID%/
│       └── %VERSION%/
├── config.toml             # 软件源等设置（官方源、镜像源）
└── index.json              # 全局索引文件
```

## 软件包结构

```plaintext
.                           # 一般是该包的 package-id 命名的文件夹
├── ...                     # 应用文件
├── target/
│   └── package-id.zip.papk # 打包好出的软件包文件
├── .gitignore              # 忽略 target 文件夹
└── metadata.json           # 全局索引文件
```

## 安装

```bash
cargo install pageos-pkgr
```

## 使用说明

### 应用包管理

```bash
# 创建新应用包
pageos-pkgr app new <package-name>

# 在当前目录初始化应用包
pageos-pkgr app init

# 添加文件到应用包
pageos-pkgr app add <file-path>
```

### 软件源管理

```bash
# 创建新软件源
pageos-pkgr source new <source-repo-path>

# 添加软件包到仓库
pageos-pkgr source add <package-path>

# 更新仓库索引
pageos-pkgr source update
```

### 本地仓库管理

```bash
# 创建本地仓库
pageos-pkgr local new <local-repo-path>

# 安装软件包
pageos-pkgr local install <package-name>

# 升级软件包
pageos-pkgr local upgrade
```

## 开发

### 依赖

- Rust 1.70+
- Cargo

### 构建

```bash
cargo build --release
```

## 许可证

本项目采用 [Mozilla Public License 2.0](https://www.mozilla.org/en-US/MPL/2.0/) 开源许可证。

## 贡献

欢迎提交 Pull Request 或 Issue 报告问题。

## 相关项目

- [pageos-greet](https://github.com/swaybien/pageos-greet) - PageOS 登录界面
- [pageos-core](https://github.com/swaybien/pageos-core) - PageOS 核心框架
- [pageos-apps](https://github.com/swaybien/pageos-apps) - 官方网页应用仓库
