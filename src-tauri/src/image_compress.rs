use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    time::Instant,
};

use image::{
    codecs::jpeg::JpegEncoder,
    DynamicImage,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

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

pub fn compress_image_at_path(
    app: &AppHandle,
    path: &Path,
    mode: CompressionMode,
) -> Result<CompressResult, String> {
    let original_size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    let started = Instant::now();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    emit(app, path, "读取图片", 0, 3);

    let output = output_path_for(path, &ext);

    match ext.as_str() {
        "svg" => compress_svg(path, &output, mode)?,
        "png" => compress_png(app, path, &output, mode)?,
        "gif" => compress_gif(path, &output, mode)?,
        _ => compress_raster(app, path, &output, &ext, mode)?,
    }

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

fn compress_raster(
    app: &AppHandle,
    path: &Path,
    output: &Path,
    ext: &str,
    mode: CompressionMode,
) -> Result<(), String> {
    let img = image::open(path).map_err(|e| format!("无法加载图片: {e}"))?;

    emit(app, path, "压缩中", 1, 3);

    let (quality, max_dim) = match mode {
        CompressionMode::Lossless => (95, None),
        CompressionMode::Light => (85, None),
        CompressionMode::Standard => (70, Some(2048u32)),
        CompressionMode::Extreme => (50, Some(1440u32)),
    };

    let img = match max_dim {
        Some(limit) if img.width() > limit || img.height() > limit => {
            img.resize(limit, limit, image::imageops::FilterType::Lanczos3)
        }
        _ => img,
    };

    emit(app, path, "写入文件", 2, 3);

    match ext {
        "jpg" | "jpeg" => write_jpeg(&img, output, quality)?,
        "webp" => write_webp(&img, output, quality)?,
        "bmp" | "tiff" | "tif" => {
            write_jpeg(&img, output, quality)?;
        }
        _ => write_jpeg(&img, output, quality)?,
    }

    Ok(())
}

fn write_jpeg(img: &DynamicImage, output: &Path, quality: u8) -> Result<(), String> {
    let rgb = img.to_rgb8();
    let mut buf = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut buf, quality)
        .encode_image(&DynamicImage::ImageRgb8(rgb))
        .map_err(|e| format!("JPEG 编码失败: {e}"))?;
    fs::write(output, buf.into_inner()).map_err(|e| e.to_string())
}

fn write_webp(img: &DynamicImage, output: &Path, quality: u8) -> Result<(), String> {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let encoder = webp::Encoder::from_rgba(&rgba, w, h);
    let mem = if quality >= 95 {
        encoder.encode_lossless()
    } else {
        encoder.encode(quality as f32)
    };
    fs::write(output, &*mem).map_err(|e| e.to_string())
}

fn compress_png(
    app: &AppHandle,
    path: &Path,
    output: &Path,
    mode: CompressionMode,
) -> Result<(), String> {
    emit(app, path, "PNG 优化中", 1, 3);

    let data = fs::read(path).map_err(|e| e.to_string())?;

    match mode {
        CompressionMode::Lossless | CompressionMode::Light => {
            let mut opts = oxipng::Options::from_preset(match mode {
                CompressionMode::Lossless => 2,
                _ => 4,
            });
            opts.strip = oxipng::StripChunks::Safe;

            let optimized =
                oxipng::optimize_from_memory(&data, &opts).map_err(|e| format!("oxipng 失败: {e}"))?;

            emit(app, path, "写入文件", 2, 3);
            fs::write(output, optimized).map_err(|e| e.to_string())?;
        }
        CompressionMode::Standard | CompressionMode::Extreme => {
            let img = image::load_from_memory(&data).map_err(|e| format!("PNG 加载失败: {e}"))?;
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();

            let max_colors = match mode {
                CompressionMode::Standard => 256,
                _ => 128,
            };

            let mut liq = imagequant::new();
            liq.set_max_colors(max_colors).map_err(|e| format!("imagequant 设置失败: {e}"))?;
            liq.set_quality(0, match mode {
                CompressionMode::Standard => 80,
                _ => 60,
            }).map_err(|e| format!("imagequant 质量设置失败: {e}"))?;

            let pixels: Vec<rgb::RGBA8> = rgba
                .pixels()
                .map(|p| rgb::RGBA8 { r: p[0], g: p[1], b: p[2], a: p[3] })
                .collect();

            let mut img_liq = liq
                .new_image(pixels, w as usize, h as usize, 0.0)
                .map_err(|e| format!("imagequant 图像创建失败: {e}"))?;

            let mut res = liq
                .quantize(&mut img_liq)
                .map_err(|e| format!("imagequant 量化失败: {e}"))?;

            res.set_dithering_level(1.0).map_err(|e| format!("抖动设置失败: {e}"))?;

            let (palette, pixels_indexed) = res
                .remapped(&mut img_liq)
                .map_err(|e| format!("imagequant 映射失败: {e}"))?;

            emit(app, path, "写入文件", 2, 3);

            let file = fs::File::create(output).map_err(|e| e.to_string())?;
            let mut encoder = png::Encoder::new(file, w, h);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_compression(png::Compression::High);

            let png_palette: Vec<u8> = palette.iter().flat_map(|c| [c.r, c.g, c.b]).collect();
            let png_trns: Vec<u8> = palette.iter().map(|c| c.a).collect();
            encoder.set_palette(png_palette);
            encoder.set_trns(png_trns);

            let mut writer = encoder
                .write_header()
                .map_err(|e| format!("PNG 头写入失败: {e}"))?;
            writer
                .write_image_data(&pixels_indexed)
                .map_err(|e| format!("PNG 数据写入失败: {e}"))?;
        }
    }
    Ok(())
}

fn compress_svg(path: &Path, output: &Path, _mode: CompressionMode) -> Result<(), String> {
    let svg_data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let opt = resvg::usvg::Options::default();
    let tree =
        resvg::usvg::Tree::from_str(&svg_data, &opt).map_err(|e| format!("SVG 解析失败: {e}"))?;

    let cleaned = tree.to_string(&resvg::usvg::WriteOptions::default());

    let minified = cleaned
        .lines()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("");

    fs::write(output, minified).map_err(|e| e.to_string())
}

fn compress_gif(path: &Path, output: &Path, mode: CompressionMode) -> Result<(), String> {
    let img = image::open(path).map_err(|e| format!("GIF 加载失败: {e}"))?;

    let (max_dim, quality) = match mode {
        CompressionMode::Lossless => (None, 95u8),
        CompressionMode::Light => (None, 85),
        CompressionMode::Standard => (Some(1024u32), 70),
        CompressionMode::Extreme => (Some(720u32), 50),
    };

    let img = match max_dim {
        Some(l) if img.width() > l || img.height() > l => {
            img.resize(l, l, image::imageops::FilterType::Lanczos3)
        }
        _ => img,
    };

    write_webp(&img, output, quality)
}

fn output_path_for(p: &Path, ext: &str) -> PathBuf {
    let parent = p.parent().unwrap_or_else(|| Path::new("."));
    let stem = p.file_stem().and_then(|v| v.to_str()).unwrap_or("image");
    let out_ext = match ext {
        "svg" => "svg",
        "png" => "png",
        "webp" => "webp",
        "gif" => "webp",
        "bmp" | "tiff" | "tif" => "jpg",
        _ => "jpg",
    };
    parent.join(format!("{stem}_compressed.{out_ext}"))
}
