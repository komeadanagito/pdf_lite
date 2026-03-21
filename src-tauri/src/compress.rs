use std::{
  fs,
  io::Cursor,
  path::{Path, PathBuf},
  time::Instant,
};

use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage};
use lopdf::{Document, Object, ObjectId, Stream};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::{
  atomic::{AtomicUsize, Ordering},
  Arc,
};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CompressionMode {
  Lossless = 0,
  Light = 1,
  Standard = 2,
  Extreme = 3,
}

impl CompressionMode {
  pub fn from_u8(value: u8) -> Result<Self, String> {
    match value {
      0 => Ok(Self::Lossless),
      1 => Ok(Self::Light),
      2 => Ok(Self::Standard),
      3 => Ok(Self::Extreme),
      _ => Err(format!("invalid compression mode: {}", value)),
    }
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::Lossless => "lossless",
      Self::Light => "light",
      Self::Standard => "standard",
      Self::Extreme => "extreme",
    }
  }
}

#[derive(Debug, Clone, Serialize)]
pub struct CompressResult {
  pub input_path: String,
  pub output_path: String,
  pub original_size: u64,
  pub compressed_size: u64,
  pub saved_bytes: i64,
  pub compression_ratio: f64,
  pub mode: String,
  pub duration_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct PdfInfo {
  pub path: String,
  pub file_name: String,
  pub pages: usize,
  pub size_bytes: u64,
  pub title: Option<String>,
  pub author: Option<String>,
  pub creator: Option<String>,
  pub producer: Option<String>,
  pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ProgressEvent {
  path: String,
  stage: String,
  completed: usize,
  total: usize,
}

pub fn get_pdf_info_for_path(path: &Path) -> Result<PdfInfo, String> {
  let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
  let doc = Document::load(path).map_err(|e| e.to_string())?;
  let pages = doc.get_pages().len();
  let file_name = path
    .file_name()
    .and_then(|n| n.to_str())
    .unwrap_or("document.pdf")
    .to_string();
  let info = info_dictionary(&doc);

  Ok(PdfInfo {
    path: path.to_string_lossy().to_string(),
    file_name,
    pages,
    size_bytes: metadata.len(),
    title: info.as_ref().and_then(|d| dict_string(d, b"Title")),
    author: info.as_ref().and_then(|d| dict_string(d, b"Author")),
    creator: info.as_ref().and_then(|d| dict_string(d, b"Creator")),
    producer: info.as_ref().and_then(|d| dict_string(d, b"Producer")),
    version: Some(doc.version),
  })
}

// ─── Main compress entry ───────────────────────────────────────────

pub fn compress_pdf_at_path(
  app: &AppHandle,
  path: &Path,
  mode: CompressionMode,
) -> Result<CompressResult, String> {
  let original_size = fs::metadata(path).map_err(|e| e.to_string())?.len();
  let started = Instant::now();
  emit(app, path, "加载文档", 0, 4);

  let mut doc = Document::load(path).map_err(|e| e.to_string())?;
  doc.version = "1.5".to_string();

  emit(app, path, "结构优化", 1, 4);
  let _ = doc.compress();

  match mode {
    CompressionMode::Lossless => {}
    CompressionMode::Light => {
      recompress_images(app, path, &mut doc, 80, Some(1800))?;
    }
    CompressionMode::Standard => {
      recompress_images(app, path, &mut doc, 65, Some(1500))?;
      emit(app, path, "清理冗余", 3, 4);
      strip_interactive(&mut doc, true);
    }
    CompressionMode::Extreme => {
      recompress_images(app, path, &mut doc, 45, Some(1200))?;
      emit(app, path, "清理冗余", 3, 4);
      strip_interactive(&mut doc, true);
      strip_metadata(&mut doc);
    }
  }

  let output_path = output_path_for(path);
  if output_path.exists() {
    let _ = fs::remove_file(&output_path);
  }
  doc.save(&output_path).map_err(|e| e.to_string())?;

  let mut compressed_size = fs::metadata(&output_path).map_err(|e| e.to_string())?.len();
  if compressed_size >= original_size {
    fs::copy(path, &output_path).map_err(|e| e.to_string())?;
    compressed_size = original_size;
  }

  emit(app, path, "完成", 4, 4);

  Ok(CompressResult {
    input_path: path.to_string_lossy().to_string(),
    output_path: output_path.to_string_lossy().to_string(),
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

// ─── Image recompression ───────────────────────────────────────────

struct ImageTask {
  id: ObjectId,
  jpeg_bytes: Vec<u8>,
  original_len: usize,
  width: u32,
  height: u32,
}

fn recompress_images(
  app: &AppHandle,
  path: &Path,
  doc: &mut Document,
  quality: u8,
  max_dim: Option<u32>,
) -> Result<(), String> {
  // Phase 1: Find image streams and decode them using lopdf's built-in decompression
  let image_ids: Vec<ObjectId> = doc
    .objects
    .iter()
    .filter_map(|(id, obj)| match obj {
      Object::Stream(s) if is_image_stream(s) => Some(*id),
      _ => None,
    })
    .collect();

  // Decompress all streams in-place first (lopdf handles zlib, predictors, etc.)
  for &id in &image_ids {
    if let Some(Object::Stream(stream)) = doc.objects.get_mut(&id) {
      let _ = stream.decompress();
    }
  }

  // Build tasks: extract decoded bytes + metadata
  let mut tasks: Vec<ImageTask> = Vec::new();
  for &id in &image_ids {
    let stream = match doc.objects.get(&id) {
      Some(Object::Stream(s)) => s,
      _ => continue,
    };

    let w = dict_int(&stream.dict, b"Width").unwrap_or(0) as u32;
    let h = dict_int(&stream.dict, b"Height").unwrap_or(0) as u32;
    if w == 0 || h == 0 {
      continue;
    }

    let bpc = dict_int(&stream.dict, b"BitsPerComponent").unwrap_or(8);
    if bpc != 8 {
      continue;
    }

    // After decompress(), content is raw pixel data.
    // Determine colour components from data size.
    let pixel_count = (w as usize) * (h as usize);
    let data_len = stream.content.len();
    let components = if data_len == pixel_count {
      1 // Grayscale
    } else if data_len == pixel_count * 3 {
      3 // RGB
    } else if data_len == pixel_count * 4 {
      4 // CMYK
    } else {
      continue; // Unknown layout, skip
    };

    // Build raw image from pixels
    let img: DynamicImage = match components {
      1 => match image::GrayImage::from_raw(w, h, stream.content.clone()) {
        Some(g) => DynamicImage::ImageLuma8(g),
        None => continue,
      },
      3 => match image::RgbImage::from_raw(w, h, stream.content.clone()) {
        Some(rgb) => DynamicImage::ImageRgb8(rgb),
        None => continue,
      },
      4 => {
        let rgb_data = cmyk_to_rgb(&stream.content);
        match image::RgbImage::from_raw(w, h, rgb_data) {
          Some(rgb) => DynamicImage::ImageRgb8(rgb),
          None => continue,
        }
      }
      _ => continue,
    };

    // Encode to JPEG to get baseline bytes for comparison
    let mut buf = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut buf, quality)
      .encode_image(&img)
      .ok();
    let jpeg_bytes = buf.into_inner();
    if jpeg_bytes.is_empty() {
      continue;
    }

    tasks.push(ImageTask {
      id,
      jpeg_bytes,
      original_len: data_len,
      width: w,
      height: h,
    });
  }

  let total = tasks.len();
  if total == 0 {
    return Ok(());
  }

  emit(app, path, &format!("处理 {total} 张图片"), 2, 4);

  // Phase 2: Parallel resize + re-encode
  let done = Arc::new(AtomicUsize::new(0));
  let app2 = app.clone();
  let path_str = path.to_string_lossy().to_string();

  struct CompressedImage {
    id: ObjectId,
    data: Vec<u8>,
    new_w: u32,
    new_h: u32,
  }

  let results: Vec<CompressedImage> = tasks
    .into_par_iter()
    .filter_map(|task| {
      // Decode the baseline JPEG we just encoded
      let img = image::load_from_memory(&task.jpeg_bytes).ok()?;

      // Resize if needed
      let img = if let Some(limit) = max_dim {
        if task.width > limit || task.height > limit {
          img.resize(limit, limit, FilterType::Lanczos3)
        } else {
          img
        }
      } else {
        img
      };

      let new_w = img.width();
      let new_h = img.height();

      // Re-encode at target quality (if resized, this will be smaller)
      let mut out = Cursor::new(Vec::new());
      JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&img)
        .ok()?;
      let data = out.into_inner();

      // Only keep if smaller than original stream
      if data.len() >= task.original_len {
        // Track progress even for skipped images
        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
        if n % 5 == 0 || n == total {
          let _ = app2.emit(
            "compress-progress",
            ProgressEvent {
              path: path_str.clone(),
              stage: format!("图片 {n}/{total}"),
              completed: 2,
              total: 4,
            },
          );
        }
        return None;
      }

      let n = done.fetch_add(1, Ordering::Relaxed) + 1;
      if n % 5 == 0 || n == total {
        let _ = app2.emit(
          "compress-progress",
          ProgressEvent {
            path: path_str.clone(),
            stage: format!("图片 {n}/{total}"),
            completed: 2,
            total: 4,
          },
        );
      }

      Some(CompressedImage {
        id: task.id,
        data,
        new_w,
        new_h,
      })
    })
    .collect();

  // Phase 3: Write results back
  for result in results {
    if let Some(Object::Stream(stream)) = doc.objects.get_mut(&result.id) {
      stream.content = result.data;
      stream.dict.remove(b"Filter");
      stream.dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
      stream.dict.set("Length", Object::Integer(stream.content.len() as i64));
      stream.dict.remove(b"DecodeParms");
      // Update dimensions if resized
      stream.dict.set("Width", Object::Integer(result.new_w as i64));
      stream.dict.set("Height", Object::Integer(result.new_h as i64));
      // Remove colour space — DCTDecode JPEG carries its own
      stream.dict.remove(b"ColorSpace");
      stream.dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
      stream.dict.set("BitsPerComponent", Object::Integer(8));
    }
  }

  Ok(())
}

// ─── Cleanup helpers ───────────────────────────────────────────────

fn strip_interactive(doc: &mut Document, remove_annots: bool) {
  doc.trailer.remove(b"Info");
  if let Some(root_id) = catalog_id(doc) {
    if let Some(Object::Dictionary(root)) = doc.objects.get_mut(&root_id) {
      root.remove(b"Outlines");
      root.remove(b"AcroForm");
      root.remove(b"Names");
    }
  }
  if remove_annots {
    let page_ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    for pid in page_ids {
      if let Some(Object::Dictionary(page)) = doc.objects.get_mut(&pid) {
        page.remove(b"Annots");
      }
    }
  }
}

fn strip_metadata(doc: &mut Document) {
  if let Some(root_id) = catalog_id(doc) {
    if let Some(Object::Dictionary(root)) = doc.objects.get_mut(&root_id) {
      root.remove(b"Metadata");
      root.remove(b"Lang");
      root.remove(b"PageLabels");
      root.remove(b"StructTreeRoot");
      root.remove(b"MarkInfo");
    }
  }
}

// ─── Utilities ─────────────────────────────────────────────────────

fn is_image_stream(stream: &Stream) -> bool {
  matches!(
    stream.dict.get(b"Subtype"),
    Ok(Object::Name(ref name)) if name.as_slice() == b"Image"
  )
}

fn cmyk_to_rgb(cmyk: &[u8]) -> Vec<u8> {
  let mut rgb = Vec::with_capacity(cmyk.len() / 4 * 3);
  for chunk in cmyk.chunks_exact(4) {
    let (c, m, y, k) = (
      chunk[0] as f32 / 255.0,
      chunk[1] as f32 / 255.0,
      chunk[2] as f32 / 255.0,
      chunk[3] as f32 / 255.0,
    );
    rgb.push(((1.0 - c) * (1.0 - k) * 255.0) as u8);
    rgb.push(((1.0 - m) * (1.0 - k) * 255.0) as u8);
    rgb.push(((1.0 - y) * (1.0 - k) * 255.0) as u8);
  }
  rgb
}

fn output_path_for(path: &Path) -> PathBuf {
  let parent = path.parent().unwrap_or_else(|| Path::new("."));
  let stem = path
    .file_stem()
    .and_then(|v| v.to_str())
    .unwrap_or("document");
  parent.join(format!("{stem}_compressed.pdf"))
}

fn emit(app: &AppHandle, path: &Path, stage: &str, completed: usize, total: usize) {
  let _ = app.emit(
    "compress-progress",
    ProgressEvent {
      path: path.to_string_lossy().to_string(),
      stage: stage.to_string(),
      completed,
      total,
    },
  );
}

fn catalog_id(doc: &Document) -> Option<ObjectId> {
  match doc.trailer.get(b"Root") {
    Ok(Object::Reference(id)) => Some(*id),
    _ => None,
  }
}

fn info_dictionary<'a>(doc: &'a Document) -> Option<&'a lopdf::Dictionary> {
  let id = match doc.trailer.get(b"Info") {
    Ok(Object::Reference(id)) => *id,
    _ => return None,
  };
  match doc.objects.get(&id) {
    Some(Object::Dictionary(d)) => Some(d),
    Some(Object::Stream(s)) => Some(&s.dict),
    _ => None,
  }
}

fn dict_string(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
  match dict.get(key).ok()? {
    Object::String(b, _) => Some(String::from_utf8_lossy(b).to_string()),
    Object::Name(b) => Some(String::from_utf8_lossy(b).to_string()),
    _ => None,
  }
}

fn dict_int(dict: &lopdf::Dictionary, key: &[u8]) -> Option<i64> {
  match dict.get(key).ok()? {
    Object::Integer(i) => Some(*i),
    _ => None,
  }
}
