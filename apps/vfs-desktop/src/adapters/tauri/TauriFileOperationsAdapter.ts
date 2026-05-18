/**
 * Tauri File Operations Adapter
 *
 * Implements IFileOperations using Tauri invoke
 */
import { invoke } from '@tauri-apps/api/core';
import { IFileOperations } from '../../ports/storage/IFileOperations';
import { FileMetadata as DomainFileMetadata } from '../../domain/entities/FileMetadata';
import {
  FileMetadata as StorageFileMetadata,
  ListFilesResponse,
} from '../../types/storage';
import { FileTierStatus } from '../../domain/enums/FileTierStatus';
import { TranscodeStatus } from '../../domain/enums/TranscodeStatus';
import { AssetCategory } from '../../domain/enums/AssetCategory';
import { ApprovalStatus } from '../../domain/enums/ApprovalStatus';

/**
 * Convert storage FileMetadata to domain FileMetadata
 */
function convertToDomainMetadata(
  storage: StorageFileMetadata,
): DomainFileMetadata {
  // Explicitly map fields to ensure type compatibility
  const domain: DomainFileMetadata = {
    id: storage.id,
    name: storage.name,
    path: storage.path,
    size: storage.size,
    size_human: storage.size_human,
    lastModified: storage.lastModified,
    mimeType: storage.mimeType,
    thumbnail: storage.thumbnail,
    isDirectory: storage.isDirectory,
    isHidden: storage.isHidden,
    tierStatus: storage.tierStatus as FileTierStatus,
    canWarm: storage.canWarm,
    isCached: storage.isCached,
    isWarmed: storage.isWarmed,
    canTranscode: storage.canTranscode,
    transcodeStatus: storage.transcodeStatus as TranscodeStatus | undefined,
    transcodeProgress: storage.transcodeProgress,
    duration: storage.duration,
    width: storage.width,
    height: storage.height,
    frameRate: storage.frameRate,
    videoCodec: storage.videoCodec,
    audioCodec: storage.audioCodec,
    audioChannels: storage.audioChannels,
    audioSampleRate: storage.audioSampleRate,
    audioBitrate: storage.audioBitrate,
    videoBitrate: storage.videoBitrate,
    container: storage.container,
    colorSpace: storage.colorSpace,
    hdrFormat: storage.hdrFormat,
    tags: storage.tags?.map((t) => (typeof t === 'string' ? t : t.name)),
    colorLabel: storage.colorLabel,
    comments: storage.comments,
    createdAt: storage.createdAt,
    project: storage.project,
    client: storage.client,
    department: storage.department,
    assetCategory: storage.assetCategory as AssetCategory | undefined,
    usageRights: storage.usageRights,
    approvalStatus: storage.approvalStatus as ApprovalStatus | undefined,
    createdBy: storage.createdBy,
    modifiedBy: storage.modifiedBy,
    expiresAt: storage.expiresAt,
    customFields: storage.customFields,
  };
  return domain;
}

export class TauriFileOperationsAdapter implements IFileOperations {
  async listFiles(
    sourceId: string,
    path: string,
  ): Promise<DomainFileMetadata[]> {
    // Convert camelCase to snake_case for Tauri
    // Handle both old array response and new paginated response for backward compatibility
    const response = await invoke<StorageFileMetadata[] | ListFilesResponse>(
      'vfs_list_files',
      {
        source_id: sourceId,
        path,
      },
    );

    // Check if response is paginated (has 'files' property) or legacy array
    const files = Array.isArray(response) ? response : response.files || [];
    return files.map(convertToDomainMetadata);
  }

  async getFileMetadata(
    sourceId: string,
    path: string,
  ): Promise<DomainFileMetadata | null> {
    try {
      // Note: This command may not exist yet - placeholder for future implementation
      // Convert camelCase to snake_case for Tauri
      const metadata = await invoke<StorageFileMetadata>(
        'vfs_get_file_metadata',
        {
          source_id: sourceId,
          path,
        },
      );
      return convertToDomainMetadata(metadata);
    } catch {
      return null;
    }
  }

