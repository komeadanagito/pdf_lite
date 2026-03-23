# File Lite

使用 Rust + Tauri 2 + React 构建的跨平台文件压缩应用，支持 PDF、图片和视频压缩。

## 功能

### PDF 压缩
- 四档模式：无损、轻度(200DPI)、标准(150DPI)、极限(100DPI)
- Ghostscript 引擎优先，纯 Rust 回退
- 批量添加、拖拽添加

### 图片压缩
- 支持 JPG、PNG、WebP、BMP、TIFF、GIF、SVG
- PNG：oxipng 无损优化 + imagequant 有损量化
- WebP：质量重编码
- SVG：usvg 解析简化
- 四档模式：无损、轻度(Q85)、标准(Q70)、极限(Q50)

### 视频压缩
- 支持 MP4、MOV、AVI、MKV
- FFmpeg H.264 编码
- 四档模式：无损(重封装)、轻度(CRF23)、标准(CRF28+1080p)、极限(CRF32+720p)

### 界面
- Apple 风格毛玻璃 UI
- 自定义标题栏
- Tab 切换（PDF / 图片 / 视频）
- 移动端自适应布局

## 开发运行

```bash
npm install
npm run tauri:dev
```

## 打包

### Windows

#### 1. 准备 Ghostscript（PDF 压缩引擎）

下载 [Ghostscript](https://ghostscript.com/releases/gsdnld.html)（10.x 推荐），将以下目录复制到 `src-tauri/resources/gs/`：

```
src-tauri/resources/gs/
  bin/          ← gswin64c.exe + gsdll*.dll
  lib/          ← 完整复制
  Resource/     ← 完整复制
  iccprofiles/  ← 完整复制
```

#### 2. 准备 FFmpeg（视频压缩引擎）

下载 [FFmpeg](https://ffmpeg.org/download.html) 静态构建版，将 `ffmpeg.exe` 放入：

```
src-tauri/resources/ffmpeg/
  ffmpeg.exe
```

#### 3. 执行打包

```bash
npm run tauri:build
```

安装包在 `src-tauri/target/release/bundle/nsis/` 目录下。

### Android

需要先安装 Android SDK、NDK 和 JDK，然后：

```bash
npm run tauri android init
npm run tauri android build
```

## 许可证

- Ghostscript 使用 AGPL 许可证，分发时请遵守 AGPL 合规要求
- FFmpeg 使用 LGPL/GPL 许可证，请根据所用构建版本遵守相应要求
