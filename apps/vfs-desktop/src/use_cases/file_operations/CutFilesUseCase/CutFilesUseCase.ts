/**
 * Cut Files Use Case
 *
 * Business logic for cutting files to clipboard
 */
import { IClipboardService } from '../../../ports/storage/IClipboardService';
import { IEventBus } from '../../../ports/events/IEventBus';
import { DomainEventType } from '../../../domain/enums/DomainEventType';

export interface CutFilesRequest {
  sourceId: string;
  paths: string[];
}

export interface CutFilesResponse {
  success: boolean;
  error?: string;
}

export class CutFilesUseCase {
  constructor(
    private readonly clipboardService: IClipboardService,
    private readonly eventBus: IEventBus,
  ) {}

  async execute(request: CutFilesRequest): Promise<CutFilesResponse> {
    try {
      if (!request.sourceId) {
        return { success: false, error: 'No source selected' };
      }

      if (request.paths.length === 0) {
        return { success: false, error: 'No files to cut' };
      }

      await this.clipboardService.cut(request.sourceId, request.paths);

      // Emit domain event
      this.eventBus.emit({
        type: DomainEventType.FileCreated,
        payload: {
          operation: 'cut',
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