  async deleteFiles(sourceId: string, paths: string[]): Promise<void> {
    // Validate inputs
    if (!sourceId || sourceId.trim() === '') {
      throw new Error('Source ID cannot be empty');
    }

    if (!paths || paths.length === 0) {
      throw new Error('No files to delete');
    }

    // Normalize paths
    const normalizePath = (path: string): string => {
      const trimmed = path.trim();
      if (trimmed === '' || trimmed === '/') {
        return '/';
      }
      const withoutLeading = trimmed.replace(/^\/+/, '');
      const withoutTrailing = withoutLeading.replace(/\/+$/, '');
      return withoutTrailing === '' ? '/' : `/${withoutTrailing}`;
    };

    // Validate: cannot delete root directory
    const normalizedPaths = paths.map(normalizePath);
    if (normalizedPaths.some((p) => p === '/')) {
      throw new Error('Cannot delete root directory');
    }

    // Convert camelCase to snake_case for Tauri and delete files in parallel
    const deletePromises = normalizedPaths.map((path) =>
      invoke('vfs_delete', {
        source_id: sourceId.trim(),
        path,
      }).catch((error) => {
        const errorMessage =
          error instanceof Error ? error.message : 'Unknown error';
        throw new Error(`Failed to delete ${path}: ${errorMessage}`);
      }),
    );

    await Promise.all(deletePromises);
  }

  async renameFile(
    sourceId: string,
    oldPath: string,
    newName: string,
  ): Promise<DomainFileMetadata> {
    // Validate inputs
    if (!sourceId || sourceId.trim() === '') {
      throw new Error('Source ID cannot be empty');
    }

    if (!oldPath || oldPath.trim() === '') {
      throw new Error('Old path cannot be empty');
    }

    if (!newName || newName.trim() === '') {
      throw new Error('New name cannot be empty');
    }

    // Normalize old path: trim whitespace, ensure leading slash, remove trailing slashes
    const normalizePath = (path: string): string => {
      const trimmed = path.trim();
      if (trimmed === '' || trimmed === '/') {
        return '/';
      }
      const withoutLeading = trimmed.replace(/^\/+/, '');
      const withoutTrailing = withoutLeading.replace(/\/+$/, '');
      return withoutTrailing === '' ? '/' : `/${withoutTrailing}`;
    };

    const normalizedOldPath = normalizePath(oldPath);

    // Validate: cannot rename root directory
    if (normalizedOldPath === '/') {
      throw new Error('Cannot rename root directory');
    }

    // Validate new name: check for invalid characters
    const trimmedNewName = newName.trim();
    if (trimmedNewName === '' || trimmedNewName === '/') {
      throw new Error('Invalid file name');
    }

    // Check for invalid characters in new name (platform-specific)
    // eslint-disable-next-line no-control-regex
    const invalidChars = /[<>:"|?*\x00-\x1f]/;
    if (invalidChars.test(trimmedNewName)) {
      throw new Error(`Invalid characters in file name: ${trimmedNewName}`);
    }

    // Build new path from old path and new name
    const pathParts = normalizedOldPath.split('/').filter((p) => p.length > 0);
    pathParts.pop(); // Remove old filename
    const newPath =
      pathParts.length === 0
        ? `/${trimmedNewName}`
        : `/${[...pathParts, trimmedNewName].join('/')}`;

    // Convert camelCase to snake_case for Tauri
    try {
      await invoke('vfs_rename', {
        source_id: sourceId.trim(),
        old_path: normalizedOldPath,
        new_path: newPath,
      });
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error';
      throw new Error(`Failed to rename file: ${errorMessage}`);
    }

    // Return updated metadata by listing files (simplified - could be optimized)
    const files = await this.listFiles(sourceId, newPath);
    const file = files.find((f) => f.path === newPath);
    if (!file) {
      throw new Error(`File not found after rename: ${newPath}`);
    }
    return file;
  }

  async createDirectory(sourceId: string, path: string): Promise<void> {
    // Convert camelCase to snake_case for Tauri
    await invoke('vfs_mkdir', {
      source_id: sourceId,
      path,
    });
  }
}
