/**
 * Storage Icon Utility
 *
 * Get appropriate icon for storage sources
 */
import { StorageSource } from '../../domain/entities/StorageSource';

export function getStorageIcon(source: StorageSource): string {
  switch (source.category) {
    case 'local':
      return 'hard-drive';
    case 'cloud':
      return 'cloud';
    case 'network':
      return 'network';
    case 'hybrid':
      return 'server';
    default:
      return 'folder';
  }
}
