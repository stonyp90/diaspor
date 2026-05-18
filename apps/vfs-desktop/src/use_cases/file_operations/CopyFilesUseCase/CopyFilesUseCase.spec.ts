/**
 * Copy Files Use Case Unit Tests
 *
 * Comprehensive test suite for the CopyFilesUseCase covering:
 * - Successful operations
 * - Input validation
 * - Error handling
 * - Event emission
 * - Edge cases
 */
import { CopyFilesUseCase } from './CopyFilesUseCase';
import { IClipboardService } from '../../../ports/storage/IClipboardService';
import { IEventBus } from '../../../ports/events/IEventBus';
import { DomainEventType } from '../../../domain/enums/DomainEventType';

describe('CopyFilesUseCase', () => {
  let useCase: CopyFilesUseCase;
  let mockClipboardService: jest.Mocked<IClipboardService>;
  let mockEventBus: jest.Mocked<IEventBus>;

  beforeEach(() => {
    // Reset all mocks before each test
    jest.clearAllMocks();

    mockClipboardService = {
      copy: jest.fn().mockResolvedValue(undefined),
      cut: jest.fn(),
      paste: jest.fn(),
      getClipboardContent: jest.fn(),
      hasFiles: jest.fn(),
      clear: jest.fn(),
    };

    mockEventBus = {
      emit: jest.fn(),
      subscribe: jest.fn(),
      unsubscribe: jest.fn(),
    };

    useCase = new CopyFilesUseCase(mockClipboardService, mockEventBus);
  });

  describe('Successful Operations', () => {
    it('should copy a single file successfully', async () => {
      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(result.error).toBeUndefined();
      expect(mockClipboardService.copy).toHaveBeenCalledTimes(1);
      expect(mockClipboardService.copy).toHaveBeenCalledWith(
        request.sourceId,
        request.paths,
      );
    });

    it('should copy multiple files successfully', async () => {
      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt', '/file2.txt', '/file3.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockClipboardService.copy).toHaveBeenCalledWith(
        request.sourceId,
        request.paths,
      );
      expect(mockClipboardService.copy).toHaveBeenCalledTimes(1);
    });

    it('should emit FileCreated event with correct payload', async () => {
      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt', '/file2.txt'],
      };

      await useCase.execute(request);

      expect(mockEventBus.emit).toHaveBeenCalledTimes(1);
      const emittedEvent = mockEventBus.emit.mock.calls[0][0];

      expect(emittedEvent.type).toBe(DomainEventType.FileCreated);
      expect(emittedEvent.payload).toEqual({
        operation: 'copy',
        sourceId: request.sourceId,
        paths: request.paths,
      });
      expect(emittedEvent.timestamp).toBeDefined();
      expect(typeof emittedEvent.timestamp).toBe('string');
    });

    it('should handle files with special characters in paths', async () => {
      const request = {
        sourceId: 'source-1',
        paths: [
          '/file with spaces.txt',
          '/file-with-dashes.txt',
          '/file_with_underscores.txt',
          '/file.with.dots.txt',
        ],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockClipboardService.copy).toHaveBeenCalledWith(
        request.sourceId,
        request.paths,
      );
    });

    it('should handle nested directory paths', async () => {
      const request = {
        sourceId: 'source-1',
        paths: ['/folder1/subfolder/file.txt', '/folder2/file.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockClipboardService.copy).toHaveBeenCalledWith(
        request.sourceId,
        request.paths,
      );
    });
  });

  describe('Input Validation', () => {
    it('should return error when sourceId is empty string', async () => {
      const request = {
        sourceId: '',
        paths: ['/file1.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No source selected');
      expect(mockClipboardService.copy).not.toHaveBeenCalled();
      expect(mockEventBus.emit).not.toHaveBeenCalled();
    });

    it('should return error when sourceId is whitespace only', async () => {
      const request = {
        sourceId: '   ',
        paths: ['/file1.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No source selected');
      expect(mockClipboardService.copy).not.toHaveBeenCalled();
    });

    it('should return error when paths array is empty', async () => {
      const request = {
        sourceId: 'source-1',
        paths: [],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No files to copy');
      expect(mockClipboardService.copy).not.toHaveBeenCalled();
      expect(mockEventBus.emit).not.toHaveBeenCalled();
    });

    it('should handle null sourceId gracefully', async () => {
      const request = {
        sourceId: null as unknown as string,
        paths: ['/file1.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No source selected');
      expect(mockClipboardService.copy).not.toHaveBeenCalled();
    });
  });

  describe('Error Handling', () => {
    it('should handle clipboard service errors', async () => {
      const errorMessage = 'Clipboard service unavailable';
      mockClipboardService.copy.mockRejectedValue(new Error(errorMessage));

      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe(errorMessage);
      expect(mockClipboardService.copy).toHaveBeenCalled();
      expect(mockEventBus.emit).not.toHaveBeenCalled();
    });

    it('should handle non-Error exceptions', async () => {
      mockClipboardService.copy.mockRejectedValue('String error');

      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Unknown error');
    });

    it('should handle network timeout errors', async () => {
      const timeoutError = new Error('Network timeout');
      timeoutError.name = 'TimeoutError';
      mockClipboardService.copy.mockRejectedValue(timeoutError);

      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Network timeout');
    });

    it('should not emit event when clipboard service fails', async () => {
      mockClipboardService.copy.mockRejectedValue(new Error('Service error'));

      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
      };

      await useCase.execute(request);

      expect(mockEventBus.emit).not.toHaveBeenCalled();
    });
  });

  describe('Edge Cases', () => {
    it('should handle very long file paths', async () => {
      const longPath = '/folder/' + 'subfolder/'.repeat(50) + 'file.txt';
      const request = {
        sourceId: 'source-1',
        paths: [longPath],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockClipboardService.copy).toHaveBeenCalledWith(
        request.sourceId,
        request.paths,
      );
    });

    it('should handle paths with unicode characters', async () => {
      const request = {
        sourceId: 'source-1',
        paths: ['/文件.txt', '/файл.txt', '/ファイル.txt'],
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockClipboardService.copy).toHaveBeenCalledWith(
        request.sourceId,
        request.paths,
      );
    });

    it('should handle large number of files', async () => {
      const manyPaths = Array.from({ length: 100 }, (_, i) => `/file${i}.txt`);
      const request = {
        sourceId: 'source-1',
        paths: manyPaths,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockClipboardService.copy).toHaveBeenCalledWith(
        request.sourceId,
        manyPaths,
      );
    });

    it('should preserve path order', async () => {
      const paths = ['/z.txt', '/a.txt', '/m.txt'];
      const request = {
        sourceId: 'source-1',
        paths,
      };

      await useCase.execute(request);

      expect(mockClipboardService.copy).toHaveBeenCalledWith(
        request.sourceId,
        paths, // Should preserve original order
      );
    });
  });

  describe('Integration', () => {
    it('should complete full workflow: validate -> copy -> emit event', async () => {
      const request = {
        sourceId: 'source-1',
        paths: ['/file1.txt'],
      };

      const result = await useCase.execute(request);

      // Verify validation passed
      expect(result.success).toBe(true);

      // Verify clipboard service was called
      expect(mockClipboardService.copy).toHaveBeenCalledTimes(1);
      expect(mockClipboardService.copy).toHaveBeenCalledWith(
        request.sourceId,
        request.paths,
      );

      // Verify event was emitted
      expect(mockEventBus.emit).toHaveBeenCalledTimes(1);
      expect(mockEventBus.emit).toHaveBeenCalledWith(
        expect.objectContaining({
          type: DomainEventType.FileCreated,
          payload: expect.objectContaining({
            operation: 'copy',
            sourceId: request.sourceId,
            paths: request.paths,
          }),
        }),
      );
    });
  });
});
