/**
 * Tauri Storage Adapter
 *
 * Implements IStorageService using Tauri invoke
 */
import { invoke } from '@tauri-apps/api/core';
import { IStorageService } from '../../ports/storage/IStorageService';
import { StorageSource } from '../../domain/entities/StorageSource';

export class TauriStorageAdapter implements IStorageService {
  async listSources(): Promise<StorageSource[]> {
    const sources = await invoke<StorageSource[]>('vfs_list_sources');
    return sources;
  }

  async getSource(sourceId: string): Promise<StorageSource | null> {
    try {
      // Convert camelCase to snake_case for Tauri
      const source = await invoke<StorageSource>('vfs_get_source', {
        source_id: sourceId,
      });
      return source;
    } catch {
      return null;
    }
  }

  async addSource(
    source: Omit<StorageSource, 'id' | 'status'>,
  ): Promise<StorageSource> {
    // Convert camelCase to snake_case for Tauri
    const newSource = await invoke<StorageSource>('vfs_add_source', {
      name: source.name,
      provider_id: source.providerId,
      category: source.category,
      config: source.config,
    });
    return newSource;
  }

  async removeSource(sourceId: string): Promise<void> {
    // Tauri automatically converts camelCase (JS) to snake_case (Rust)
    await invoke('vfs_remove_source', { sourceId });
  }

  async updateSource(
    sourceId: string,
    updates: Partial<StorageSource>,
  ): Promise<StorageSource> {
    // Convert camelCase to snake_case for Tauri
    const updated = await invoke<StorageSource>('vfs_update_source', {
      source_id: sourceId,
      updates,
    });
    return updated;
  }
}
