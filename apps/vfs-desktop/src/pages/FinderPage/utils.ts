/**
 * FinderPage Utility Functions
 *
 * Helper functions extracted from FinderPage for better maintainability.
 */
import React from 'react';
import type { StorageSource, FileMetadata } from './types';
import type { StorageCategory } from '../../types/storage';
import {
  IconFolder,
  IconCloud,
  IconNetwork,
  IconDatabase,
  IconHome,
  IconDesktop,
  IconDocuments,
  IconDownloads,
  IconPictures,
  IconMusic,
  IconVolumes,
  IconFlame,
  IconSnowflake,
  IconClock,
  getFileIcon as getFileIconComponent,
} from '../../components/CyberpunkIcons';

/**
 * Get display label for storage source
 * Follows standard naming patterns:
 * - SMB/CIFS: \\server\share or //server/share
 * - NFS: server:/export
 * - S3: s3://bucket/prefix
 * - Cloud: provider://container
 */
export function getStorageDisplayLabel(source: StorageSource): string {
  const { providerId, name, config } = source;

  // For named sources, just return the name
  if (name && !name.includes('/') && !name.includes('\\')) {
    return name;
  }

  // Format based on provider type
  switch (providerId) {
    case 'smb':
    case 'cifs': {
      const server = config?.server as string;
      const share = config?.share as string;
      if (server && share) {
        // Windows UNC format
        return `\\\\${server}\\${share}`;
      }
      return name;
    }
    case 'nfs': {
      const server = config?.server as string;
      const exportPath = config?.export as string;
      if (server && exportPath) {
        // NFS format: server:/export
        return `${server}:${exportPath}`;
      }
      return name;
    }
    case 'aws-s3':
    case 's3-compatible': {
      const bucket = config?.bucket as string;
      const prefix = config?.prefix as string;
      if (bucket) {
        // S3 URI format
        return prefix ? `s3://${bucket}/${prefix}` : `s3://${bucket}`;
      }
      return name;
    }
    case 'gcs': {
      const bucket = config?.bucket as string;
      if (bucket) {
        return `gs://${bucket}`;
      }
      return name;
    }
    case 'azure-blob': {
      const account = config?.accountName as string;
      const container = config?.container as string;
      if (account && container) {
        return `azure://${account}/${container}`;
      }
      return name;
    }
    case 'sftp': {
      const host = config?.host as string;
      const path = config?.remotePath as string;
      if (host) {
        return `sftp://${host}${path || '/'}`;
      }
      return name;
    }
    case 'webdav': {
      const url = config?.url as string;
      if (url) {
        return url.replace(/^https?:\/\//, 'dav://');
      }
      return name;
    }
    default:
      return name;
  }
}

/**
 * Get file icon based on file type
 */
export function getFileIcon(file: FileMetadata, size = 48): React.ReactNode {
  const isFolder =
    file.isDirectory || file.mimeType === 'folder' || file.path.endsWith('/');

  if (isFolder) {
    // Use simple folder icon - cleaner at all sizes
    return React.createElement(IconFolder, {
      size,
      color: 'currentColor',
      glow: false,
      className: 'folder-icon',
    });
  }

  // Use the helper function to get the appropriate icon component
  // All file icons use currentColor to inherit from CSS variables
  const IconComponent = getFileIconComponent(file.name, file.mimeType);
  return React.createElement(IconComponent, {
    size,
    color: 'currentColor',
    glow: false,
  });
}

/**
 * Format date string for display
 */
export function formatDate(dateStr: string | undefined): string {
  if (!dateStr || dateStr === '' || dateStr === '0') return '-';
  try {
    // Handle ISO 8601 format (YYYY-MM-DDTHH:MM:SS.sssZ) or legacy format (YYYY-MM-DD HH:MM:SS)
    let date: Date;
    if (dateStr.includes('T')) {
      // ISO format
      date = new Date(dateStr);
    } else if (dateStr.match(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/)) {
      // Legacy format: YYYY-MM-DD HH:MM:SS - convert to ISO
      date = new Date(dateStr.replace(' ', 'T') + 'Z');
    } else if (/^\d+$/.test(dateStr)) {
      // Unix timestamp (seconds)
      date = new Date(parseInt(dateStr, 10) * 1000);
    } else {
      // Try parsing as-is
      date = new Date(dateStr);
    }

    if (isNaN(date.getTime())) return '-';

    // Check if date is Unix epoch (1970-01-01) - treat as invalid
    const epochTime = new Date('1970-01-01T00:00:00Z').getTime();
    if (date.getTime() <= epochTime) return '-';

    // Format as date and time
    return date.toLocaleString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return '-';
  }
}

/**
 * Format file size for display
 */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/**
 * Check if source is object storage (S3, GCS, Azure Blob)
 */
export function isObjectStorage(source: StorageSource | null): boolean {
  if (!source) return false;
  return (
    source.category === 'cloud' ||
    source.providerId === 'aws-s3' ||
    source.providerId === 's3-compatible' ||
    source.providerId === 'gcs' ||
    source.providerId === 'azure-blob'
  );
}

/**
 * Check if storage supports file system operations (copy, cut, paste, rename, duplicate)
 * Only local, network, and hybrid storage support these operations.
 * Cloud object storage (S3, GCS, Azure Blob) does not support true file system operations.
 */
export function supportsFilesystemOperations(
  source: StorageSource | null,
): boolean {
  if (!source) return false;
  return (
    source.category === 'local' ||
    source.category === 'network' ||
    source.category === 'hybrid'
  );
}

/**
 * Normalize path for VFS operations
 */
export function normalizePath(path: string): string {
  if (!path) return '/';
  const normalized = path.trim().replace(/\/+/g, '/');
  if (normalized === '' || normalized === '/') return '/';
  return normalized.startsWith('/') ? normalized : `/${normalized}`;
}

/**
 * Get parent path from a file path
 */
export function getParentPath(path: string): string {
  const normalized = normalizePath(path);
  if (normalized === '/') return '/';
  const parts = normalized.split('/').filter(Boolean);
  if (parts.length <= 1) return '/';
  parts.pop();
  return '/' + parts.join('/');
}

/**
 * Get cyberpunk icon component based on folder name
 */
function getLocationIcon(name: string) {
  const lowerName = name.toLowerCase();
  if (lowerName === 'home' || lowerName.includes('user')) return IconHome;
  if (lowerName === 'desktop') return IconDesktop;
  if (lowerName === 'documents' || lowerName === 'docs') return IconDocuments;
  if (lowerName === 'downloads') return IconDownloads;
  if (lowerName === 'pictures' || lowerName === 'photos') return IconPictures;
  if (lowerName === 'music' || lowerName === 'audio') return IconMusic;
  if (lowerName === 'volumes' || lowerName === 'drives') return IconVolumes;
  return IconFolder;
}

/**
 * Get storage icon based on category
 */
export function getStorageIcon(source: StorageSource) {
  switch (source.category) {
    case 'local':
      return getLocationIcon(source.name);
    case 'cloud':
      return IconCloud;
    case 'network':
      return IconNetwork;
    case 'hybrid':
      return IconDatabase;
    default:
      return IconFolder;
  }
}

/**
 * Get storage class icon component based on tier status
 * Returns an icon component for visual representation of storage tier
 * Only returns icons for object storage tiers (cold, nearline, hot)
 */
export function getStorageClassIcon(tierStatus: string | undefined): React.FC<{
  size?: number;
  color?: string;
  glow?: boolean;
  className?: string;
}> | null {
  if (!tierStatus) return null;

  const tier = tierStatus.toLowerCase();

  switch (tier) {
    case 'hot':
      // Flame icon for hot storage
      return IconFlame;
    case 'cold':
      // Snowflake icon for cold storage
      return IconSnowflake;
    case 'nearline':
      // Clock icon for nearline storage
      return IconClock;
    default:
      return null;
  }
}

/**
 * Get color for storage category based on tier mapping
 * Returns a tier color for visual representation:
 * - Local → Hot (green)
 * - Cloud (S3 Standard) → Nearline (purple)
 * - Cold (S3 Instant Retrieval) → Cold (cyan)
 */
export function getStorageCategoryColor(
  category: StorageCategory | undefined,
  tierStatus?: 'hot' | 'warm' | 'cold' | 'nearline',
): string {
  if (!category) return 'var(--finder-text-quaternary)';

  // Cold tier always uses cold color (for S3 Instant Retrieval, etc.)
  if (tierStatus === 'cold') {
    return 'var(--tier-cold, #64d2ff)';
  }

  switch (category) {
    case 'local':
      // Local storage → Hot tier color
      return 'var(--tier-hot, #30d158)';
    case 'cloud':
      // Cloud storage (S3 Standard) → Nearline tier color
      return 'var(--tier-nearline, #bf5af2)';
    case 'network':
      return 'var(--tier-nearline, #bf5af2)'; // Network shares also use nearline
    case 'block':
      return 'var(--tier-hot, #30d158)'; // Block storage → Hot
    case 'hybrid':
      return 'var(--tier-nearline, #bf5af2)'; // Hybrid → Nearline
    case 'custom':
      return 'var(--tier-archive, #8e8e93)'; // Custom → Archive gray
    default:
      return 'var(--finder-text-quaternary)';
  }
}

/**
 * Get storage class letter and tier class based on category and tier status
 * Returns the letter (L, N, C, H) and CSS class for the badge
 * Mapping:
 * - Local → L (Local)
 * - Network mount → H (Hot)
 * - Cloud/S3 Standard → N (Nearline)
 * - Cold/S3 Instant Retrieval → C (Cold)
 */
export function getStorageClassBadge(
  category: StorageCategory | undefined,
  tierStatus?: string,
): { letter: string; tierClass: string } {
  if (!category) {
    return { letter: '', tierClass: '' };
  }

  // Map categories to storage class badges
  // For local storage, always show 'L' regardless of tier status
  // For other categories, check tier status for cold storage
  switch (category) {
    case 'local':
      // Local storage always shows 'L' - tier status doesn't apply
      return { letter: 'L', tierClass: 'local' };
    case 'network':
      // Network mounts show 'H' for Hot
      return { letter: 'H', tierClass: 'hot' };
    case 'cloud':
      // Cloud storage: check tier status for cold, otherwise nearline
      if (tierStatus) {
        const tier = tierStatus.toLowerCase();
        if (tier === 'cold' || tier === 'instant-retrieval') {
          return { letter: 'C', tierClass: 'cold' };
        }
      }
      return { letter: 'N', tierClass: 'nearline' }; // Cloud/S3 Standard = Nearline (N)
    case 'block':
      return { letter: 'C', tierClass: 'cold' }; // Block = Cold (C)
    case 'hybrid':
      return { letter: 'N', tierClass: 'nearline' }; // Hybrid = Nearline (N)
    case 'custom':
      return { letter: 'N', tierClass: 'nearline' }; // Custom = Nearline (N) (default)
    default:
      return { letter: '', tierClass: '' };
  }
}
