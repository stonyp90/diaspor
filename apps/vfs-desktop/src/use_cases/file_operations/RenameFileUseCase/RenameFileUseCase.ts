/**
 * Rename File Use Case
 *
 * Business logic for renaming a file
 */
import { IFileOperations } from '../../../ports/storage/IFileOperations';
import { IEventBus } from '../../../ports/events/IEventBus';
import { FileMetadata } from '../../../domain/entities/FileMetadata';
import { DomainEventType } from '../../../domain/enums/DomainEventType';

export interface RenameFileRequest {
  sourceId: string;
  oldPath: string;
  newName: string;
}

export interface RenameFileResponse {
  success: boolean;
  file?: FileMetadata;
  error?: string;
}

export class RenameFileUseCase {
  constructor(
    private readonly fileOperations: IFileOperations,
    private readonly eventBus: IEventBus,
  ) {}

  async execute(request: RenameFileRequest): Promise<RenameFileResponse> {
    try {
      // Validate sourceId
      if (!request.sourceId || request.sourceId.trim() === '') {
        return { success: false, error: 'No source selected' };
      }

      // Validate oldPath
      if (!request.oldPath || request.oldPath.trim() === '') {
        return { success: false, error: 'Old path cannot be empty' };
      }

      // Validate newName
      if (!request.newName || request.newName.trim() === '') {
        return { success: false, error: 'New name cannot be empty' };
      }

      // Validate: cannot rename root directory
      const normalizedOldPath = request.oldPath.trim();
      if (normalizedOldPath === '/' || normalizedOldPath === '') {
        return { success: false, error: 'Cannot rename root directory' };
      }

      // Validate new name: check for invalid characters
      const trimmedNewName = request.newName.trim();
      if (trimmedNewName === '' || trimmedNewName === '/') {
        return { success: false, error: 'Invalid file name' };
      }

      // Check for invalid characters in new name (platform-specific)
      // eslint-disable-next-line no-control-regex
      const invalidChars = /[<>:"|?*\x00-\x1f]/;
      if (invalidChars.test(trimmedNewName)) {
        return {
          success: false,
          error: `Invalid characters in file name: ${trimmedNewName}`,
        };
      }

      const file = await this.fileOperations.renameFile(
        request.sourceId.trim(),
        normalizedOldPath,
        trimmedNewName,
      );

      // Emit domain event
      this.eventBus.emit({
        type: DomainEventType.FileRenamed,
        payload: {
          sourceId: request.sourceId,
          oldPath: normalizedOldPath,
          newPath: file.path,
        },
        timestamp: new Date().toISOString(),
      });

      return { success: true, file };
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error';
      return { success: false, error: errorMessage };
    }
  }
}
