/**
 * Clipboard Service Port
 *
 * Interface for clipboard operations
 */
import { ClipboardContent } from '../../domain/entities/ClipboardContent';

export interface IClipboardService {
  /**
   * Copy files to clipboard
   */
  copy(sourceId: string, paths: string[]): Promise<void>;

  /**
   * Cut files to clipboard
   */
  cut(sourceId: string, paths: string[]): Promise<void>;

  /**
   * Paste files from clipboard
   */
  paste(
    destSourceId: string,
    destPath: string,
  ): Promise<{ operationId: string }>;

  /**
   * Get clipboard content
   */
  getClipboardContent(): Promise<ClipboardContent | null>;

  /**
   * Check if clipboard has files
   */
  hasFiles(): Promise<boolean>;

  /**
   * Clear clipboard
   */
  clear(): Promise<void>;
}
