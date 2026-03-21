use std::{
  fs,
  io::{Cursor, Read as _},
  path::{Path, PathBuf},
  time::Instant,
};

use flate2::read::ZlibDecoder;
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
  pub fn from_u8(v: u8) -> Result<Self, String> {
    match v {
      0 => Ok(Self::Lossless),
      1 => Ok(Self::Light),
      2 => Ok(Self::Standard),
      3 => Ok(Self::Extreme),
      _ => Err(format!("invalid compression mode: {v}")),
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
  let meta = fs::metadata(path).map_err(|e| e.to_string())?;
  let doc = Document::load(path).map_err(|e| e.to_string())?;
  let pages = doc.get_pages().len();
  let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("document.pdf").to_string();
  let info = info_dict(&doc);
  Ok(PdfInfo {
    path: path.to_string_lossy().to_string(),
    file_name: fname,
    pages,
    size_bytes: meta.len(),
    title: info.as_ref().and_then(|d| dstr(d, b"Title")),
    author: info.as_ref().and_then(|d| dstr(d, b"Author")),
    creator: info.as_ref().and_then(|d| dstr(d, b"Creator")),
    producer: info.as_ref().and_then(|d| dstr(d, b"Producer")),
    version: Some(doc.version),
  })
}

// ─── Main entry ────────────────────────────────────────────────────

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

  let output = output_path_for(path);
  if output.exists() {
    let _ = fs::remove_file(&output);
  }
  doc.save(&output).map_err(|e| e.to_string())?;

  let mut compressed_size = fs::metadata(&output).map_err(|e| e.to_string())?.len();
  if compressed_size >= original_size {
    fs::copy(path, &output).map_err(|e| e.to_string())?;
    compressed_size = original_size;
  }

  emit(app, path, "完成", 4, 4);

  Ok(CompressResult {
    input_path: path.to_string_lossy().to_string(),
    output_path: output.to_string_lossy().to_string(),
    original_size,
    compressed_size,
    saved_bytes: original_size as i64 - compressed_size as i64,
    compression_ratio: if original_size == 0 { 0.0 } else {
      (1.0 - compressed_size as f64 / original_size as f64) * 100.0
    },
    mode: mode.label().to_string(),
    duration_ms: started.elapsed().as_millis(),
  })
}

// ─── Image recompression ───────────────────────────────────────────

struct ImgTask {
  id: ObjectId,
  /// Raw pixel data (decoded from whatever filter the stream used)
  pixels: Vec<u8>,
  w: u32,
  h: u32,
  components: usize,
  /// Original compressed stream length (for size comparison)
  original_stream_len: usize,
}

