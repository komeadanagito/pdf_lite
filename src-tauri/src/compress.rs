use std::{
  fs,
  io::{Cursor, Read},
  path::{Path, PathBuf},
  time::Instant,
};

use flate2::read::ZlibDecoder;
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
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
  let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
  let doc = Document::load(path).map_err(|error| error.to_string())?;
  let pages = doc.get_pages().len();
  let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("document.pdf").to_string();
  let info_dict = info_dictionary(&doc);

  Ok(PdfInfo {
    path: path.to_string_lossy().to_string(),
    file_name,
    pages,
    size_bytes: metadata.len(),
    title: info_dict.as_ref().and_then(|dict| dictionary_string(dict, b"Title")),
    author: info_dict.as_ref().and_then(|dict| dictionary_string(dict, b"Author")),
    creator: info_dict.as_ref().and_then(|dict| dictionary_string(dict, b"Creator")),
    producer: info_dict.as_ref().and_then(|dict| dictionary_string(dict, b"Producer")),
    version: Some(doc.version),
  })
}

pub fn compress_pdf_at_path(app: &AppHandle, path: &Path, mode: CompressionMode) -> Result<CompressResult, String> {
  let original_size = fs::metadata(path).map_err(|error| error.to_string())?.len();
  let started = Instant::now();
  emit_progress(app, path, "loading", 0, 4);

  let mut document = Document::load(path).map_err(|error| error.to_string())?;
  document.version = "1.5".to_string();

  emit_progress(app, path, "lossless optimization", 1, 4);
  compress_lossless(&mut document);

  match mode {
    CompressionMode::Lossless => {}
    CompressionMode::Light => {
      emit_progress(app, path, "recompressing images", 2, 4);
      recompress_images(&mut document, 85, None)?;
    }
    CompressionMode::Standard => {
      emit_progress(app, path, "recompressing images", 2, 4);
      recompress_images(&mut document, 80, None)?;
      emit_progress(app, path, "stripping annotations", 3, 4);
      strip_interactive_content(&mut document, true);
    }
    CompressionMode::Extreme => {
      emit_progress(app, path, "recompressing images", 2, 4);
      recompress_images(&mut document, 72, Some(2200))?;
      emit_progress(app, path, "stripping annotations", 3, 4);
      strip_interactive_content(&mut document, true);
      strip_extreme_metadata(&mut document);
    }
  }

  let output_path = build_output_path(path);
  if output_path.exists() {
    let _ = fs::remove_file(&output_path);
  }
  document.save(&output_path).map_err(|error| error.to_string())?;

  let compressed_size = fs::metadata(&output_path).map_err(|error| error.to_string())?.len();
  let saved_bytes = original_size as i64 - compressed_size as i64;
  let compression_ratio = if original_size == 0 {
    0.0
  } else {
    (1.0 - compressed_size as f64 / original_size as f64) * 100.0
  };

  emit_progress(app, path, "completed", 4, 4);

  Ok(CompressResult {
    input_path: path.to_string_lossy().to_string(),
    output_path: output_path.to_string_lossy().to_string(),
    original_size,
    compressed_size,
    saved_bytes,
    compression_ratio,
    mode: mode.label().to_string(),
    duration_ms: started.elapsed().as_millis(),
  })
}

fn compress_lossless(doc: &mut Document) {
  let _ = doc.compress();
}

fn recompress_images(doc: &mut Document, quality: u8, max_dimension: Option<u32>) -> Result<(), String> {
  let image_ids: Vec<ObjectId> = doc
    .objects
    .iter()
    .filter_map(|(id, object)| match object {
      Object::Stream(stream) if is_image_stream(stream) => Some(*id),
      _ => None,
    })
    .collect();

  for id in image_ids {
    // Resolve ICCBased component count before mutably borrowing the stream
    let icc_components = resolve_icc_components(doc, id);

    if let Some(Object::Stream(stream)) = doc.objects.get_mut(&id) {
      if let Some(new_bytes) = recompress_image_stream(stream, quality, max_dimension, icc_components) {
        stream.content = new_bytes;
        stream.dict.remove(b"Filter");
        stream.dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        stream.dict.set("Length", Object::Integer(stream.content.len() as i64));
        stream.dict.remove(b"DecodeParms");
      }
    }
  }

  Ok(())
}

