/**
 * File Listing Cache with TTL
 *
 * Caches file listings to improve performance on Windows
 * and reduce redundant file system operations.
 */

import type { FileMetadata } from '../types/storage';

interface CacheEntry {
  files: FileMetadata[];
  timestamp: number;
  continuationToken?: string | null;
}

interface CacheKey {
  sourceId: string;
  path: string;
}

const DEFAULT_TTL = 5000; // 5 seconds
const MAX_CACHE_SIZE = 100; // Max number of cached directory listings

class FileListingCache {
  private cache: Map<string, CacheEntry>;
  private ttl: number;

  constructor(ttl: number = DEFAULT_TTL) {
    this.cache = new Map();
    this.ttl = ttl;
  }

  private getKey(sourceId: string, path: string): string {
    return `${sourceId}:${path}`;
  }

  /**
   * Get cached file listing if available and not expired
   */
  get(sourceId: string, path: string): FileMetadata[] | null {
    const key = this.getKey(sourceId, path);
    const entry = this.cache.get(key);

    if (!entry) {
      return null;
    }

    const now = Date.now();
    if (now - entry.timestamp > this.ttl) {
      // Expired, remove from cache
      this.cache.delete(key);
      return null;
    }

    return entry.files;
  }

  /**
   * Store file listing in cache
   */
  set(
    sourceId: string,
    path: string,
    files: FileMetadata[],
    continuationToken?: string | null,
  ): void {
    const key = this.getKey(sourceId, path);

    // Enforce cache size limit (LRU eviction)
    if (this.cache.size >= MAX_CACHE_SIZE && !this.cache.has(key)) {
      // Remove oldest entry
      const firstKey = this.cache.keys().next().value;
      if (firstKey) {
        this.cache.delete(firstKey);
      }
    }

    this.cache.set(key, {
      files,
      timestamp: Date.now(),
      continuationToken,
    });
  }

  /**
   * Invalidate cache for a specific path
   */
  invalidate(sourceId: string, path: string): void {
    const key = this.getKey(sourceId, path);
    this.cache.delete(key);
  }

  /**
   * Invalidate all cache entries for a source
   */
  invalidateSource(sourceId: string): void {
    const keysToDelete: string[] = [];
    for (const key of this.cache.keys()) {
      if (key.startsWith(`${sourceId}:`)) {
        keysToDelete.push(key);
      }
    }
    keysToDelete.forEach((key) => this.cache.delete(key));
  }

  /**
   * Clear all cached entries
   */
  clear(): void {
    this.cache.clear();
  }

  /**
   * Get cache statistics
   */
  getStats(): { size: number; ttl: number } {
    return {
      size: this.cache.size,
      ttl: this.ttl,
    };
  }
}

// Singleton instance
const fileCache = new FileListingCache();

export { fileCache, FileListingCache };
export type { CacheKey };