fn recompress_images(
  app: &AppHandle,
  path: &Path,
  doc: &mut Document,
  quality: u8,
  max_dim: Option<u32>,
) -> Result<(), String> {
  let image_ids: Vec<ObjectId> = doc
    .objects
    .iter()
    .filter_map(|(id, obj)| match obj {
      Object::Stream(s) if is_image(s) => Some(*id),
      _ => None,
    })
    .collect();

  let mut tasks: Vec<ImgTask> = Vec::new();

  for &id in &image_ids {
    let stream = match doc.objects.get(&id) {
      Some(Object::Stream(s)) => s,
      _ => continue,
    };

    let w = dint(&stream.dict, b"Width").unwrap_or(0) as u32;
    let h = dint(&stream.dict, b"Height").unwrap_or(0) as u32;
    if w < 16 || h < 16 {
      continue;
    }

    let original_stream_len = stream.content.len();

    // Determine the filter chain
    let filter = stream.dict.get(b"Filter").ok();
    let filter_name: Option<&[u8]> = match filter {
      Some(Object::Name(n)) => Some(n.as_slice()),
      _ => None,
    };

    match filter_name {
      Some(b"DCTDecode") => {
        // Stream content is raw JPEG bytes — decode directly
        let img = match image::load_from_memory(&stream.content) {
          Ok(img) => img,
          Err(_) => continue,
        };
        let (components, pixels) = img_to_rgb_pixels(&img);
        tasks.push(ImgTask { id, pixels, w: img.width(), h: img.height(), components, original_stream_len });
      }
      Some(b"FlateDecode") => {
        // Stream content is zlib-compressed raw pixels
        let mut raw = Vec::new();
        if ZlibDecoder::new(stream.content.as_slice()).read_to_end(&mut raw).is_err() || raw.is_empty() {
          continue;
        }

        let bpc = dint(&stream.dict, b"BitsPerComponent").unwrap_or(8);
        if bpc != 8 {
          continue;
        }

        // Detect if PNG prediction filters were applied (predictor 10-15)
        let dp = stream.dict.get(b"DecodeParms").ok();
        let predictor = match dp {
          Some(Object::Dictionary(d)) => dint(d, b"Predictor").unwrap_or(1),
          _ => 1,
        };

        // Get component count from DecodeParms.Colors, or infer from data size
        let dp_colors = match dp {
          Some(Object::Dictionary(d)) => dint(d, b"Colors").map(|c| c as usize),
          _ => None,
        };

        let pixel_count = (w as usize) * (h as usize);
        let components = if let Some(c) = dp_colors {
          c
        } else {
          // Infer from raw data length
          let len = raw.len();
          if predictor >= 10 {
            // With PNG predictor: each row has 1 extra byte
            let rows = h as usize;
            let row_payload = if rows > 0 { (len / rows).saturating_sub(1) } else { 0 };
            let per_pixel = if w > 0 { row_payload / (w as usize) } else { 0 };
            if per_pixel >= 1 && per_pixel <= 4 { per_pixel } else { continue; }
          } else if len == pixel_count { 1 }
          else if len == pixel_count * 3 { 3 }
          else if len == pixel_count * 4 { 4 }
          else { continue; }
        };

        // Undo PNG prediction if needed
        let pixels = if predictor >= 10 {
          match undo_png_predict(&raw, w as usize, h as usize, components) {
            Some(p) => p,
            None => continue,
          }
        } else {
          raw
        };

        // Validate data size
        if pixels.len() != pixel_count * components {
          continue;
        }

        tasks.push(ImgTask { id, pixels, w, h, components, original_stream_len });
      }
      _ => continue,
    }
  }

  let total = tasks.len();
  if total == 0 {
    emit(app, path, "没有可压缩的图片", 2, 4);
    return Ok(());
  }

  emit(app, path, &format!("压缩 {total} 张图片"), 2, 4);

  let done = Arc::new(AtomicUsize::new(0));
  let app2 = app.clone();
  let path_s = path.to_string_lossy().to_string();

  struct Result {
    id: ObjectId,
    jpeg: Vec<u8>,
    new_w: u32,
    new_h: u32,
  }

  let results: Vec<Result> = tasks
    .into_par_iter()
    .filter_map(|task| {
      // Build DynamicImage from raw pixels
      let img: DynamicImage = match task.components {
        1 => {
          let g = image::GrayImage::from_raw(task.w, task.h, task.pixels)?;
          DynamicImage::ImageLuma8(g).into_rgb8().into()
        }
        3 => {
          let rgb = image::RgbImage::from_raw(task.w, task.h, task.pixels)?;
          DynamicImage::ImageRgb8(rgb)
        }
        4 => {
          let rgb_data = cmyk_to_rgb(&task.pixels);
          let rgb = image::RgbImage::from_raw(task.w, task.h, rgb_data)?;
          DynamicImage::ImageRgb8(rgb)
        }
        _ => return None,
      };

      // Resize if needed
      let img = match max_dim {
        Some(limit) if img.width() > limit || img.height() > limit => {
          img.resize(limit, limit, FilterType::Lanczos3)
        }
        _ => img,
      };

      let new_w = img.width();
      let new_h = img.height();

      // Encode to JPEG
      let mut buf = Cursor::new(Vec::new());
      JpegEncoder::new_with_quality(&mut buf, quality).encode_image(&img).ok()?;
      let jpeg = buf.into_inner();

      // Progress
      let n = done.fetch_add(1, Ordering::Relaxed) + 1;
      if n % 5 == 0 || n == total {
        let _ = app2.emit("compress-progress", ProgressEvent {
          path: path_s.clone(),
          stage: format!("图片 {n}/{total}"),
          completed: 2, total: 4,
        });
      }

      // Only keep if actually smaller
      if jpeg.len() >= task.original_stream_len {
        return None;
      }

      Some(Result { id: task.id, jpeg, new_w, new_h })
    })
    .collect();

  // Phase 3: Write back
  for r in results {
    if let Some(Object::Stream(stream)) = doc.objects.get_mut(&r.id) {
      stream.content = r.jpeg;
      stream.dict.remove(b"Filter");
      stream.dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
      stream.dict.set("Length", Object::Integer(stream.content.len() as i64));
      stream.dict.remove(b"DecodeParms");
      stream.dict.set("Width", Object::Integer(r.new_w as i64));
      stream.dict.set("Height", Object::Integer(r.new_h as i64));
      stream.dict.remove(b"ColorSpace");
      stream.dict.set("ColorSpace", Object::Name(b"DeviceRGB".to_vec()));
      stream.dict.set("BitsPerComponent", Object::Integer(8));
    }
  }

  Ok(())
}

fn img_to_rgb_pixels(img: &DynamicImage) -> (usize, Vec<u8>) {
  let rgb = img.to_rgb8();
  (3, rgb.into_raw())
}

// ─── PNG prediction reversal ───────────────────────────────────────

