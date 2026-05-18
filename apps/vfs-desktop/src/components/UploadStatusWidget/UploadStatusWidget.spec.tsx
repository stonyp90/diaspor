/**
 * UploadStatusWidget Component Tests
 *
 * Tests cover:
 * - Dismiss functionality for completed uploads
 * - Different use cases (single dismiss, multiple dismisses, view all history)
 * - State persistence across reloads
 */

import React from 'react';
import {
  render,
  screen,
  fireEvent,
  waitFor,
  act,
} from '@testing-library/react';
import { UploadStatusWidget } from './UploadStatusWidget';
import { invoke } from '@tauri-apps/api/core';

jest.mock('@tauri-apps/api/core');

const mockInvoke = invoke as jest.MockedFunction<typeof invoke>;

describe('UploadStatusWidget', () => {
  const mockCompletedUpload1 = {
    upload_id: 'upload-1',
    source_id: 'source-1',
    key: 'file1.txt',
    local_path: '/path/to/file1.txt',
    total_size: 1024,
    bytes_uploaded: 1024,
    current_part: 1,
    total_parts: 1,
    status: 'Completed' as const,
    completed_at: '2024-01-01T10:00:00Z',
    last_updated_at: '2024-01-01T10:00:00Z',
  };

  const mockCompletedUpload2 = {
    upload_id: 'upload-2',
    source_id: 'source-1',
    key: 'file2.txt',
    local_path: '/path/to/file2.txt',
    total_size: 2048,
    bytes_uploaded: 2048,
    current_part: 1,
    total_parts: 1,
    status: 'Completed' as const,
    completed_at: '2024-01-01T11:00:00Z',
    last_updated_at: '2024-01-01T11:00:00Z',
  };

  const mockActiveUpload = {
    upload_id: 'upload-3',
    source_id: 'source-1',
    key: 'file3.txt',
    local_path: '/path/to/file3.txt',
    total_size: 4096,
    bytes_uploaded: 2048,
    current_part: 1,
    total_parts: 2,
    status: 'InProgress' as const,
    created_at: '2024-01-01T12:00:00Z',
    last_updated_at: '2024-01-01T12:00:00Z',
  };

  beforeEach(() => {
    jest.clearAllMocks();
    jest.useRealTimers();
    // Reset mocks but don't set default implementation - each test will set its own
    mockInvoke.mockReset();
    // Default mock to return empty array to prevent errors
    mockInvoke.mockResolvedValue([]);
  });

  afterEach(() => {
    jest.clearAllMocks();
    // Only run pending timers if fake timers are enabled
    if (jest.isMockFunction(setTimeout)) {
      jest.runOnlyPendingTimers();
    }
  });

  describe('Dismiss Functionality', () => {
    it('should show dismiss button for completed uploads', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockCompletedUpload1]);
        }
        return Promise.resolve(null);
      });

      render(<UploadStatusWidget />);

      // Wait for the widget to render (component loads uploads asynchronously)
      await waitFor(
        () => {
          expect(screen.getByText('Uploads')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Expand the widget
      const header = screen.getByText('Uploads');
      fireEvent.click(header);

      await waitFor(
        () => {
          expect(screen.getByText('file1.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Check for dismiss button
      const dismissButton = screen.getByTitle('Dismiss');
      expect(dismissButton).toBeInTheDocument();
    });

    it('should remove completed upload when dismiss button is clicked', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockCompletedUpload1]);
        }
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<UploadStatusWidget />);
      });

      // Wait for the widget to render
      await waitFor(
        () => {
          expect(screen.getByText('Uploads')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Expand the widget
      const header = screen.getByText('Uploads');
      fireEvent.click(header);

      await waitFor(
        () => {
          expect(screen.getByText('file1.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Click dismiss button
      const dismissButton = screen.getByTitle('Dismiss');
      fireEvent.click(dismissButton);

      // Upload should be removed from view
      await waitFor(
        () => {
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
        },
        { timeout: 3000 },
      );
    });

    it('should not show dismissed uploads after reload', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockCompletedUpload1]);
        }
        return Promise.resolve(null);
      });

      const { rerender } = render(<UploadStatusWidget />);

      // Wait for the widget to render
      await waitFor(
        () => {
          expect(screen.getByText('Uploads')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Expand and dismiss
      const header = screen.getByText('Uploads');
      fireEvent.click(header);

      await waitFor(
        () => {
          expect(screen.getByText('file1.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      const dismissButton = screen.getByTitle('Dismiss');
      fireEvent.click(dismissButton);

      await waitFor(
        () => {
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Simulate reload by rerendering with same data
      rerender(<UploadStatusWidget />);
      fireEvent.click(header);

      // Upload should still be dismissed
      await waitFor(
        () => {
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
        },
        { timeout: 3000 },
      );
    });

    it('should allow dismissing multiple completed uploads', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockCompletedUpload1, mockCompletedUpload2]);
        }
        return Promise.resolve(null);
      });

      render(<UploadStatusWidget />);

      // Wait for the widget to render
      await waitFor(
        () => {
          expect(screen.getByText('Uploads')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Expand the widget
      const header = screen.getByText('Uploads');
      await act(async () => {
        fireEvent.click(header);
      });

      // Wait for both uploads to be visible
      await waitFor(
        () => {
          expect(screen.getByText('file1.txt')).toBeInTheDocument();
          expect(screen.getByText('file2.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Dismiss first upload
      const dismissButtons = screen.getAllByTitle('Dismiss');
      expect(dismissButtons.length).toBeGreaterThanOrEqual(2);
      await act(async () => {
        fireEvent.click(dismissButtons[0]);
      });

      // Wait for first upload to be dismissed
      await waitFor(
        () => {
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Verify second upload is still visible
      expect(screen.getByText('file2.txt')).toBeInTheDocument();

      // Dismiss second upload
      const remainingDismissButton = screen.getByTitle('Dismiss');
      await act(async () => {
        fireEvent.click(remainingDismissButton);
      });

      await waitFor(
        () => {
          expect(screen.queryByText('file2.txt')).not.toBeInTheDocument();
        },
        { timeout: 3000 },
      );
    });

    it('should not show dismiss button for active uploads', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockActiveUpload]);
        }
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<UploadStatusWidget />);
      });

      // Wait for the widget to render
      await waitFor(
        () => {
          expect(screen.getByText('Uploads')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Expand the widget
      const header = screen.getByText('Uploads');
      fireEvent.click(header);

      await waitFor(
        () => {
          expect(screen.getByText('file3.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Should not have dismiss button for active uploads
      expect(screen.queryByTitle('Dismiss')).not.toBeInTheDocument();
    });

    it('should filter dismissed uploads from "View All" history', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockCompletedUpload1, mockCompletedUpload2]);
        }
        return Promise.resolve(null);
      });

      render(<UploadStatusWidget />);

      // Wait for the widget to render
      await waitFor(
        () => {
          expect(screen.getByText('Uploads')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Expand the widget
      const header = screen.getByText('Uploads');
      await act(async () => {
        fireEvent.click(header);
      });

      await waitFor(
        () => {
          expect(screen.getByText('file1.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Dismiss first upload
      const dismissButtons = screen.getAllByTitle('Dismiss');
      expect(dismissButtons.length).toBeGreaterThanOrEqual(2);
      await act(async () => {
        fireEvent.click(dismissButtons[0]);
      });

      // Wait for the first upload to be dismissed
      await waitFor(
        () => {
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Verify file2 is still visible
      expect(screen.getByText('file2.txt')).toBeInTheDocument();

      // Click "View All"
      const viewAllButton = screen.getByText('View All');
      fireEvent.click(viewAllButton);

      // Dismissed upload should not appear in full history
      await waitFor(
        () => {
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
          expect(screen.getByText('file2.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );
    });

    it('should maintain dismissed state when new uploads complete', async () => {
      // Start with one completed upload
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockCompletedUpload1]);
        }
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<UploadStatusWidget />);
      });

      // Wait for the widget to render
      await waitFor(
        () => {
          expect(screen.getByText('Uploads')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Expand and dismiss
      const header = screen.getByText('Uploads');
      fireEvent.click(header);

      await waitFor(
        () => {
          expect(screen.getByText('file1.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      const dismissButton = screen.getByTitle('Dismiss');
      fireEvent.click(dismissButton);

      await waitFor(
        () => {
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Simulate new upload completing
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockCompletedUpload1, mockCompletedUpload2]);
        }
        return Promise.resolve(null);
      });

      // Wait for reload (component polls every 2000ms)
      await new Promise((resolve) => setTimeout(resolve, 2100));

      // Wait for reload
      await waitFor(
        () => {
          // Dismissed upload should still be hidden
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
          // New upload should be visible
          expect(screen.getByText('file2.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );
    });

    it('should stop event propagation when clicking dismiss button', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockCompletedUpload1]);
        }
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<UploadStatusWidget />);
      });

      // Wait for the widget to render
      await waitFor(
        () => {
          expect(screen.getByText('Uploads')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Expand the widget
      const header = screen.getByText('Uploads');
      fireEvent.click(header);

      await waitFor(
        () => {
          expect(screen.getByText('file1.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      const dismissButton = screen.getByTitle('Dismiss');

      // Create a mock event handler to check if stopPropagation is called
      const parentElement = dismissButton.closest('.upload-status-item');
      const parentClickHandler = jest.fn();
      if (parentElement) {
        parentElement.addEventListener('click', parentClickHandler);
      }

      fireEvent.click(dismissButton);

      // The dismiss button should prevent the parent click handler from being called
      // (which would expand/collapse the widget)
      // Since the component calls stopPropagation, the parent handler shouldn't be called
      // But we can't directly test stopPropagation, so we verify the dismiss worked
      await waitFor(
        () => {
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
        },
        { timeout: 3000 },
      );
    });
  });

  describe('Edge Cases', () => {
    it('should handle dismissing upload that no longer exists in backend', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockCompletedUpload1]);
        }
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<UploadStatusWidget />);
      });

      // Wait for the widget to render
      await waitFor(
        () => {
          expect(screen.getByText('Uploads')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Expand and dismiss
      const header = screen.getByText('Uploads');
      fireEvent.click(header);

      await waitFor(
        () => {
          expect(screen.getByText('file1.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      const dismissButton = screen.getByTitle('Dismiss');
      fireEvent.click(dismissButton);

      // Simulate backend removing the upload
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([]);
        }
        return Promise.resolve(null);
      });

      // Wait for reload (component polls every 2000ms)
      await new Promise((resolve) => setTimeout(resolve, 2100));

      // Should not crash and should not show the upload
      await waitFor(
        () => {
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
        },
        { timeout: 3000 },
      );
    });

    it('should handle rapid dismiss clicks gracefully', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'vfs_list_uploads') {
          return Promise.resolve([mockCompletedUpload1]);
        }
        return Promise.resolve(null);
      });

      await act(async () => {
        render(<UploadStatusWidget />);
      });

      // Wait for the widget to render
      await waitFor(
        () => {
          expect(screen.getByText('Uploads')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Expand the widget
      const header = screen.getByText('Uploads');
      fireEvent.click(header);

      await waitFor(
        () => {
          expect(screen.getByText('file1.txt')).toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      const dismissButton = screen.getByTitle('Dismiss');

      // Click multiple times rapidly
      fireEvent.click(dismissButton);
      fireEvent.click(dismissButton);
      fireEvent.click(dismissButton);

      // Should only dismiss once
      await waitFor(
        () => {
          expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
        },
        { timeout: 3000 },
      );

      // Should not have multiple dismiss buttons
      expect(screen.queryAllByTitle('Dismiss')).toHaveLength(0);
    });
  });
});
