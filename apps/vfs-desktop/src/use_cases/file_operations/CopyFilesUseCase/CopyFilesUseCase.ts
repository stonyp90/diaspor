/**
 * Copy Files Use Case
 *
 * Business logic for copying files to clipboard
 */
import { IClipboardService } from '../../../ports/storage/IClipboardService';
import { IEventBus } from '../../../ports/events/IEventBus';
import { DomainEventType } from '../../../domain/enums/DomainEventType';

export interface CopyFilesRequest {
  sourceId: string;
  paths: string[];
}

export interface CopyFilesResponse {
  success: boolean;
  error?: string;
}

export class CopyFilesUseCase {
  constructor(
    private readonly clipboardService: IClipboardService,
    private readonly eventBus: IEventBus,
  ) {}

  async execute(request: CopyFilesRequest): Promise<CopyFilesResponse> {
    try {
      // Validate sourceId: must be non-empty and not just whitespace
      if (!request.sourceId || !request.sourceId.trim()) {
        return { success: false, error: 'No source selected' };
      }

      // Validate paths: must have at least one file
      if (request.paths.length === 0) {
        return { success: false, error: 'No files to copy' };
      }

      await this.clipboardService.copy(request.sourceId, request.paths);

      // Emit domain event
      this.eventBus.emit({
        type: DomainEventType.FileCreated,
        payload: {
          operation: 'copy',
          sourceId: request.sourceId,
          paths: request.paths,
        },
        timestamp: new Date().toISOString(),
      });

      return { success: true };
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error';
      return { success: false, error: errorMessage };
    }
  }
}
