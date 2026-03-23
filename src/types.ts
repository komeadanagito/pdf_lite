export type CompressionMode = 0 | 1 | 2 | 3;

export type FileStatus = 'ready' | 'queued' | 'compressing' | 'done' | 'error';

export type FileCategory = 'pdf' | 'image' | 'video';

export interface FileItem {
  id: string;
  path: string;
  name: string;
  size: number;
  category: FileCategory;
  pages?: number;
  status: FileStatus;
  selected: boolean;
  compressedSize?: number;
  outputPath?: string;
  error?: string;
}

export interface PdfInfo {
  path: string;
  file_name: string;
  pages: number;
  size_bytes: number;
  title?: string | null;
  author?: string | null;
  creator?: string | null;
  producer?: string | null;
  version?: string | null;
}

export interface CompressResult {
  input_path: string;
  output_path: string;
  original_size: number;
  compressed_size: number;
  saved_bytes: number;
  compression_ratio: number;
  mode: string;
  duration_ms: number;
}

export interface CompressionProgress {
  path: string;
  stage: string;
  completed: number;
  total: number;
}

export const PDF_EXTENSIONS = ['pdf'];
export const IMAGE_EXTENSIONS = ['jpg', 'jpeg', 'png', 'webp', 'bmp', 'tiff', 'tif', 'gif', 'svg'];
export const VIDEO_EXTENSIONS = ['mp4', 'mov', 'avi', 'mkv', 'wmv', 'flv', 'webm'];

export function getFileExtension(path: string): string {
  return path.split('.').pop()?.toLowerCase() ?? '';
}

export function getFileCategory(path: string): FileCategory | null {
  const ext = getFileExtension(path);
  if (PDF_EXTENSIONS.includes(ext)) return 'pdf';
  if (IMAGE_EXTENSIONS.includes(ext)) return 'image';
  if (VIDEO_EXTENSIONS.includes(ext)) return 'video';
  return null;
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes === 0) return '-';
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 100 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

export function formatRatio(original: number, compressed?: number): string {
  if (!compressed || original <= 0) return '-';
  const saved = (1 - compressed / original) * 100;
  return saved >= 0 ? `↓ ${saved.toFixed(1)}%` : `↑ ${Math.abs(saved).toFixed(1)}%`;
}
