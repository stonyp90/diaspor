/**
 * Load Sources List Use Case
 *
 * Business logic for loading all storage sources
 */
import { IStorageService } from '../../ports/storage/IStorageService';
import { StorageSource } from '../../domain/entities/StorageSource';

export interface LoadSourcesListResponse {
  success: boolean;
  sources?: StorageSource[];
  error?: string;
}

export class LoadSourcesListUseCase {
  constructor(private readonly storageService: IStorageService) {}

  async execute(): Promise<LoadSourcesListResponse> {
    try {
      const sources = await this.storageService.listSources();

      return { success: true, sources };
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error';
      return { success: false, error: errorMessage };
    }
  }
}