fn undo_png_predict(data: &[u8], w: usize, h: usize, components: usize) -> Option<Vec<u8>> {
  let stride = w * components;
  let row_bytes = stride + 1;
  if data.len() < row_bytes * h {
    return None;
  }

  let mut out = Vec::with_capacity(stride * h);
  let mut prev = vec![0u8; stride];

  for row in 0..h {
    let off = row * row_bytes;
    let ft = data[off];
    let src = &data[off + 1..off + row_bytes];
    let mut cur = src.to_vec();

    match ft {
      0 => {}
      1 => { for i in components..stride { cur[i] = cur[i].wrapping_add(cur[i - components]); } }
      2 => { for i in 0..stride { cur[i] = cur[i].wrapping_add(prev[i]); } }
      3 => {
        for i in 0..stride {
          let a = if i >= components { cur[i - components] as u16 } else { 0 };
          let b = prev[i] as u16;
          cur[i] = cur[i].wrapping_add(((a + b) / 2) as u8);
        }
      }
      4 => {
        for i in 0..stride {
          let a = if i >= components { cur[i - components] } else { 0 };
          let b = prev[i];
          let c = if i >= components { prev[i - components] } else { 0 };
          cur[i] = cur[i].wrapping_add(paeth(a, b, c));
        }
      }
      _ => return None,
    }

    out.extend_from_slice(&cur);
    prev = cur;
  }
  Some(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
  let (a, b, c) = (a as i32, b as i32, c as i32);
  let p = a + b - c;
  let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
  if pa <= pb && pa <= pc { a as u8 } else if pb <= pc { b as u8 } else { c as u8 }
}

fn cmyk_to_rgb(cmyk: &[u8]) -> Vec<u8> {
  let mut rgb = Vec::with_capacity(cmyk.len() / 4 * 3);
  for ch in cmyk.chunks_exact(4) {
    let (c, m, y, k) = (ch[0] as f32 / 255.0, ch[1] as f32 / 255.0, ch[2] as f32 / 255.0, ch[3] as f32 / 255.0);
    rgb.push(((1.0 - c) * (1.0 - k) * 255.0) as u8);
    rgb.push(((1.0 - m) * (1.0 - k) * 255.0) as u8);
    rgb.push(((1.0 - y) * (1.0 - k) * 255.0) as u8);
  }
  rgb
}

// ─── Cleanup ───────────────────────────────────────────────────────

fn strip_interactive(doc: &mut Document, annots: bool) {
  doc.trailer.remove(b"Info");
  if let Some(rid) = cat_id(doc) {
    if let Some(Object::Dictionary(r)) = doc.objects.get_mut(&rid) {
      r.remove(b"Outlines");
      r.remove(b"AcroForm");
      r.remove(b"Names");
    }
  }
  if annots {
    let pids: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    for pid in pids {
      if let Some(Object::Dictionary(p)) = doc.objects.get_mut(&pid) {
        p.remove(b"Annots");
      }
    }
  }
}

fn strip_metadata(doc: &mut Document) {
  if let Some(rid) = cat_id(doc) {
    if let Some(Object::Dictionary(r)) = doc.objects.get_mut(&rid) {
      r.remove(b"Metadata");
      r.remove(b"Lang");
      r.remove(b"PageLabels");
      r.remove(b"StructTreeRoot");
      r.remove(b"MarkInfo");
    }
  }
}

// ─── Helpers ───────────────────────────────────────────────────────

fn is_image(s: &Stream) -> bool {
  matches!(s.dict.get(b"Subtype"), Ok(Object::Name(ref n)) if n.as_slice() == b"Image")
}

fn output_path_for(p: &Path) -> PathBuf {
  let parent = p.parent().unwrap_or_else(|| Path::new("."));
  let stem = p.file_stem().and_then(|v| v.to_str()).unwrap_or("doc");
  parent.join(format!("{stem}_compressed.pdf"))
}

fn emit(app: &AppHandle, path: &Path, stage: &str, done: usize, total: usize) {
  let _ = app.emit("compress-progress", ProgressEvent {
    path: path.to_string_lossy().to_string(),
    stage: stage.to_string(),
    completed: done,
    total,
  });
}

fn cat_id(doc: &Document) -> Option<ObjectId> {
  match doc.trailer.get(b"Root") {
    Ok(Object::Reference(id)) => Some(*id),
    _ => None,
  }
}

fn info_dict<'a>(doc: &'a Document) -> Option<&'a lopdf::Dictionary> {
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

fn dstr(d: &lopdf::Dictionary, k: &[u8]) -> Option<String> {
  match d.get(k).ok()? {
    Object::String(b, _) | Object::Name(b) => Some(String::from_utf8_lossy(b).to_string()),
    _ => None,
  }
}

fn dint(d: &lopdf::Dictionary, k: &[u8]) -> Option<i64> {
  match d.get(k).ok()? {
    Object::Integer(i) => Some(*i),
    _ => None,
  }
}
