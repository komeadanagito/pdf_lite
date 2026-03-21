export type CompressionMode = 0 | 1 | 2 | 3;

export type FileStatus = 'ready' | 'queued' | 'compressing' | 'done' | 'error';

export interface PdfFileItem {
  id: string;
  path: string;
  name: string;
  size: number;
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
