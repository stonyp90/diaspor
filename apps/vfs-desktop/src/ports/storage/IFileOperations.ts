/**
 * File Operations Port
 *
 * Interface for file CRUD operations
 */
import { FileMetadata } from '../../domain/entities/FileMetadata';

export interface IFileOperations {
  /**
   * List files in a directory
   */
  listFiles(sourceId: string, path: string): Promise<FileMetadata[]>;

  /**
   * Get file metadata
   */
  getFileMetadata(sourceId: string, path: string): Promise<FileMetadata | null>;

  /**
   * Delete files
   */
  deleteFiles(sourceId: string, paths: string[]): Promise<void>;

  /**
   * Rename a file
   */
  renameFile(
    sourceId: string,
    oldPath: string,
    newName: string,
  ): Promise<FileMetadata>;

  /**
   * Create a new directory
   */
  createDirectory(sourceId: string, path: string): Promise<void>;
}
