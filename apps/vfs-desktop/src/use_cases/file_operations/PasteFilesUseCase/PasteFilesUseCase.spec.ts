/**
 * Paste Files Use Case Unit Tests
 *
 * Comprehensive test suite for the PasteFilesUseCase covering:
 * - Successful operations
 * - Input validation
 * - Error handling
 * - Event emission
 * - Edge cases
 */
import { PasteFilesUseCase } from './PasteFilesUseCase';
import { IClipboardService } from '../../../ports/storage/IClipboardService';
import { IEventBus } from '../../../ports/events/IEventBus';
import { DomainEventType } from '../../../domain/enums/DomainEventType';

describe('PasteFilesUseCase', () => {
  let useCase: PasteFilesUseCase;
  let mockClipboardService: jest.Mocked<IClipboardService>;
  let mockEventBus: jest.Mocked<IEventBus>;

  beforeEach(() => {
    jest.clearAllMocks();

    mockClipboardService = {
      copy: jest.fn(),
      cut: jest.fn(),
      paste: jest.fn(),
      getClipboardContent: jest.fn(),
      hasFiles: jest.fn().mockResolvedValue(true),
      clear: jest.fn(),
    };

    mockEventBus = {
      emit: jest.fn(),
      subscribe: jest.fn(),
      unsubscribe: jest.fn(),
    };

    useCase = new PasteFilesUseCase(mockClipboardService, mockEventBus);
  });

  describe('Successful Operations', () => {
    it('should paste files successfully', async () => {
      const operationId = 'op-123';
      mockClipboardService.paste.mockResolvedValue({
        operationId,
      });

      const request = {
        destSourceId: 'dest-source-1',
        destPath: '/destination',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(result.operationId).toBe(operationId);
      expect(mockClipboardService.hasFiles).toHaveBeenCalled();
      expect(mockClipboardService.paste).toHaveBeenCalledWith(
        'dest-source-1',
        '/destination',
      );
      expect(mockEventBus.emit).toHaveBeenCalledWith(
        expect.objectContaining({
          type: DomainEventType.OperationStarted,
          payload: {
            operation: 'paste',
            operationId,
            destSourceId: 'dest-source-1',
            destPath: '/destination',
          },
        }),
      );
    });

    it('should handle root path destination', async () => {
      const operationId = 'op-456';
      mockClipboardService.paste.mockResolvedValue({
        operationId,
      });

      const request = {
        destSourceId: 'dest-source-1',
        destPath: '/',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockClipboardService.paste).toHaveBeenCalledWith(
        'dest-source-1',
        '/',
      );
    });
  });

  describe('Input Validation', () => {
    it('should return error when destSourceId is empty', async () => {
      const request = {
        destSourceId: '',
        destPath: '/destination',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('No destination source selected');
      expect(mockClipboardService.paste).not.toHaveBeenCalled();
    });

    it('should return error when clipboard is empty', async () => {
      mockClipboardService.hasFiles.mockResolvedValue(false);

      const request = {
        destSourceId: 'dest-source-1',
        destPath: '/destination',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Clipboard is empty');
      expect(mockClipboardService.paste).not.toHaveBeenCalled();
    });
  });

  describe('Error Handling', () => {
    it('should handle clipboard service errors', async () => {
      mockClipboardService.paste.mockRejectedValue(
        new Error('Destination not found'),
      );

      const request = {
        destSourceId: 'dest-source-1',
        destPath: '/destination',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Destination not found');
    });

    it('should handle hasFiles check errors', async () => {
      mockClipboardService.hasFiles.mockRejectedValue(
        new Error('Clipboard check failed'),
      );

      const request = {
        destSourceId: 'dest-source-1',
        destPath: '/destination',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Clipboard check failed');
    });

    it('should handle unknown errors', async () => {
      mockClipboardService.paste.mockRejectedValue('Unknown error');

      const request = {
        destSourceId: 'dest-source-1',
        destPath: '/destination',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Unknown error');
    });
  });

  describe('Edge Cases', () => {
    it('should handle empty destPath', async () => {
      const operationId = 'op-789';
      mockClipboardService.paste.mockResolvedValue({
        operationId,
      });

      const request = {
        destSourceId: 'dest-source-1',
        destPath: '',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockClipboardService.paste).toHaveBeenCalledWith(
        'dest-source-1',
        '',
      );
    });

    it('should handle nested destination paths', async () => {
      const operationId = 'op-nested';
      mockClipboardService.paste.mockResolvedValue({
        operationId,
      });

      const request = {
        destSourceId: 'dest-source-1',
        destPath: '/folder/subfolder/nested',
      };

      const result = await useCase.execute(request);

      expect(result.success).toBe(true);
      expect(mockClipboardService.paste).toHaveBeenCalledWith(
        'dest-source-1',
        '/folder/subfolder/nested',
      );
    });
  });
});
