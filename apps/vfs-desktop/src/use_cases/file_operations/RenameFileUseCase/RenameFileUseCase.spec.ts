/**
 * Rename File Use Case Unit Tests
 *
 * Comprehensive test suite for the RenameFileUseCase covering:
 * - Successful operations
 * - Input validation
 * - Error handling
 * - Event emission
 * - Edge cases
 */
import { RenameFileUseCase } from './RenameFileUseCase';
import { IFileOperations } from '../../../ports/storage/IFileOperations';
import { IEventBus } from '../../../ports/events/IEventBus';
import { FileMetadata } from '../../../domain/entities/FileMetadata';
import { FileTierStatus } from '../../../domain/enums/FileTierStatus';
import { DomainEventType } from '../../../domain/enums/DomainEventType';

describe('RenameFileUseCase', () => {
  let useCase: RenameFileUseCase;
  let mockFileOperations: jest.Mocked<IFileOperations>;
  let mockEventBus: jest.Mocked<IEventBus>;

  beforeEach(() => {
    jest.clearAllMocks();

    mockFileOperations = {
      listFiles: jest.fn(),
      getFileMetadata: jest.fn(),
      deleteFiles: jest.fn(),
      renameFile: jest.fn(),
      createDirectory: jest.fn(),
    };

    mockEventBus = {
      emit: jest.fn(),
      subscribe: jest.fn(),
      unsubscribe: jest.fn(),
    };

    useCase = new RenameFileUseCase(mockFileOperations, mockEventBus);
  });

  describe('Successful Operations', () => {
    it('should rename a file successfully', async () => {
      const mockFile: FileMetadata = {
        id: 'file-1',
        name: 'new-name.txt',
        path: '/new-name.txt',
        size: 100,
        lastModified: '2024-01-01T00:00:00Z',
        tierStatus: FileTierStatus.Hot,
        canWarm: false,
        canTranscode: false,
      };

      mockFileOperations.renameFile.mockResolvedValue(mockFile);

      const request = {
        sourceId: 'source-1',
        oldPath: '/old-name.txt',
        newName: 'new-name.txt',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(result.file).toEqual(mockFile);
      expect(mockFileOperations.renameFile).toHaveBeenCalledWith(
        'source-1',
        '/old-name.txt',
        'new-name.txt',
      );
      expect(mockEventBus.emit).toHaveBeenCalledWith(
        expect.objectContaining({
          type: DomainEventType.FileRenamed,
          payload: {
            sourceId: 'source-1',
            oldPath: '/old-name.txt',
            newPath: '/new-name.txt',
          },
        }),
      );
    });

    it('should trim whitespace from sourceId, oldPath, and newName', async () => {
      const mockFile: FileMetadata = {
        id: 'file-1',
        name: 'new-name.txt',
        path: '/new-name.txt',
        size: 100,
        lastModified: '2024-01-01T00:00:00Z',
        tierStatus: FileTierStatus.Hot,
        canWarm: false,
        canTranscode: false,
      };

      mockFileOperations.renameFile.mockResolvedValue(mockFile);

      const request = {
        sourceId: '  source-1  ',
        oldPath: '  /old-name.txt  ',
        newName: '  new-name.txt  ',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockFileOperations.renameFile).toHaveBeenCalledWith(
        'source-1',
        '/old-name.txt',
        'new-name.txt',
      );
    });
  });

  describe('Input Validation', () => {
    it('should return error when sourceId is empty', async () => {
      const request = {
        sourceId: '',
        oldPath: '/file.txt',
        newName: 'new-name.txt',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No source selected');
      expect(mockFileOperations.renameFile).not.toHaveBeenCalled();
    });

    it('should return error when sourceId is whitespace only', async () => {
      const request = {
        sourceId: '   ',
        oldPath: '/file.txt',
        newName: 'new-name.txt',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No source selected');
      expect(mockFileOperations.renameFile).not.toHaveBeenCalled();
    });

    it('should return error when oldPath is empty', async () => {
      const request = {
        sourceId: 'source-1',
        oldPath: '',
        newName: 'new-name.txt',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Old path cannot be empty');
      expect(mockFileOperations.renameFile).not.toHaveBeenCalled();
    });

    it('should return error when newName is empty', async () => {
      const request = {
        sourceId: 'source-1',
        oldPath: '/file.txt',
        newName: '',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('New name cannot be empty');
      expect(mockFileOperations.renameFile).not.toHaveBeenCalled();
    });

    it('should return error when trying to rename root directory', async () => {
      const request = {
        sourceId: 'source-1',
        oldPath: '/',
        newName: 'new-root',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Cannot rename root directory');
      expect(mockFileOperations.renameFile).not.toHaveBeenCalled();
    });

    it('should return error when newName contains invalid characters', async () => {
      const invalidNames = [
        'file<name>.txt',
        'file:name.txt',
        'file|name.txt',
        'file?name.txt',
        'file*name.txt',
      ];

      for (const invalidName of invalidNames) {
        const request = {
          sourceId: 'source-1',
          oldPath: '/file.txt',
          newName: invalidName,
        };

        const result = await useCase.execute(request);

        expect(result.success).toBe(false);
        expect(result.error).toContain('Invalid characters');
        expect(mockFileOperations.renameFile).not.toHaveBeenCalled();
      }
    });

    it('should return error when newName is just a slash', async () => {
      const request = {
        sourceId: 'source-1',
        oldPath: '/file.txt',
        newName: '/',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Invalid file name');
      expect(mockFileOperations.renameFile).not.toHaveBeenCalled();
    });
  });

  describe('Error Handling', () => {
    it('should handle file operations errors', async () => {
      mockFileOperations.renameFile.mockRejectedValue(
        new Error('File not found'),
      );

      const request = {
        sourceId: 'source-1',
        oldPath: '/file.txt',
        newName: 'new-name.txt',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('File not found');
    });

    it('should handle unknown errors', async () => {
      mockFileOperations.renameFile.mockRejectedValue('Unknown error');

      const request = {
        sourceId: 'source-1',
        oldPath: '/file.txt',
        newName: 'new-name.txt',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Unknown error');
    });
  });

  describe('Edge Cases', () => {
    it('should handle very long file names', async () => {
      const longName = 'a'.repeat(255) + '.txt';
      const mockFile: FileMetadata = {
        id: 'file-1',
        name: longName,
        path: `/${longName}`,
        size: 100,
        lastModified: '2024-01-01T00:00:00Z',
        tierStatus: FileTierStatus.Hot,
        canWarm: false,
        canTranscode: false,
      };

      mockFileOperations.renameFile.mockResolvedValue(mockFile);

      const request = {
        sourceId: 'source-1',
        oldPath: '/file.txt',
        newName: longName,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockFileOperations.renameFile).toHaveBeenCalledWith(
        'source-1',
        '/file.txt',
        longName,
      );
    });

    it('should handle unicode characters in file names', async () => {
      const unicodeName = 'файл-测试-ファイル.txt';
      const mockFile: FileMetadata = {
        id: 'file-1',
        name: unicodeName,
        path: `/${unicodeName}`,
        size: 100,
        lastModified: '2024-01-01T00:00:00Z',
        tierStatus: FileTierStatus.Hot,
        canWarm: false,
        canTranscode: false,
      };

      mockFileOperations.renameFile.mockResolvedValue(mockFile);

      const request = {
        sourceId: 'source-1',
        oldPath: '/file.txt',
        newName: unicodeName,
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockFileOperations.renameFile).toHaveBeenCalledWith(
        'source-1',
        '/file.txt',
        unicodeName,
      );
    });
  });
});
