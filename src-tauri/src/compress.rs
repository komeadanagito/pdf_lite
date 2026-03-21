use std::{
  fs,
  io::Cursor,
  path::{Path, PathBuf},
  time::Instant,
};

use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, ImageEncoder};
use lopdf::{Document, Object, ObjectId, Stream};
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
    if let Some(Object::Stream(stream)) = doc.objects.get_mut(&id) {
      if let Some(new_bytes) = recompress_image_stream(stream, quality, max_dimension) {
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

fn recompress_image_stream(stream: &mut Stream, quality: u8, max_dimension: Option<u32>) -> Option<Vec<u8>> {
  let image = image::load_from_memory(&stream.content).ok()?;
  let image = if let Some(max_dimension) = max_dimension {
    let (width, height) = (image.width(), image.height());
    if width > max_dimension || height > max_dimension {
      image.resize(max_dimension, max_dimension, FilterType::Lanczos3)
    } else {
      image
    }
  } else {
    image
  };

  let mut output = Cursor::new(Vec::new());
  let mut encoder = JpegEncoder::new_with_quality(&mut output, quality);
  encoder.encode_image(&image).ok()?;
  Some(output.into_inner())
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
