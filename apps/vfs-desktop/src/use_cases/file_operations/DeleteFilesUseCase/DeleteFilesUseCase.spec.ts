/**
 * Delete Files Use Case Unit Tests
 *
 * Comprehensive test suite for the DeleteFilesUseCase covering:
 * - Successful operations
 * - Input validation
 * - Error handling
 * - Confirmation dialogs
 * - Event emission
 * - Edge cases
 */
import { DeleteFilesUseCase } from './DeleteFilesUseCase';
import { IFileOperations } from '../../../ports/storage/IFileOperations';
import { IDialogService } from '../../../ports/ui/IDialogService';
import { IEventBus } from '../../../ports/events/IEventBus';
import { DomainEventType } from '../../../domain/enums/DomainEventType';
import { DialogType } from '../../../domain/enums/DialogType';

describe('DeleteFilesUseCase', () => {
  let useCase: DeleteFilesUseCase;
  let mockFileOperations: jest.Mocked<IFileOperations>;
  let mockDialogService: jest.Mocked<IDialogService>;
  let mockEventBus: jest.Mocked<IEventBus>;

  beforeEach(() => {
    jest.clearAllMocks();

    mockFileOperations = {
      listFiles: jest.fn(),
      getFileMetadata: jest.fn(),
      deleteFiles: jest.fn().mockResolvedValue(undefined),
      renameFile: jest.fn(),
      createDirectory: jest.fn(),
    };

    mockDialogService = {
      showMessage: jest.fn().mockResolvedValue(undefined),
      showConfirm: jest.fn().mockResolvedValue(true),
      showError: jest.fn().mockResolvedValue(undefined),
      showInfo: jest.fn().mockResolvedValue(undefined),
      showWarning: jest.fn().mockResolvedValue(undefined),
      showOpenDialog: jest.fn().mockResolvedValue(null),
      showSaveDialog: jest.fn().mockResolvedValue(null),
    };

    mockEventBus = {
      emit: jest.fn(),
      subscribe: jest.fn(),
      unsubscribe: jest.fn(),
    };

    useCase = new DeleteFilesUseCase(
      mockFileOperations,
      mockDialogService,
      mockEventBus,
    );
  });

  describe('Successful Operations', () => {
    it('should delete a single file successfully', async () => {
      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
        skipConfirmation: false,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockDialogService.showConfirm).toHaveBeenCalledWith(
        expect.objectContaining({
          title: 'Delete Files',
          message: expect.stringContaining('1 item(s)'),
          type: DialogType.Warning,
        }),
      );
      expect(mockFileOperations.deleteFiles).toHaveBeenCalledWith('source-1', [
        '/file1.txt',
      ]);
      expect(mockEventBus.emit).toHaveBeenCalledWith(
        expect.objectContaining({
          type: DomainEventType.FileDeleted,
          payload: {
            sourceId: 'source-1',
            paths: ['/file1.txt'],
          },
        }),
      );
    });

    it('should delete multiple files successfully', async () => {
      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt', '/file2.txt', '/file3.txt'],
        skipConfirmation: false,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockDialogService.showConfirm).toHaveBeenCalledWith(
        expect.objectContaining({
          message: expect.stringContaining('3 item(s)'),
        }),
      );
      expect(mockFileOperations.deleteFiles).toHaveBeenCalledWith('source-1', [
        '/file1.txt',
        '/file2.txt',
        '/file3.txt',
      ]);
    });

    it('should skip confirmation when skipConfirmation is true', async () => {
      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
        skipConfirmation: true,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockDialogService.showConfirm).not.toHaveBeenCalled();
      expect(mockFileOperations.deleteFiles).toHaveBeenCalled();
    });
  });

  describe('Input Validation', () => {
    it('should return error when sourceId is empty', async () => {
      const request = {
        sourceId: '',
        paths: ['/file1.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No source selected');
      expect(mockFileOperations.deleteFiles).not.toHaveBeenCalled();
    });

    it('should return error when paths array is empty', async () => {
      const request = {
        sourceId: 'source-1',
        paths: [],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No files to delete');
      expect(mockFileOperations.deleteFiles).not.toHaveBeenCalled();
    });
  });

  describe('Confirmation Dialog', () => {
    it('should cancel operation when user rejects confirmation', async () => {
      mockDialogService.showConfirm.mockResolvedValue(false);

      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
        skipConfirmation: false,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Operation cancelled');
      expect(mockFileOperations.deleteFiles).not.toHaveBeenCalled();
      expect(mockEventBus.emit).not.toHaveBeenCalled();
    });

    it('should proceed when user confirms deletion', async () => {
      mockDialogService.showConfirm.mockResolvedValue(true);

      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
        skipConfirmation: false,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockFileOperations.deleteFiles).toHaveBeenCalled();
    });
  });

  describe('Error Handling', () => {
    it('should handle file operations errors', async () => {
      mockFileOperations.deleteFiles.mockRejectedValue(
        new Error('Permission denied'),
      );

      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
        skipConfirmation: true,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Permission denied');
    });

    it('should handle unknown errors', async () => {
      mockFileOperations.deleteFiles.mockRejectedValue('Unknown error');

      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
        skipConfirmation: true,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Unknown error');
    });
  });

  describe('Edge Cases', () => {
    it('should handle deleting many files', async () => {
      const manyPaths = Array.from({ length: 100 }, (_, i) => `/file${i}.txt`);

      const request = {
        sourceId: 'source-1',
        paths: manyPaths,
        skipConfirmation: true,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockFileOperations.deleteFiles).toHaveBeenCalledWith(
        'source-1',
        manyPaths,
      );
    });

    it('should handle nested paths', async () => {
      const request = {
        sourceId: 'source-1',
        paths: ['/folder/subfolder/file.txt'],
        skipConfirmation: true,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockFileOperations.deleteFiles).toHaveBeenCalledWith('source-1', [
        '/folder/subfolder/file.txt',
      ]);
    });
  });
});
