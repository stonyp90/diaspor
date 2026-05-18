/**
 * File Metadata Entity
 *
 * Core domain entity representing a file in the virtual file system
 */
import { FileSize } from '../value_objects/FileSize';
import { Path } from '../value_objects/Path';
import { FileTierStatus } from '../enums/FileTierStatus';
import { TranscodeStatus } from '../enums/TranscodeStatus';
import { ApprovalStatus } from '../enums/ApprovalStatus';
import { AssetCategory } from '../enums/AssetCategory';

export interface FileMetadata {
  id: string;
  name: string;
  path: string;
  size: number;
  size_human?: string;
  lastModified: string;
  mimeType?: string;
  thumbnail?: string;

  isDirectory?: boolean;

  /** Is hidden file (starts with . on Unix, or has hidden attribute on Windows) */
  isHidden?: boolean;

  tierStatus: FileTierStatus;
  canWarm: boolean;
  isCached?: boolean;
  isWarmed?: boolean;

  canTranscode: boolean;
  transcodeStatus?: TranscodeStatus;
  transcodeProgress?: number;

  // Media Metadata (video/audio)
  duration?: number;
  width?: number;
  height?: number;
  frameRate?: number;
  videoCodec?: string;
  audioCodec?: string;
  audioChannels?: number;
  audioSampleRate?: number;
  audioBitrate?: number;
  videoBitrate?: number;
  container?: string;
  colorSpace?: string;
  hdrFormat?: string;

  // Extended Metadata
  tags?: string[];
  colorLabel?: string;
  comments?: string;
  createdAt?: string;

  // Organization Metadata
  project?: string;
  client?: string;
  department?: string;
  assetCategory?: AssetCategory;
  usageRights?: string;
  approvalStatus?: ApprovalStatus;
  createdBy?: string;
  modifiedBy?: string;
  expiresAt?: string;
  customFields?: Record<string, string | number | boolean>;
}

/**
 * File Metadata Domain Methods
 */
export class FileMetadataEntity {
  constructor(private readonly metadata: FileMetadata) {}

  getPath(): Path {
    return Path.create(this.metadata.path);
  }

  getSize(): FileSize {
    return FileSize.fromBytes(this.metadata.size);
  }

  isMediaFile(): boolean {
    const mimeType = this.metadata.mimeType || '';
    return (
      mimeType.startsWith('video/') ||
      mimeType.startsWith('audio/') ||
      mimeType.startsWith('image/')
    );
  }

  isVideoFile(): boolean {
    return (this.metadata.mimeType || '').startsWith('video/');
  }

  isAudioFile(): boolean {
    return (this.metadata.mimeType || '').startsWith('audio/');
  }

  isImageFile(): boolean {
    return (this.metadata.mimeType || '').startsWith('image/');
  }

  canBeTranscribed(): boolean {
    return this.isVideoFile() || this.isAudioFile();
  }

  toPlainObject(): FileMetadata {
    return { ...this.metadata };
  }
}
