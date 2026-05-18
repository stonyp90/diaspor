/**
 * OperationsPanel Component Tests
 *
 * Tests for the compact timeline-based operations panel.
 */

import React from 'react';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { OperationsPanel } from './OperationsPanel';
import { invoke } from '@tauri-apps/api/core';

// Mock Tauri API
jest.mock('@tauri-apps/api/core', () => ({
  invoke: jest.fn(),
}));

const mockInvoke = invoke as jest.MockedFunction<typeof invoke>;

describe('OperationsPanel', () => {
  // Helper to create mock operations
  const createMockOperation = (
    operationId: string,
    operationType: string,
    status: string,
    fileCount = 1,
    bytesProcessed = 0,
    totalSize = 1000,
  ) => {
    const now = Date.now();
    // Create files array if fileCount > 1
    const files = fileCount > 1 
      ? Array.from({ length: fileCount }, (_, i) => ({
          local_path: `/local/file${i + 1}.txt`,
          remote_path: `/remote/file${i + 1}.txt`,
          file_size: Math.floor(totalSize / fileCount),
          bytes_processed: Math.floor(bytesProcessed / fileCount),
          status,
        }))
      : undefined;
    
    return {
      operation_id: operationId,
      operation_type: operationType,
      source_id: 'source-1',
      source_path: fileCount > 1 ? '/path/to/folder' : '/path/to/file1.txt',
      destination_path: '/destination',
      file_size: totalSize,
      bytes_processed: bytesProcessed,
      status,
      file_count: fileCount,
      files,
      created_at: now - 1000, // 1 second ago
      completed_at: status === 'Completed' || status === 'Failed' || status === 'Canceled' ? now - 500 : undefined,
    };
  };

  beforeEach(() => {
    jest.clearAllMocks();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'vfs_list_operations') return Promise.resolve([]);
      if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
      if (cmd === 'vfs_list_sources') return Promise.resolve([]);
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    jest.clearAllTimers();
  });

  describe('Panel Visibility', () => {
    it('should not render when no operations are available', async () => {
      await act(async () => {
        render(<OperationsPanel />);
      });
      expect(screen.queryByText(/active|done|Operations/)).not.toBeInTheDocument();
    });

    it('should render when operations are available', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'InProgress', 1, 500, 1000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      await waitFor(() => {
        expect(screen.getByText(/1 active/)).toBeInTheDocument();
      });
    });
  });

  describe('Single File Operations', () => {
    it('should display single file upload operation', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'InProgress', 1, 500, 1000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      await waitFor(() => {
        expect(screen.getByText(/1 active/)).toBeInTheDocument();
      });

      // Expand panel
      const header = screen.getByText(/1 active/);
      await act(async () => {
        fireEvent.click(header);
      });

      await waitFor(() => {
        expect(screen.getByText('file1.txt')).toBeInTheDocument();
        expect(screen.getByText('50%')).toBeInTheDocument();
      });
    });

    it('should show completed status for completed operations', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'Completed', 1, 1000, 1000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      await waitFor(() => {
        expect(screen.getByText(/1 done/)).toBeInTheDocument();
      });

      // Expand panel
      const header = screen.getByText(/1 done/);
      await act(async () => {
        fireEvent.click(header);
      });

      await waitFor(() => {
        expect(screen.getByText('✓')).toBeInTheDocument();
      });
    });

    it('should show failed status for failed operations', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'Failed', 1, 500, 1000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      await waitFor(() => {
        expect(screen.getByText(/1 done/)).toBeInTheDocument();
      });

      // Expand panel
      const header = screen.getByText(/1 done/);
      await act(async () => {
        fireEvent.click(header);
      });

      await waitFor(() => {
        expect(screen.getByText('!')).toBeInTheDocument();
      });
    });
  });

  describe('Multi-File Operations', () => {
    it('should display multi-file operation with item count', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'InProgress', 5, 2500, 5000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      await waitFor(() => {
        expect(screen.getByText(/1 active/)).toBeInTheDocument();
      });

      // Expand panel
      const header = screen.getByText(/1 active/);
      await act(async () => {
        fireEvent.click(header);
      });

      await waitFor(() => {
        expect(screen.getByText('5 items')).toBeInTheDocument();
        expect(screen.getByText('50%')).toBeInTheDocument();
      });
    });
  });

  describe('Expand/Collapse Details', () => {
    it('should expand operation to show details', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'InProgress', 3, 1500, 3000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      // Expand panel first
      await waitFor(() => {
        expect(screen.getByText(/1 active/)).toBeInTheDocument();
      });
      await act(async () => {
        fireEvent.click(screen.getByText(/1 active/));
      });

      await waitFor(() => {
        expect(screen.getByText('3 items')).toBeInTheDocument();
      });

      // Click on operation row to expand details
      await act(async () => {
        fireEvent.click(screen.getByText('3 items'));
      });

      await waitFor(() => {
        expect(screen.getByText('Type:')).toBeInTheDocument();
        expect(screen.getByText('Upload')).toBeInTheDocument();
        expect(screen.getByText('Files:')).toBeInTheDocument();
        expect(screen.getByText('3')).toBeInTheDocument();
      });
    });
  });

  describe('Operation Actions', () => {
    it('should show cancel button for active operations', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'InProgress', 1, 500, 1000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      // Expand panel
      await waitFor(() => {
        expect(screen.getByText(/1 active/)).toBeInTheDocument();
      });
      await act(async () => {
        fireEvent.click(screen.getByText(/1 active/));
      });

      await waitFor(() => {
        expect(screen.getByTitle('Cancel')).toBeInTheDocument();
      });
    });

    it('should show retry button for failed operations', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'Failed', 1, 500, 1000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      // Expand panel
      await waitFor(() => {
        expect(screen.getByText(/1 done/)).toBeInTheDocument();
      });
      await act(async () => {
        fireEvent.click(screen.getByText(/1 done/));
      });

      await waitFor(() => {
        expect(screen.getByTitle('Retry')).toBeInTheDocument();
      });
    });

    it('should show dismiss button for all operations', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'Completed', 1, 1000, 1000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      // Expand panel
      await waitFor(() => {
        expect(screen.getByText(/1 done/)).toBeInTheDocument();
      });
      await act(async () => {
        fireEvent.click(screen.getByText(/1 done/));
      });

      await waitFor(() => {
        expect(screen.getByTitle('Dismiss')).toBeInTheDocument();
      });
    });
  });

  describe('Clear All Functionality', () => {
    it('should show Clear button when there are completed operations', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'Completed', 1, 1000, 1000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      // Expand panel
      await waitFor(() => {
        expect(screen.getByText(/1 done/)).toBeInTheDocument();
      });
      await act(async () => {
        fireEvent.click(screen.getByText(/1 done/));
      });

      await waitFor(() => {
        expect(screen.getByText('Clear')).toBeInTheDocument();
      });
    });
  });

  describe('Error Handling', () => {
    it('should handle API errors gracefully', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.reject(new Error('API Error'));
        return Promise.resolve([]);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      await waitFor(() => {
        expect(screen.queryByText(/active|done/)).not.toBeInTheDocument();
      });
    });
  });

  describe('Operation Type Filtering', () => {
    it('should filter operations by operationTypes prop', async () => {
      const uploadOp = createMockOperation('op-1', 'Upload', 'InProgress');
      const downloadOp = createMockOperation('op-2', 'Download', 'InProgress');
      const deleteOp = createMockOperation('op-3', 'Delete', 'InProgress');

      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([uploadOp, downloadOp, deleteOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel operationTypes={['Upload', 'Download']} />);
      });

      await waitFor(() => {
        // Should show 2 active (Upload + Download, not Delete)
        expect(screen.getByText(/2 active/)).toBeInTheDocument();
      });
    });
  });

  describe('Progress Display', () => {
    it('should show overall progress percentage in header when collapsed', async () => {
      const mockOp = createMockOperation('op-1', 'Upload', 'InProgress', 1, 750, 1000);
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_operations') return Promise.resolve([mockOp]);
        if (cmd === 'vfs_list_uploads') return Promise.resolve([]);
        if (cmd === 'vfs_list_sources') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<OperationsPanel />);
      });

      await waitFor(() => {
        expect(screen.getByText('75%')).toBeInTheDocument();
      });
    });
  });
});
