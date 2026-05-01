use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, path::BaseDirectory};

use crate::compress::{CompressResult, CompressionMode};

#[derive(Debug, Clone, Serialize)]
struct ProgressEvent {
    path: String,
    stage: String,
    completed: usize,
    total: usize,
}

fn emit(app: &AppHandle, path: &Path, stage: &str, done: usize, total: usize) {
    let _ = app.emit(
        "compress-progress",
        ProgressEvent {
            path: path.to_string_lossy().to_string(),
            stage: stage.to_string(),
            completed: done,
            total,
        },
    );
}

pub fn compress_video_at_path(
    app: &AppHandle,
    path: &Path,
    mode: CompressionMode,
) -> Result<CompressResult, String> {
    let original_size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    let started = Instant::now();
    let output = output_path_for(path);

    let ffmpeg = find_ffmpeg(app).ok_or_else(|| {
        ffmpeg_missing_message()
    })?;

    emit(app, path, "FFmpeg 压缩中", 1, 3);
    compress_with_ffmpeg(&ffmpeg, path, &output, mode)?;

    let mut compressed_size = fs::metadata(&output).map_err(|e| e.to_string())?.len();
    if compressed_size >= original_size {
        fs::copy(path, &output).map_err(|e| e.to_string())?;
        compressed_size = original_size;
    }

    emit(app, path, "完成", 3, 3);

    Ok(CompressResult {
        input_path: path.to_string_lossy().to_string(),
        output_path: output.to_string_lossy().to_string(),
        original_size,
        compressed_size,
        saved_bytes: original_size as i64 - compressed_size as i64,
        compression_ratio: if original_size == 0 {
            0.0
        } else {
            (1.0 - compressed_size as f64 / original_size as f64) * 100.0
        },
        mode: mode.label().to_string(),
        duration_ms: started.elapsed().as_millis(),
    })
}

fn find_ffmpeg(app: &AppHandle) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let exe_name = "ffmpeg.exe";
    #[cfg(target_os = "android")]
    let exe_name = "ffmpeg";
    #[cfg(not(any(target_os = "windows", target_os = "android")))]
    let exe_name = "ffmpeg";

    if let Ok(bundled) = app
        .path()
        .resolve(format!("resources/ffmpeg/{exe_name}"), BaseDirectory::Resource)
    {
        if bundled.exists() {
            return Some(bundled);
        }
    }

    #[cfg(target_os = "android")]
    {
        let data_dir = app.path().app_local_data_dir().ok()?;
        let ffmpeg_in_data = data_dir.join("ffmpeg");
        if ffmpeg_in_data.exists() {
            return Some(ffmpeg_in_data);
        }
    }

    for candidate in ffmpeg_path_candidates() {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let mut probe = Command::new("ffmpeg");
    probe.arg("-version");
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        probe.creation_flags(CREATE_NO_WINDOW);
    }
    if probe.output().is_ok() {
        return Some(PathBuf::from("ffmpeg"));
    }

    None
}

fn ffmpeg_path_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/opt/homebrew/bin/ffmpeg"),
        PathBuf::from("/usr/local/bin/ffmpeg"),
        PathBuf::from("/usr/bin/ffmpeg"),
    ]
}

fn ffmpeg_missing_message() -> String {
    #[cfg(target_os = "macos")]
    {
        return "未找到 FFmpeg。Mac 可通过 Homebrew 安装：brew install ffmpeg，或将 ffmpeg 放入 src-tauri/resources/ffmpeg/ 后重新打包".to_string();
    }

    #[cfg(target_os = "windows")]
    {
        return "未找到 FFmpeg。请将 ffmpeg.exe 放入 src-tauri/resources/ffmpeg/ 目录后重新打包".to_string();
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        "未找到 FFmpeg。请安装 ffmpeg，或将可执行文件放入 src-tauri/resources/ffmpeg/ 后重新打包".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffmpeg_path_candidates_include_homebrew_locations_on_macos() {
        let candidates = ffmpeg_path_candidates();

        assert!(candidates.contains(&PathBuf::from("/opt/homebrew/bin/ffmpeg")));
        assert!(candidates.contains(&PathBuf::from("/usr/local/bin/ffmpeg")));
    }
}

fn compress_with_ffmpeg(
    ffmpeg_path: &Path,
    input: &Path,
    output: &Path,
    mode: CompressionMode,
) -> Result<(), String> {
    if output.exists() {
        let _ = fs::remove_file(output);
    }

    let mut cmd = Command::new(ffmpeg_path);

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.arg("-i").arg(input);
    cmd.arg("-y");

    match mode {
        CompressionMode::Lossless => {
            cmd.arg("-c:v").arg("copy").arg("-c:a").arg("copy");
        }
        CompressionMode::Light => {
            cmd.arg("-c:v")
                .arg("libx264")
                .arg("-crf")
                .arg("23")
                .arg("-preset")
                .arg("medium")
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg("128k");
        }
        CompressionMode::Standard => {
            cmd.arg("-c:v")
                .arg("libx264")
                .arg("-crf")
                .arg("28")
                .arg("-preset")
                .arg("medium")
                .arg("-vf")
                .arg("scale='min(1920,iw)':'min(1080,ih)':force_original_aspect_ratio=decrease")
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg("96k");
        }
        CompressionMode::Extreme => {
            cmd.arg("-c:v")
                .arg("libx264")
                .arg("-crf")
                .arg("32")
                .arg("-preset")
                .arg("slow")
                .arg("-vf")
                .arg("scale='min(1280,iw)':'min(720,ih)':force_original_aspect_ratio=decrease")
                .arg("-c:a")
                .arg("aac")
                .arg("-b:a")
                .arg("64k");
        }
    }

    cmd.arg(output);

    let result = cmd
        .output()
        .map_err(|e| format!("FFmpeg 启动失败: {e}"))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!("FFmpeg 错误: {stderr}"));
    }

    if !output.exists() {
        return Err("FFmpeg 未生成输出文件".to_string());
    }

    Ok(())
}

fn output_path_for(p: &Path) -> PathBuf {
    let parent = p.parent().unwrap_or_else(|| Path::new("."));
    let stem = p.file_stem().and_then(|v| v.to_str()).unwrap_or("video");
    parent.join(format!("{stem}_compressed.mp4"))
}