/// Resolve the component count for an ICCBased color space by following the reference.
fn resolve_icc_components(doc: &Document, image_id: ObjectId) -> Option<usize> {
  let stream = match doc.objects.get(&image_id) {
    Some(Object::Stream(s)) => s,
    _ => return None,
  };

  let cs = stream.dict.get(b"ColorSpace").ok()?;
  let arr = match cs {
    Object::Array(a) => a,
    _ => return None,
  };

  match arr.first() {
    Some(Object::Name(name)) if name.as_slice() == b"ICCBased" => {}
    _ => return None,
  }

  let ref_id = match arr.get(1) {
    Some(Object::Reference(id)) => *id,
    _ => return None,
  };

  match doc.objects.get(&ref_id) {
    Some(Object::Stream(icc_stream)) => dict_integer(&icc_stream.dict, b"N").map(|n| n as usize),
    _ => None,
  }
}

fn recompress_image_stream(stream: &mut Stream, quality: u8, max_dimension: Option<u32>, icc_components: Option<usize>) -> Option<Vec<u8>> {
  // Determine current filter - only handle Name filters (not arrays of filters)
  let filter_name = match stream.dict.get(b"Filter").ok()? {
    Object::Name(name) => name.clone(),
    _ => return None,
  };

  let image = match filter_name.as_slice() {
    b"DCTDecode" => {
      // Content is raw JPEG bytes - can decode directly
      image::load_from_memory(&stream.content).ok()?
    }
    b"FlateDecode" => {
      // Content is zlib-compressed raw pixel data - need full decode pipeline
      decode_flate_image(stream, icc_components)?
    }
    _ => return None,
  };

  // Skip tiny images - recompressing them as JPEG can increase size
  if image.width() < 16 || image.height() < 16 {
    return None;
  }

  let image = if let Some(max_dim) = max_dimension {
    let (w, h) = (image.width(), image.height());
    if w > max_dim || h > max_dim {
      image.resize(max_dim, max_dim, FilterType::Lanczos3)
    } else {
      image
    }
  } else {
    image
  };

  let mut output = Cursor::new(Vec::new());
  let mut encoder = JpegEncoder::new_with_quality(&mut output, quality);
  encoder.encode_image(&image).ok()?;

  // Only use recompressed version if it is actually smaller
  let new_bytes = output.into_inner();
  if new_bytes.len() >= stream.content.len() {
    return None;
  }
  Some(new_bytes)
}

/// Decode a FlateDecode image stream into a DynamicImage.
fn decode_flate_image(stream: &Stream, icc_components: Option<usize>) -> Option<image::DynamicImage> {
  // Decompress zlib data
  let mut decoder = ZlibDecoder::new(stream.content.as_slice());
  let mut raw = Vec::new();
  decoder.read_to_end(&mut raw).ok()?;

  let width = dict_integer(&stream.dict, b"Width")? as usize;
  let height = dict_integer(&stream.dict, b"Height")? as usize;
  let bits = dict_integer(&stream.dict, b"BitsPerComponent").unwrap_or(8);
  if bits != 8 {
    return None;
  }

  // Use directly-known color space, pre-resolved ICCBased count, or infer from data size
  let components = color_space_components(&stream.dict)
    .or(icc_components)
    .or_else(|| infer_components_from_size(&raw, width, height, stream));

  // Undo PNG prediction filter if present (Predictor 10-15)
  let components = components?;

  let predictor = stream
    .dict
    .get(b"DecodeParms")
    .ok()
    .and_then(|p| match p {
      Object::Dictionary(d) => dict_integer(d, b"Predictor"),
      _ => None,
    })
    .unwrap_or(1);

  let pixels = if predictor >= 10 {
    undo_png_prediction(&raw, width, height, components)?
  } else {
    raw
  };

  match components {
    1 => {
      let img = image::GrayImage::from_raw(width as u32, height as u32, pixels)?;
      Some(image::DynamicImage::ImageLuma8(img))
    }
    3 => {
      let img = image::RgbImage::from_raw(width as u32, height as u32, pixels)?;
      Some(image::DynamicImage::ImageRgb8(img))
    }
    4 => {
      // CMYK → convert to RGB before re-encoding as JPEG
      let rgb = cmyk_to_rgb(&pixels);
      let img = image::RgbImage::from_raw(width as u32, height as u32, rgb)?;
      Some(image::DynamicImage::ImageRgb8(img))
    }
    _ => None,
  }
}

