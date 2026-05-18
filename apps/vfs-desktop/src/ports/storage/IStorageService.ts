/**
 * Storage Service Port
 *
 * Interface for storage operations
 */
import { StorageSource } from '../../domain/entities/StorageSource';

export interface IStorageService {
  /**
   * List all storage sources
   */
  listSources(): Promise<StorageSource[]>;

  /**
   * Get a specific storage source by ID
   */
  getSource(sourceId: string): Promise<StorageSource | null>;

  /**
   * Add a new storage source
   */
  addSource(
    source: Omit<StorageSource, 'id' | 'status'>,
  ): Promise<StorageSource>;

  /**
   * Remove a storage source
   */
  removeSource(sourceId: string): Promise<void>;

  /**
   * Update a storage source
   */
  updateSource(
    sourceId: string,
    updates: Partial<StorageSource>,
  ): Promise<StorageSource>;
}
