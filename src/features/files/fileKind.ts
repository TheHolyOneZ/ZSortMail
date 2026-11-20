import {
  File,
  FileAudio,
  FileDoc,
  FileImage,
  FilePdf,
  FileText,
  FileVideo,
  FileXls,
  FileZip,
} from '@phosphor-icons/react';
import type { ComponentType } from 'react';

export type FileKind =
  | 'image'
  | 'pdf'
  | 'text'
  | 'audio'
  | 'video'
  | 'doc'
  | 'sheet'
  | 'zip'
  | 'other';

export function fileKind(mime: string, name: string): FileKind {
  const ext = name.toLowerCase().split('.').pop() ?? '';
  if (mime.startsWith('image/')) return 'image';
  if (mime === 'application/pdf' || ext === 'pdf') return 'pdf';
  if (mime.startsWith('audio/')) return 'audio';
  if (mime.startsWith('video/')) return 'video';
  if (mime === 'text/plain' || ext === 'txt' || ext === 'md' || ext === 'log') return 'text';
  if (['doc', 'docx', 'odt', 'rtf', 'pages'].includes(ext) || mime.includes('wordprocessing'))
    return 'doc';
  if (['xls', 'xlsx', 'ods', 'csv', 'numbers'].includes(ext) || mime.includes('spreadsheet'))
    return 'sheet';
  if (['zip', 'gz', 'tar', '7z', 'rar'].includes(ext) || mime.includes('zip')) return 'zip';
  return 'other';
}

export const kindIcon: Record<
  FileKind,
  ComponentType<{ size?: number; weight?: 'regular' | 'fill' }>
> = {
  image: FileImage,
  pdf: FilePdf,
  text: FileText,
  audio: FileAudio,
  video: FileVideo,
  doc: FileDoc,
  sheet: FileXls,
  zip: FileZip,
  other: File,
};

export function canPreview(kind: FileKind): boolean {
  return (
    kind === 'image' || kind === 'pdf' || kind === 'text' || kind === 'audio' || kind === 'video'
  );
}