/// Reverse the per-row PNG prediction filtering applied by many PDF writers.
fn undo_png_prediction(data: &[u8], width: usize, height: usize, components: usize) -> Option<Vec<u8>> {
  let stride = width * components;
  let row_size = stride + 1; // first byte of each row is the filter type

  if data.len() < row_size * height {
    return None;
  }

  let mut result = Vec::with_capacity(stride * height);
  let mut prev = vec![0u8; stride];

  for row in 0..height {
    let base = row * row_size;
    let filter_type = data[base];
    let src = &data[base + 1..base + row_size];
    let mut cur = src.to_vec();

    match filter_type {
      0 => {} // None
      1 => {
        // Sub
        for i in components..stride {
          cur[i] = cur[i].wrapping_add(cur[i - components]);
        }
      }
      2 => {
        // Up
        for i in 0..stride {
          cur[i] = cur[i].wrapping_add(prev[i]);
        }
      }
      3 => {
        // Average
        for i in 0..stride {
          let a = if i >= components { cur[i - components] as u16 } else { 0 };
          let b = prev[i] as u16;
          cur[i] = cur[i].wrapping_add(((a + b) / 2) as u8);
        }
      }
      4 => {
        // Paeth
        for i in 0..stride {
          let a = if i >= components { cur[i - components] } else { 0 };
          let b = prev[i];
          let c = if i >= components { prev[i - components] } else { 0 };
          cur[i] = cur[i].wrapping_add(paeth(a, b, c));
        }
      }
      _ => return None,
    }

    result.extend_from_slice(&cur);
    prev = cur;
  }

  Some(result)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
  let (a, b, c) = (a as i32, b as i32, c as i32);
  let p = a + b - c;
  let pa = (p - a).abs();
  let pb = (p - b).abs();
  let pc = (p - c).abs();
  if pa <= pb && pa <= pc {
    a as u8
  } else if pb <= pc {
    b as u8
  } else {
    c as u8
  }
}

fn color_space_components(dict: &Dictionary) -> Option<usize> {
  let cs = dict.get(b"ColorSpace").ok()?;
  match cs {
    Object::Name(name) => match name.as_slice() {
      b"DeviceGray" | b"CalGray" => Some(1),
      b"DeviceRGB" | b"CalRGB" => Some(3),
      b"DeviceCMYK" => Some(4),
      _ => None,
    },
    Object::Array(arr) => match arr.first() {
      Some(Object::Name(name)) => match name.as_slice() {
        b"ICCBased" => None, // Resolved separately via resolve_icc_components
        b"CalRGB" => Some(3),
        b"CalGray" => Some(1),
        _ => None,
      },
      _ => None,
    },
    _ => None,
  }
}

