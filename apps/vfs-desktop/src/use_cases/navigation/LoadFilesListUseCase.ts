/**
 * Load Files List Use Case
 *
 * Business logic for loading a list of files from a storage source
 */
import { IFileOperations } from '../../ports/storage/IFileOperations';
import { FileMetadata } from '../../domain/entities/FileMetadata';

export interface LoadFilesListRequest {
  sourceId: string;
  path: string;
}

export interface LoadFilesListResponse {
  success: boolean;
  files?: FileMetadata[];
  error?: string;
}

export class LoadFilesListUseCase {
  constructor(private readonly fileOperations: IFileOperations) {}

  async execute(request: LoadFilesListRequest): Promise<LoadFilesListResponse> {
    try {
      if (!request.sourceId) {
        return { success: false, error: 'No source selected' };
      }

      const files = await this.fileOperations.listFiles(
        request.sourceId,
        request.path,
      );

      return { success: true, files };
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error';
      return { success: false, error: errorMessage };
    }
  }
}
