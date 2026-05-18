/**
 * TransferPanel Component Tests
 *
 * Tests for the modern timeline-based TransferPanel
 */

import React from 'react';
import {
  render,
  screen,
  fireEvent,
  waitFor,
  act,
} from '@testing-library/react';
import { TransferPanel } from './TransferPanel';
import { invoke } from '@tauri-apps/api/core';

// Mock Tauri API
jest.mock('@tauri-apps/api/core', () => ({
  invoke: jest.fn(),
}));

const mockInvoke = invoke as jest.MockedFunction<typeof invoke>;

// Mock operation data
const createMockOperation = (overrides = {}) => ({
  operation_id: `op-${Math.random().toString(36).substr(2, 9)}`,
  operation_type: 'Copy',
  source_id: 'local-1',
  source_path: '/Users/test/file.txt',
  bytes_processed: 0,
  status: 'InProgress' as const,
  created_at: Date.now() / 1000,
  ...overrides,
});

describe('TransferPanel', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockInvoke.mockResolvedValue([]);
  });

  describe('visibility', () => {
    it('should not render when isVisible is false', () => {
      render(<TransferPanel isVisible={false} />);
      expect(screen.queryByText('Operations')).not.toBeInTheDocument();
    });

    it('should render when isVisible is true', async () => {
      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });
      expect(screen.getByText('Operations')).toBeInTheDocument();
    });
  });

  describe('tabs', () => {
    it('should show Active and All tabs', async () => {
      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });
      expect(screen.getByText(/Active/)).toBeInTheDocument();
      expect(screen.getByText(/All/)).toBeInTheDocument();
    });

    it('should switch between tabs', async () => {
      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      const allTab = screen.getByText(/All/);
      await act(async () => {
        fireEvent.click(allTab);
      });
      expect(allTab).toHaveClass('xfer__tab--on');
    });
  });

  describe('operations display', () => {
    it('should display empty state when no operations', async () => {
      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });
      expect(screen.getByText('No active operations')).toBeInTheDocument();
    });

    it('should display operations from backend', async () => {
      const ops = [
        createMockOperation({ source_path: '/test/document.pdf' }),
        createMockOperation({ source_path: '/test/image.png' }),
      ];
      mockInvoke.mockResolvedValue(ops);

      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      await waitFor(() => {
        expect(screen.getByText('document.pdf')).toBeInTheDocument();
        expect(screen.getByText('image.png')).toBeInTheDocument();
      });
    });

    it('should show operation progress percentage', async () => {
      const ops = [
        createMockOperation({
          source_path: '/test/file.txt',
          file_size: 1000,
          bytes_processed: 500,
          status: 'InProgress',
        }),
      ];
      mockInvoke.mockResolvedValue(ops);

      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      await waitFor(() => {
        expect(screen.getByText('50%')).toBeInTheDocument();
      });
    });

    it('should show completed checkmark for finished operations', async () => {
      const ops = [
        createMockOperation({
          status: 'Completed',
          completed_at: Date.now() / 1000,
        }),
      ];
      mockInvoke.mockResolvedValue(ops);

      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      await waitFor(() => {
        expect(screen.getByText('✓')).toBeInTheDocument();
      });
    });

    it('should show failed indicator for failed operations', async () => {
      const ops = [
        createMockOperation({
          status: 'Failed',
          error: 'Connection timeout',
          completed_at: Date.now() / 1000,
        }),
      ];
      mockInvoke.mockResolvedValue(ops);

      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      await waitFor(() => {
        expect(screen.getByText('!')).toBeInTheDocument();
      });
    });
  });

  describe('expandable details', () => {
    it('should expand operation details on click', async () => {
      const ops = [
        createMockOperation({
          source_path: '/test/file.txt',
          file_size: 1024,
          bytes_processed: 512,
          status: 'InProgress',
        }),
      ];
      mockInvoke.mockResolvedValue(ops);

      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      await waitFor(() => {
        expect(screen.getByText('file.txt')).toBeInTheDocument();
      });

      // Click on the operation row to expand
      const row = screen.getByText('file.txt').closest('.xop__row');
      await act(async () => {
        if (row) fireEvent.click(row);
      });

      // Should show expanded details
      await waitFor(() => {
        expect(screen.getByText('Status')).toBeInTheDocument();
      });
    });

    it('should show error in expanded details for failed operations', async () => {
      const ops = [
        createMockOperation({
          status: 'Failed',
          error: 'Disk full',
          completed_at: Date.now() / 1000,
        }),
      ];
      mockInvoke.mockResolvedValue(ops);

      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      await waitFor(() => {
        expect(screen.getByText('file.txt')).toBeInTheDocument();
      });

      // Expand the operation
      const row = screen.getByText('file.txt').closest('.xop__row');
      await act(async () => {
        if (row) fireEvent.click(row);
      });

      await waitFor(() => {
        expect(screen.getByText('Disk full')).toBeInTheDocument();
      });
    });
  });

  describe('dismiss operations', () => {
    it('should dismiss operation on close button click', async () => {
      const ops = [
        createMockOperation({
          operation_id: 'test-op-1',
          status: 'Completed',
          completed_at: Date.now() / 1000,
        }),
      ];
      mockInvoke.mockResolvedValue(ops);

      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      await waitFor(() => {
        expect(screen.getByText('file.txt')).toBeInTheDocument();
      });

      // Click dismiss button
      const dismissBtn = screen.getByTitle('Dismiss');
      await act(async () => {
        fireEvent.click(dismissBtn);
      });

      expect(mockInvoke).toHaveBeenCalledWith('vfs_delete_operation', {
        operationId: 'test-op-1',
      });
    });

    it('should clear all completed operations', async () => {
      const ops = [
        createMockOperation({
          operation_id: 'op-1',
          status: 'Completed',
          completed_at: Date.now() / 1000,
        }),
        createMockOperation({
          operation_id: 'op-2',
          status: 'Completed',
          completed_at: Date.now() / 1000,
        }),
      ];
      mockInvoke.mockResolvedValue(ops);

      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      await waitFor(() => {
        expect(screen.getByText('Clear All')).toBeInTheDocument();
      });

      const clearBtn = screen.getByText('Clear All');
      await act(async () => {
        fireEvent.click(clearBtn);
      });

      // Should call delete for each completed operation
      expect(mockInvoke).toHaveBeenCalledWith('vfs_delete_operation', {
        operationId: 'op-1',
      });
      expect(mockInvoke).toHaveBeenCalledWith('vfs_delete_operation', {
        operationId: 'op-2',
      });
    });
  });

  describe('close button', () => {
    it('should call onClose when close button is clicked', async () => {
      const onClose = jest.fn();
      await act(async () => {
        render(<TransferPanel isVisible={true} onClose={onClose} />);
      });

      const closeBtn = screen.getByText('×');
      await act(async () => {
        fireEvent.click(closeBtn);
      });

      expect(onClose).toHaveBeenCalled();
    });
  });

  describe('badge', () => {
    it('should show active count badge when there are active operations', async () => {
      const ops = [
        createMockOperation({ status: 'InProgress' }),
        createMockOperation({ status: 'Pending' }),
      ];
      mockInvoke.mockResolvedValue(ops);

      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      await waitFor(() => {
        expect(screen.getByText('2')).toBeInTheDocument();
      });
    });
  });

  describe('sorting', () => {
    it('should sort operations by timestamp (most recent first)', async () => {
      const now = Date.now() / 1000;
      const ops = [
        createMockOperation({
          source_path: '/old.txt',
          created_at: now - 3600,
        }),
        createMockOperation({ source_path: '/new.txt', created_at: now }),
        createMockOperation({
          source_path: '/middle.txt',
          created_at: now - 1800,
        }),
      ];
      mockInvoke.mockResolvedValue(ops);

      await act(async () => {
        render(<TransferPanel isVisible={true} />);
      });

      await waitFor(() => {
        const items = screen.getAllByText(/\.txt$/);
        // Most recent first
        expect(items[0]).toHaveTextContent('new.txt');
        expect(items[1]).toHaveTextContent('middle.txt');
        expect(items[2]).toHaveTextContent('old.txt');
      });
    });
  });

  describe('operation types', () => {
    it.each([
      ['Upload', '↑'],
      ['Download', '↓'],
      ['Copy', '⧉'],
      ['Move', '→'],
      ['Delete', '×'],
      ['Rename', '✎'],
    ])(
      'should show correct icon for %s operation',
      async (opType, expectedIcon) => {
        const ops = [createMockOperation({ operation_type: opType })];
        mockInvoke.mockResolvedValue(ops);

        await act(async () => {
          render(<TransferPanel isVisible={true} />);
        });

        await waitFor(() => {
          expect(screen.getByText(expectedIcon)).toBeInTheDocument();
        });
      },
    );
  });
});
