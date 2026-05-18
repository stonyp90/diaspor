/**
 * Tauri Clipboard Adapter
 *
 * Implements IClipboardService using Tauri invoke
 */
import { invoke } from '@tauri-apps/api/core';
import { IClipboardService } from '../../ports/storage/IClipboardService';
import { ClipboardContent } from '../../domain/entities/ClipboardContent';

export class TauriClipboardAdapter implements IClipboardService {
  async copy(sourceId: string, paths: string[]): Promise<void> {
    // Tauri automatically converts JavaScript camelCase (sourceId) to Rust snake_case (source_id)
    await invoke('vfs_clipboard_copy', {
      sourceId,
      paths,
    });
  }

  async cut(sourceId: string, paths: string[]): Promise<void> {
    // Tauri automatically converts JavaScript camelCase (sourceId) to Rust snake_case (source_id)
    await invoke('vfs_clipboard_cut', {
      sourceId,
      paths,
    });
  }

  async paste(
    destSourceId: string,
    destPath: string,
  ): Promise<{ operationId: string }> {
    // Convert camelCase to snake_case for Tauri
    const result = await invoke<{ operation_id: string }>(
      'vfs_clipboard_paste_to_vfs',
      {
        dest_source_id: destSourceId,
        dest_path: destPath,
      },
    );
    // Convert snake_case to camelCase for TypeScript
    return { operationId: result.operation_id };
  }

  async getClipboardContent(): Promise<ClipboardContent | null> {
    try {
      const content = await invoke<ClipboardContent>('vfs_clipboard_read');
      return content;
    } catch {
      return null;
    }
  }

  async hasFiles(): Promise<boolean> {
    try {
      const hasFiles = await invoke<boolean>('vfs_clipboard_has_files');
      return hasFiles;
    } catch {
      return false;
    }
  }

  async clear(): Promise<void> {
    await invoke('vfs_clipboard_clear');
  }
}
