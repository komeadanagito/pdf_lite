# PDF Lite

一个使用 Rust + Tauri 2 + React 构建的跨平台 PDF 压缩桌面应用。

## 功能

- 四档压缩模式：无损、轻度、标准、极限
- 支持批量添加 PDF 文件
- 支持拖拽添加文件
- 支持压缩后大小对比
- 支持任务进度事件

## 开发运行

```bash
source "$HOME/.cargo/env"
npm install
npm run tauri:dev
```

## 打包

```bash
source "$HOME/.cargo/env"
npm run tauri:build
```

## Windows 使用方式

在 Windows 上，推荐使用打包后的安装器直接安装和启动：

1. 运行 `npm run tauri:build` 生成安装包。
2. 在 `src-tauri/target/release/bundle/nsis/` 目录中找到 `setup.exe` 安装器。
3. 双击安装器完成安装。
4. 安装完成后，可以从桌面快捷方式或开始菜单直接启动 `PDF Lite`。
5. 不需要手动打开终端，也不需要安装 Rust 环境才能使用已经打包好的程序。
