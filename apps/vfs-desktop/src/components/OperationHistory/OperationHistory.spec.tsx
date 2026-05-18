/**
 * OperationHistory Component Tests
 * Tests for the modern timeline-based OperationHistory
 */
import React from 'react';
import {
  render,
  screen,
  waitFor,
  fireEvent,
  act,
} from '@testing-library/react';
import { OperationHistory } from './OperationHistory';
import * as tauriCore from '@tauri-apps/api/core';

// Mock Tauri API
jest.mock('@tauri-apps/api/core', () => ({
  invoke: jest.fn(),
}));

const mockInvoke = tauriCore.invoke as jest.MockedFunction<
  typeof tauriCore.invoke
>;

describe('OperationHistory', () => {
  const mockOperations = [
    {
      operation_id: 'op1',
      operation_type: 'Upload',
      source_id: 'source1',
      source_path: '/path/to/file1.txt',
      destination_path: '/s3/bucket/file1.txt',
      file_size: 1024 * 1024,
      bytes_processed: 1024 * 1024,
      status: 'Completed',
      created_at: '2024-01-01T10:00:00Z',
      completed_at: '2024-01-01T10:00:05Z',
    },
    {
      operation_id: 'op2',
      operation_type: 'Download',
      source_id: 'source1',
      source_path: '/s3/bucket/file2.txt',
      destination_path: '/local/file2.txt',
      file_size: 2048 * 1024,
      bytes_processed: 1024 * 512,
      status: 'InProgress',
      created_at: '2024-01-01T11:00:00Z',
    },
    {
      operation_id: 'op3',
      operation_type: 'Delete',
      source_id: 'source1',
      source_path: '/path/to/file3.txt',
      file_size: 512 * 1024,
      bytes_processed: 512 * 1024,
      status: 'Failed',
      error: 'Permission denied',
      created_at: '2024-01-01T12:00:00Z',
      completed_at: '2024-01-01T12:00:01Z',
    },
  ];

  beforeEach(() => {
    jest.clearAllMocks();
    mockInvoke.mockResolvedValue(mockOperations);
  });

  describe('Loading and Display', () => {
    it('should show loading state initially', async () => {
      mockInvoke.mockImplementation(
        () =>
          new Promise(() => {
            // Never resolves - intentionally empty
          }),
      );
      render(<OperationHistory />);
      expect(screen.getByText(/Loading/i)).toBeInTheDocument();
    });

    it('should load and display operations', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith('vfs_get_audit_history', {
          limit: 100,
        });
      });

      await waitFor(() => {
        expect(screen.getByText('file1.txt')).toBeInTheDocument();
        expect(screen.getByText('file2.txt')).toBeInTheDocument();
        expect(screen.getByText('file3.txt')).toBeInTheDocument();
      });
    });

    it('should display operation types', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getByText('Upload')).toBeInTheDocument();
        expect(screen.getByText('Download')).toBeInTheDocument();
        expect(screen.getByText('Delete')).toBeInTheDocument();
      });
    });
  });

  describe('Status Indicators', () => {
    it('should show checkmark for completed operations', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getAllByText('✓').length).toBeGreaterThan(0);
      });
    });

    it('should show exclamation for failed operations', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getAllByText('!').length).toBeGreaterThan(0);
      });
    });

    it('should show progress for in-progress operations', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        // 512KB / 2048KB = 25%
        expect(screen.getByText('25%')).toBeInTheDocument();
      });
    });
  });

  describe('Filtering', () => {
    it('should have filter buttons', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getByText('All')).toBeInTheDocument();
        expect(screen.getByText('✓ Done')).toBeInTheDocument();
        expect(screen.getByText('! Failed')).toBeInTheDocument();
      });
    });

    it('should filter to show only completed operations', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getByText('file1.txt')).toBeInTheDocument();
      });

      // Click on Done filter
      const doneFilter = screen.getByText('✓ Done');
      await act(async () => {
        fireEvent.click(doneFilter);
      });

      // Should still show file1.txt (completed)
      expect(screen.getByText('file1.txt')).toBeInTheDocument();
      // Should NOT show file3.txt (failed)
      expect(screen.queryByText('file3.txt')).not.toBeInTheDocument();
    });

    it('should filter to show only failed operations', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getByText('file1.txt')).toBeInTheDocument();
      });

      // Click on Failed filter
      const failedFilter = screen.getByText('! Failed');
      await act(async () => {
        fireEvent.click(failedFilter);
      });

      // Should show file3.txt (failed)
      expect(screen.getByText('file3.txt')).toBeInTheDocument();
      // Should NOT show file1.txt (completed)
      expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
    });
  });

  describe('Expandable Details', () => {
    it('should expand operation details on click', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getByText('file1.txt')).toBeInTheDocument();
      });

      // Click on an operation row
      const row = screen.getByText('file1.txt').closest('.hop__row');
      await act(async () => {
        if (row) fireEvent.click(row);
      });

      // Should show details
      await waitFor(() => {
        expect(screen.getByText('Status')).toBeInTheDocument();
        expect(screen.getByText('Source')).toBeInTheDocument();
      });
    });

    it('should show error in expanded details for failed operations', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getByText('file3.txt')).toBeInTheDocument();
      });

      // Click on failed operation
      const row = screen.getByText('file3.txt').closest('.hop__row');
      await act(async () => {
        if (row) fireEvent.click(row);
      });

      await waitFor(() => {
        expect(screen.getByText('Permission denied')).toBeInTheDocument();
      });
    });
  });

  describe('Delete Operation', () => {
    it('should call delete when clicking delete button', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getByText('file1.txt')).toBeInTheDocument();
      });

      // Find delete button
      const deleteButtons = screen.getAllByTitle('Delete');
      await act(async () => {
        fireEvent.click(deleteButtons[0]);
      });

      expect(mockInvoke).toHaveBeenCalledWith('vfs_delete_operation', {
        operation_id: expect.any(String),
      });
    });
  });

  describe('Refresh', () => {
    it('should have refresh button', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getByTitle('Refresh')).toBeInTheDocument();
      });
    });

    it('should call loadHistory when refresh is clicked', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalled();
      });

      // Clear mock to check new call
      mockInvoke.mockClear();

      const refreshBtn = screen.getByTitle('Refresh');
      await act(async () => {
        fireEvent.click(refreshBtn);
      });

      expect(mockInvoke).toHaveBeenCalledWith('vfs_get_audit_history', {
        limit: 100,
      });
    });
  });

  describe('Empty State', () => {
    it('should show empty state when no operations', async () => {
      mockInvoke.mockResolvedValue([]);

      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getByText('No operations found')).toBeInTheDocument();
      });
    });
  });

  describe('Count Display', () => {
    it('should show count of operations', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        expect(screen.getByText(/3 of 3/)).toBeInTheDocument();
      });
    });
  });

  describe('Sorting', () => {
    it('should sort by timestamp (most recent first)', async () => {
      // op3 has the latest timestamp, op1 has the earliest
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        const items = screen.getAllByText(/file\d\.txt/);
        // file3 (12:00) should come first, then file2 (11:00), then file1 (10:00)
        expect(items[0]).toHaveTextContent('file3.txt');
        expect(items[1]).toHaveTextContent('file2.txt');
        expect(items[2]).toHaveTextContent('file1.txt');
      });
    });
  });

  describe('File Size Display', () => {
    it('should format file sizes correctly', async () => {
      await act(async () => {
        render(<OperationHistory />);
      });

      await waitFor(() => {
        // The formatBytes function formats sizes, check for patterns
        // 1MB, 2MB, 512KB - look for "MB" and "KB" text
        const allText = document.body.textContent || '';
        expect(allText).toContain('MB');
        expect(allText).toContain('KB');
      });
    });
  });
});
