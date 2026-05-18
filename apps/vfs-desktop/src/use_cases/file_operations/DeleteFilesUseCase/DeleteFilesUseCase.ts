/**
 * Delete Files Use Case
 *
 * Business logic for deleting files
 */
import { IFileOperations } from '../../../ports/storage/IFileOperations';
import { IDialogService } from '../../../ports/ui/IDialogService';
import { IEventBus } from '../../../ports/events/IEventBus';
import { DomainEventType } from '../../../domain/enums/DomainEventType';
import { DialogType } from '../../../domain/enums/DialogType';

export interface DeleteFilesRequest {
  sourceId: string;
  paths: string[];
  skipConfirmation?: boolean;
}

export interface DeleteFilesResponse {
  success: boolean;
  error?: string;
}

export class DeleteFilesUseCase {
  constructor(
    private readonly fileOperations: IFileOperations,
    private readonly dialogService: IDialogService,
    private readonly eventBus: IEventBus,
  ) {}

  async execute(request: DeleteFilesRequest): Promise<DeleteFilesResponse> {
    try {
      if (!request.sourceId) {
        return { success: false, error: 'No source selected' };
      }

      if (request.paths.length === 0) {
        return { success: false, error: 'No files to delete' };
      }

      // Show confirmation dialog unless skipped
      if (!request.skipConfirmation) {
        const confirmed = await this.dialogService.showConfirm({
          title: 'Delete Files',
          message: `Are you sure you want to delete ${request.paths.length} item(s)? This action cannot be undone.`,
          type: DialogType.Warning,
        });

        if (!confirmed) {
          return { success: false, error: 'Operation cancelled' };
        }
      }

      await this.fileOperations.deleteFiles(request.sourceId, request.paths);

      // Emit domain event
      this.eventBus.emit({
        type: DomainEventType.FileDeleted,
        payload: {
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
