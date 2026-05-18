/**
 * Paste Files Use Case
 *
 * Business logic for pasting files from clipboard
 */
import { IClipboardService } from '../../../ports/storage/IClipboardService';
import { IEventBus } from '../../../ports/events/IEventBus';
import { DomainEventType } from '../../../domain/enums/DomainEventType';

export interface PasteFilesRequest {
  destSourceId: string;
  destPath: string;
}

export interface PasteFilesResponse {
  success: boolean;
  operationId?: string;
  error?: string;
}

export class PasteFilesUseCase {
  constructor(
    private readonly clipboardService: IClipboardService,
    private readonly eventBus: IEventBus,
  ) {}

  async execute(request: PasteFilesRequest): Promise<PasteFilesResponse> {
    try {
      if (!request.destSourceId) {
        return { success: false, error: 'No destination source selected' };
      }

      // Check if clipboard has files
      const hasFiles = await this.clipboardService.hasFiles();
      if (!hasFiles) {
        return { success: false, error: 'Clipboard is empty' };
      }

      const result = await this.clipboardService.paste(
        request.destSourceId,
        request.destPath,
      );

      // Emit domain event
      this.eventBus.emit({
        type: DomainEventType.OperationStarted,
        payload: {
          operation: 'paste',
          operationId: result.operationId,
          destSourceId: request.destSourceId,
          destPath: request.destPath,
        },
        timestamp: new Date().toISOString(),
      });

      return { success: true, operationId: result.operationId };
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error';
      return { success: false, error: errorMessage };
    }
  }
}
