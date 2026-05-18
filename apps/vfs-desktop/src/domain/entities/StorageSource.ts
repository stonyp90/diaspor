/**
 * Storage Source Entity
 *
 * Core domain entity representing a storage source instance
 */
import { StorageCategory } from '../value_objects/StorageCategory';
import { StorageStatus } from '../enums/StorageStatus';
import { FileTierStatus } from '../enums/FileTierStatus';

// Re-export enums for convenience
export { StorageStatus };

export interface StorageSource {
  /** Unique instance ID */
  id: string;

  /** Display name */
  name: string;

  /** Provider ID (references StorageProviderDefinition.id) */
  providerId: string;

  /** Provider category (for quick filtering) */
  category: StorageCategory;

  /** Connection configuration (provider-specific) */
  config: Record<string, unknown>;

  /** Current connection status */
  status: StorageStatus;

  /** Error message if status is 'error' */
  error?: string;

  /** Is this source read-only */
  readOnly?: boolean;

  /** Last connected timestamp */
  lastConnected?: string;

  /** Storage tier status */
  tierStatus?: FileTierStatus;

  /** Whether this is a mounted volume that can be ejected (DMG, external drive, etc.) */
  isEjectable?: boolean;

  /** Whether this is a system location (Home, Documents, etc.) - not ejectable */
  isSystemLocation?: boolean;

  // Backward compatibility properties
  /** @deprecated Use providerId instead - maps to provider type */
  type?: string;

  /** @deprecated Use status === 'connected' */
  connected?: boolean;

  /** @deprecated Use config.path */
  path?: string;

  /** @deprecated Use config.bucket */
  bucket?: string;

  /** @deprecated Use config.region */
  region?: string;
}

/**
 * Storage Source Domain Methods
 */
export class StorageSourceEntity {
  constructor(private readonly source: StorageSource) {}

  isConnected(): boolean {
    return this.source.status === StorageStatus.Connected;
  }

  isObjectStorage(): boolean {
    return this.source.category === 'cloud';
  }

  canWrite(): boolean {
    return !this.source.readOnly && this.isConnected();
  }

  getCategory(): StorageCategory {
    return this.source.category;
  }

  toPlainObject(): StorageSource {
    return { ...this.source };
  }
}