/// Infer component count from decompressed data size when color space is unresolvable.
fn infer_components_from_size(raw: &[u8], width: usize, height: usize, stream: &Stream) -> Option<usize> {
  if width == 0 || height == 0 {
    return None;
  }
  let has_predictor = stream
    .dict
    .get(b"DecodeParms")
    .ok()
    .and_then(|p| match p {
      Object::Dictionary(d) => dict_integer(d, b"Predictor"),
      _ => None,
    })
    .map(|p| p >= 10)
    .unwrap_or(false);

  // With PNG predictor: each row has an extra filter-type byte
  let pixels_per_row = width;
  for &n in &[1usize, 3, 4] {
    let expected = if has_predictor {
      height * (pixels_per_row * n + 1)
    } else {
      height * pixels_per_row * n
    };
    if raw.len() == expected {
      return Some(n);
    }
  }
  None
}

fn dict_integer(dict: &Dictionary, key: &[u8]) -> Option<i64> {
  match dict.get(key).ok()? {
    Object::Integer(i) => Some(*i),
    _ => None,
  }
}

fn cmyk_to_rgb(cmyk: &[u8]) -> Vec<u8> {
  let mut rgb = Vec::with_capacity(cmyk.len() / 4 * 3);
  for chunk in cmyk.chunks_exact(4) {
    let c = chunk[0] as f32 / 255.0;
    let m = chunk[1] as f32 / 255.0;
    let y = chunk[2] as f32 / 255.0;
    let k = chunk[3] as f32 / 255.0;
    rgb.push(((1.0 - c) * (1.0 - k) * 255.0) as u8);
    rgb.push(((1.0 - m) * (1.0 - k) * 255.0) as u8);
    rgb.push(((1.0 - y) * (1.0 - k) * 255.0) as u8);
  }
  rgb
}

fn strip_interactive_content(doc: &mut Document, remove_annotations: bool) {
  doc.trailer.remove(b"Info");

  if let Some(root_id) = catalog_object_id(doc) {
    if let Some(Object::Dictionary(root)) = doc.objects.get_mut(&root_id) {
      root.remove(b"Outlines");
      root.remove(b"AcroForm");
      root.remove(b"Names");
    }
  }

  if remove_annotations {
    let page_ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    for page_id in page_ids {
      if let Some(Object::Dictionary(page)) = doc.objects.get_mut(&page_id) {
        page.remove(b"Annots");
      }
    }
  }
}

fn strip_extreme_metadata(doc: &mut Document) {
  if let Some(root_id) = catalog_object_id(doc) {
    if let Some(Object::Dictionary(root)) = doc.objects.get_mut(&root_id) {
      root.remove(b"Metadata");
      root.remove(b"Lang");
      root.remove(b"PageLabels");
      root.remove(b"StructTreeRoot");
      root.remove(b"MarkInfo");
    }
  }
}

fn build_output_path(path: &Path) -> PathBuf {
  let parent = path.parent().unwrap_or_else(|| Path::new("."));
  let stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or("document");
  parent.join(format!("{}_compressed.pdf", stem))
}

fn emit_progress(app: &AppHandle, path: &Path, stage: &str, completed: usize, total: usize) {
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

fn catalog_object_id(doc: &Document) -> Option<ObjectId> {
  match doc.trailer.get(b"Root") {
    Ok(Object::Reference(id)) => Some(*id),
    _ => None,
  }
}

fn info_dictionary<'a>(doc: &'a Document) -> Option<&'a lopdf::Dictionary> {
  let info_id = match doc.trailer.get(b"Info") {
    Ok(Object::Reference(id)) => Some(*id),
    _ => None,
  }?;

  match doc.objects.get(&info_id) {
    Some(Object::Dictionary(dict)) => Some(dict),
    Some(Object::Stream(stream)) => Some(&stream.dict),
    _ => None,
  }
}

fn dictionary_string(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
  let value = dict.get(key).ok()?;
  match value {
    Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
    Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
    _ => None,
  }
}

fn is_image_stream(stream: &Stream) -> bool {
  match stream.dict.get(b"Subtype") {
    Ok(Object::Name(name)) => name.as_slice() == b"Image",
    _ => false,
  }
}
