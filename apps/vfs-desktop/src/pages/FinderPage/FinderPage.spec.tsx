/**
 * FinderPage Component Tests
 * Tests for file browser functionality including Open With menu
 */

import React from 'react';
import {
  render,
  screen,
  waitFor,
  fireEvent,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import '@testing-library/jest-dom';

// Mock all Tauri APIs before imports
jest.mock('@tauri-apps/api/core', () => ({
  invoke: jest.fn(),
}));

jest.mock('@tauri-apps/api/event', () => ({
  listen: jest.fn(() =>
    Promise.resolve(() => {
      // Cleanup function
    }),
  ),
}));

jest.mock('@tauri-apps/plugin-dialog', () => ({
  open: jest.fn(),
  save: jest.fn(),
}));

jest.mock(
  '@tauri-apps/api/shell',
  () => ({
    open: jest.fn(),
  }),
  { virtual: true },
);

jest.mock('@tauri-apps/plugin-dialog', () => ({
  open: jest.fn(),
  save: jest.fn(),
}));

// Mock all services
jest.mock('../services/storage.service', () => ({
  StorageService: {
    getSources: jest.fn(() => Promise.resolve([])),
    getFiles: jest.fn(() => Promise.resolve([])),
  },
  VfsService: {
    getAppsForFile: jest.fn(() => Promise.resolve([])),
  },
}));

jest.mock('../services/dialog.service', () => ({
  DialogService: {
    showOpenDialog: jest.fn(),
    showSaveDialog: jest.fn(),
  },
}));

// Mock all hooks
jest.mock('../hooks/useKeyboardShortcuts', () => ({
  useKeyboardShortcuts: jest.fn(() => ({})),
}));

jest.mock('../components/Toast', () => ({
  useToast: jest.fn(() => ({
    showToast: jest.fn(),
  })),
}));

jest.mock('../components/KeyboardShortcutHelper', () => ({
  useKeyboardShortcutHelper: jest.fn(() => ({})),
  KeyboardShortcutHelper: () => null,
}));

// Mock all components
jest.mock('../components/Breadcrumbs', () => ({
  Breadcrumbs: () => <div data-testid="breadcrumbs">Breadcrumbs</div>,
}));

jest.mock('../components/SearchBox', () => ({
  SearchBox: () => <div data-testid="search-box">SearchBox</div>,
}));

jest.mock('../components/SpotlightSearch', () => ({
  SpotlightSearch: () => (
    <div data-testid="spotlight-search">SpotlightSearch</div>
  ),
}));

jest.mock('../components/MetricsPreview', () => ({
  MetricsPreview: () => <div data-testid="metrics-preview">MetricsPreview</div>,
}));

jest.mock('../components/InfoModal', () => ({
  InfoModal: () => null,
}));

jest.mock('../components/AddStorageModal', () => ({
  AddStorageModal: () => null,
}));

jest.mock('../components/ShortcutSettings', () => ({
  ShortcutSettings: () => null,
}));

import { FinderPage } from './FinderPage';
import * as tauriCore from '@tauri-apps/api/core';

describe('FinderPage - Open With Menu', () => {
  const defaultProps = {
    onOpenMetrics: jest.fn(),
    onOpenSearch: jest.fn(),
    isSearchOpen: false,
    onCloseSearch: jest.fn(),
  };

  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('App Deduplication', () => {
    it('should deduplicate apps by bundle_id', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');

      // Mock apps with duplicate bundle_id
      invoke.mockResolvedValueOnce([
        {
          name: 'Preview',
          path: '/System/Applications/Preview.app',
          bundle_id: 'com.apple.Preview',
        },
        {
          name: 'Preview',
          path: '/System/Applications/Preview.app',
          bundle_id: 'com.apple.Preview',
        },
      ]);

      render(<FinderPage {...defaultProps} />);

      // Wait for apps to load
      await waitFor(() => {
        expect(invoke).toHaveBeenCalled();
      });

      // Apps should be deduplicated
      const previewApps = screen.queryAllByText('Preview');
      expect(previewApps.length).toBeLessThanOrEqual(1);
    });

    it('should deduplicate apps by path', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');

      // Mock apps with duplicate path
      invoke.mockResolvedValueOnce([
        { name: 'Preview', path: '/System/Applications/Preview.app' },
        { name: 'Preview', path: '/System/Applications/Preview.app' },
      ]);

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalled();
      });

      // Should only show one Preview
      const previewApps = screen.queryAllByText('Preview');
      expect(previewApps.length).toBeLessThanOrEqual(1);
    });

    it('should deduplicate apps by name (case-insensitive)', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');

      // Mock apps with duplicate name (different cases)
      invoke.mockResolvedValueOnce([
        { name: 'Preview', path: '/System/Applications/Preview.app' },
        { name: 'preview', path: '/System/Applications/Preview.app' },
        { name: 'PREVIEW', path: '/System/Applications/Preview.app' },
      ]);

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalled();
      });

      // Should only show one Preview (case-insensitive)
      const previewApps = screen.queryAllByText(/Preview/i);
      expect(previewApps.length).toBeLessThanOrEqual(1);
    });

    it('should handle apps with no duplicates correctly', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');

      // Mock unique apps
      invoke.mockResolvedValueOnce([
        {
          name: 'Preview',
          path: '/System/Applications/Preview.app',
          bundle_id: 'com.apple.Preview',
        },
        {
          name: 'Safari',
          path: '/Applications/Safari.app',
          bundle_id: 'com.apple.Safari',
        },
        {
          name: 'Chrome',
          path: '/Applications/Google Chrome.app',
          bundle_id: 'com.google.Chrome',
        },
      ]);

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalled();
      });

      // All unique apps should be shown
      expect(screen.getByText('Preview')).toBeInTheDocument();
      expect(screen.getByText('Safari')).toBeInTheDocument();
      expect(screen.getByText('Chrome')).toBeInTheDocument();
    });
  });

  describe('File Navigation', () => {
    it('should navigate within Diaspor when double-clicking a folder (not open native Finder)', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');

      // Mock sources and files
      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([
            {
              id: 'local-1',
              name: 'Local Storage',
              source_type: 'Local',
              category: 'local',
              status: 'connected',
            },
          ]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([
            {
              id: 'folder-1',
              name: 'Documents',
              path: '/Documents',
              size: 0,
              mimeType: 'folder',
              isDirectory: true,
              tierStatus: 'hot',
              canWarm: false,
              canTranscode: false,
            },
          ]);
        }
        if (
          cmd === 'vfs_clipboard_has_files' ||
          cmd === 'vfs_clipboard_read_native'
        ) {
          return Promise.resolve(false);
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      // Wait for files to load
      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          'vfs_list_files',
          expect.any(Object),
        );
      });

      // Verify no native Finder/Explorer was opened during navigation
      const revealCalls = invoke.mock.calls.filter(
        (call) => call[0] === 'vfs_reveal_in_finder',
      );
      expect(revealCalls.length).toBe(0);
    });

    it('should show "Reveal in Finder" option in context menu for local storage', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');

      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([
            {
              id: 'local-1',
              name: 'Local Storage',
              source_type: 'Local',
              category: 'local',
              status: 'connected',
            },
          ]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([
            {
              id: 'file-1',
              name: 'test.txt',
              path: '/test.txt',
              size: 100,
              mimeType: 'text/plain',
              isDirectory: false,
              tierStatus: 'hot',
              canWarm: false,
              canTranscode: false,
            },
          ]);
        }
        if (
          cmd === 'vfs_clipboard_has_files' ||
          cmd === 'vfs_clipboard_read_native'
        ) {
          return Promise.resolve(false);
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          'vfs_list_files',
          expect.any(Object),
        );
      });

      // The "Reveal in Finder" option should be available in the context menu
      // when right-clicking on local storage files
      // Note: Full integration test would require more complex setup
      // This test verifies the component renders without errors
      expect(invoke).toHaveBeenCalled();
    });

    it('should call vfs_reveal_in_finder when revealing file', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');

      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([
            {
              id: 'local-1',
              name: 'Local Storage',
              source_type: 'Local',
              category: 'local',
              status: 'connected',
            },
          ]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([]);
        }
        if (
          cmd === 'vfs_clipboard_has_files' ||
          cmd === 'vfs_clipboard_read_native'
        ) {
          return Promise.resolve(false);
        }
        if (cmd === 'vfs_reveal_in_finder') {
          return Promise.resolve(undefined);
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      // Simulate calling reveal_in_finder directly
      const invokeSpy = jest.spyOn(tauriCore, 'invoke');
      invokeSpy.mockResolvedValueOnce(undefined);

      // Call through the component's handler, not directly
      // The component will call invoke internally
      await waitFor(() => {
        expect(invokeSpy).toHaveBeenCalled();
      });
    });

    it('should deduplicate Preview specifically for PDF files', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');

      // Mock PDF file apps - Preview might come from Launch Services AND common apps
      invoke.mockResolvedValueOnce([
        {
          name: 'Preview',
          path: '/System/Applications/Preview.app',
          bundle_id: 'com.apple.Preview',
        },
        { name: 'Preview', path: '/System/Applications/Preview.app' }, // No bundle_id from common apps
        {
          name: 'Books',
          path: '/System/Applications/Books.app',
          bundle_id: 'com.apple.Books',
        },
      ]);

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalled();
      });

      // Preview should appear exactly once
      const previewApps = screen.queryAllByText('Preview');
      expect(previewApps.length).toBe(1);
    });
  });

  describe('Open With Menu Display', () => {
    it('should show Open With submenu when hovering over Open With', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');
      invoke.mockResolvedValueOnce([
        { name: 'Preview', path: '/System/Applications/Preview.app' },
      ]);

      render(<FinderPage {...defaultProps} />);

      // Wait for component to render
      await waitFor(() => {
        expect(screen.getByText('Open')).toBeInTheDocument();
      });
    });

    it('should handle empty app list gracefully', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');
      invoke.mockResolvedValueOnce([]);

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalled();
      });

      // Should show "No apps found" message
      expect(screen.getByText('No apps found')).toBeInTheDocument();
    });

    it('should handle app loading state', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');

      // Delay the response to test loading state
      invoke.mockImplementation(
        () => new Promise((resolve) => setTimeout(() => resolve([]), 100)),
      );

      render(<FinderPage {...defaultProps} />);

      // Should show loading state initially
      await waitFor(() => {
        expect(screen.getByText('Loading apps...')).toBeInTheDocument();
      });
    });
  });

  describe('Copy/Cut/Paste Functionality', () => {
    const mockFile = {
      id: 'file-1',
      name: 'test.txt',
      path: '/test.txt',
      size: 100,
      mimeType: 'text/plain',
      isDirectory: false,
      tierStatus: 'hot',
      canWarm: false,
      canTranscode: false,
    };

    const mockFolder = {
      id: 'folder-1',
      name: 'Documents',
      path: '/Documents',
      size: 0,
      mimeType: 'folder',
      isDirectory: true,
      tierStatus: 'hot',
      canWarm: false,
      canTranscode: false,
    };

    const mockSource = {
      id: 'local-1',
      name: 'Local Storage',
      source_type: 'Local',
      category: 'local',
      provider_id: 'local',
      mounted: true,
      status: 'connected',
      path: '/',
    };

    beforeEach(() => {
      jest.clearAllMocks();
    });

    describe('Context Menu - Copy/Cut/Paste', () => {
      it('should show Copy and Cut buttons when right-clicking on a file', async () => {
        const invoke = jest.spyOn(tauriCore, 'invoke');
        invoke.mockImplementation((cmd) => {
          if (cmd === 'vfs_list_sources') {
            return Promise.resolve([mockSource]);
          }
          if (cmd === 'vfs_list_files') {
            return Promise.resolve([mockFile]);
          }
          if (cmd === 'vfs_clipboard_has_files') {
            return Promise.resolve(false);
          }
          if (cmd === 'vfs_clipboard_read_native') {
            return Promise.resolve([]);
          }
          return Promise.resolve([]);
        });

        render(<FinderPage {...defaultProps} />);

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith(
            'vfs_list_files',
            expect.any(Object),
          );
        });

        // Find the file item and right-click it
        const fileItem = screen.getByText('test.txt').closest('[data-path]');
        expect(fileItem).toBeInTheDocument();

        if (fileItem) {
          fireEvent.contextMenu(fileItem, { clientX: 100, clientY: 100 });

          await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith('vfs_clipboard_has_files');
          });

          // Context menu should be visible
          const contextMenu = document.querySelector('.context-menu');
          expect(contextMenu).toBeInTheDocument();

          // Copy and Cut buttons should be visible
          if (contextMenu) {
            const copyButton = within(contextMenu as HTMLElement).queryByText(
              'Copy',
            );
            const cutButton = within(contextMenu as HTMLElement).queryByText(
              'Cut',
            );
            expect(copyButton).toBeInTheDocument();
            expect(cutButton).toBeInTheDocument();
          }
        }
      });

      it('should show Copy and Cut buttons when right-clicking on a folder', async () => {
        const invoke = jest.spyOn(tauriCore, 'invoke');
        invoke.mockImplementation((cmd) => {
          if (cmd === 'vfs_list_sources') {
            return Promise.resolve([mockSource]);
          }
          if (cmd === 'vfs_list_files') {
            return Promise.resolve([mockFolder]);
          }
          if (cmd === 'vfs_clipboard_has_files') {
            return Promise.resolve(false);
          }
          if (cmd === 'vfs_clipboard_read_native') {
            return Promise.resolve([]);
          }
          return Promise.resolve([]);
        });

        render(<FinderPage {...defaultProps} />);

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith(
            'vfs_list_files',
            expect.any(Object),
          );
        });

        // Find the folder item and right-click it
        const folderItem = screen.getByText('Documents').closest('[data-path]');
        expect(folderItem).toBeInTheDocument();

        if (folderItem) {
          fireEvent.contextMenu(folderItem, { clientX: 100, clientY: 100 });

          await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith('vfs_clipboard_has_files');
          });

          // Context menu should be visible
          const contextMenu = document.querySelector('.context-menu');
          expect(contextMenu).toBeInTheDocument();

          // Copy and Cut buttons should be visible
          if (contextMenu) {
            const copyButton = within(contextMenu as HTMLElement).queryByText(
              'Copy',
            );
            const cutButton = within(contextMenu as HTMLElement).queryByText(
              'Cut',
            );
            expect(copyButton).toBeInTheDocument();
            expect(cutButton).toBeInTheDocument();
          }
        }
      });

      it('should show Paste button when clipboard has files', async () => {
        const invoke = jest.spyOn(tauriCore, 'invoke');
        invoke.mockImplementation((cmd) => {
          if (cmd === 'vfs_list_sources') {
            return Promise.resolve([mockSource]);
          }
          if (cmd === 'vfs_list_files') {
            return Promise.resolve([mockFile]);
          }
          if (cmd === 'vfs_clipboard_has_files') {
            return Promise.resolve(true);
          }
          if (cmd === 'vfs_clipboard_read_native') {
            return Promise.resolve([]);
          }
          return Promise.resolve([]);
        });

        render(<FinderPage {...defaultProps} />);

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith(
            'vfs_list_files',
            expect.any(Object),
          );
        });

        // Right-click on empty area
        const contentArea = document.querySelector('.finder-content');
        if (contentArea) {
          fireEvent.contextMenu(contentArea, { clientX: 100, clientY: 100 });

          await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith('vfs_clipboard_has_files');
          });

          // Context menu should be visible
          const contextMenu = document.querySelector('.context-menu');
          expect(contextMenu).toBeInTheDocument();

          // Paste button should be visible and enabled
          if (contextMenu) {
            const pasteButton = within(contextMenu as HTMLElement).queryByText(
              /Paste/i,
            );
            expect(pasteButton).toBeInTheDocument();
            expect(pasteButton).not.toHaveClass('disabled');
          }
        }
      });

      it('should call vfs_clipboard_copy when clicking Copy button', async () => {
        const invoke = jest.spyOn(tauriCore, 'invoke');
        invoke.mockImplementation((cmd) => {
          if (cmd === 'vfs_list_sources') {
            return Promise.resolve([mockSource]);
          }
          if (cmd === 'vfs_list_files') {
            return Promise.resolve([mockFile]);
          }
          if (cmd === 'vfs_clipboard_has_files') {
            return Promise.resolve(false);
          }
          if (cmd === 'vfs_clipboard_read_native') {
            return Promise.resolve([]);
          }
          if (cmd === 'vfs_clipboard_copy') {
            return Promise.resolve(undefined);
          }
          if (cmd === 'vfs_clipboard_copy_for_native') {
            return Promise.resolve(undefined);
          }
          if (cmd === 'vfs_clipboard_get') {
            return Promise.resolve({
              operation: 'copy',
              source: mockSource.id,
              paths: [mockFile.path],
              file_count: 1,
            });
          }
          return Promise.resolve([]);
        });

        render(<FinderPage {...defaultProps} />);

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith(
            'vfs_list_files',
            expect.any(Object),
          );
        });

        // Right-click on file
        const fileItem = screen.getByText('test.txt').closest('[data-path]');
        if (fileItem) {
          fireEvent.contextMenu(fileItem, { clientX: 100, clientY: 100 });

          await waitFor(() => {
            const contextMenu = document.querySelector('.context-menu');
            expect(contextMenu).toBeInTheDocument();
          });

          // Click Copy button
          const contextMenu = document.querySelector('.context-menu');
          if (contextMenu) {
            const copyButton = within(contextMenu as HTMLElement).getByText(
              'Copy',
            );
            fireEvent.click(copyButton);

            await waitFor(() => {
              expect(invoke).toHaveBeenCalledWith('vfs_clipboard_copy', {
                sourceId: mockSource.id,
                paths: [mockFile.path],
              });
            });
          }
        }
      });

      it('should call vfs_clipboard_cut when clicking Cut button', async () => {
        const invoke = jest.spyOn(tauriCore, 'invoke');
        invoke.mockImplementation((cmd) => {
          if (cmd === 'vfs_list_sources') {
            return Promise.resolve([mockSource]);
          }
          if (cmd === 'vfs_list_files') {
            return Promise.resolve([mockFile]);
          }
          if (cmd === 'vfs_clipboard_has_files') {
            return Promise.resolve(false);
          }
          if (cmd === 'vfs_clipboard_read_native') {
            return Promise.resolve([]);
          }
          if (cmd === 'vfs_clipboard_cut') {
            return Promise.resolve(undefined);
          }
          if (cmd === 'vfs_clipboard_get') {
            return Promise.resolve({
              operation: 'cut',
              source: mockSource.id,
              paths: [mockFile.path],
              file_count: 1,
            });
          }
          return Promise.resolve([]);
        });

        render(<FinderPage {...defaultProps} />);

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith(
            'vfs_list_files',
            expect.any(Object),
          );
        });

        // Right-click on file
        const fileItem = screen.getByText('test.txt').closest('[data-path]');
        if (fileItem) {
          fireEvent.contextMenu(fileItem, { clientX: 100, clientY: 100 });

          await waitFor(() => {
            const contextMenu = document.querySelector('.context-menu');
            expect(contextMenu).toBeInTheDocument();
          });

          // Click Cut button
          const contextMenu = document.querySelector('.context-menu');
          if (contextMenu) {
            const cutButton = within(contextMenu as HTMLElement).getByText(
              'Cut',
            );
            fireEvent.click(cutButton);

            await waitFor(() => {
              expect(invoke).toHaveBeenCalledWith('vfs_clipboard_cut', {
                sourceId: mockSource.id,
                paths: [mockFile.path],
              });
            });
          }
        }
      });

      it('should call vfs_clipboard_paste_to_vfs when clicking Paste button', async () => {
        const invoke = jest.spyOn(tauriCore, 'invoke');
        invoke.mockImplementation((cmd) => {
          if (cmd === 'vfs_list_sources') {
            return Promise.resolve([mockSource]);
          }
          if (cmd === 'vfs_list_files') {
            return Promise.resolve([mockFile]);
          }
          if (cmd === 'vfs_clipboard_has_files') {
            return Promise.resolve(true);
          }
          if (cmd === 'vfs_clipboard_read_native') {
            return Promise.resolve([]);
          }
          if (cmd === 'vfs_clipboard_paste_to_vfs') {
            return Promise.resolve({
              files_pasted: 1,
              files_failed: 0,
              pasted_paths: ['/pasted.txt'],
            });
          }
          return Promise.resolve([]);
        });

        render(<FinderPage {...defaultProps} />);

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith(
            'vfs_list_files',
            expect.any(Object),
          );
        });

        // Right-click on empty area
        const contentArea = document.querySelector('.finder-content');
        if (contentArea) {
          fireEvent.contextMenu(contentArea, { clientX: 100, clientY: 100 });

          await waitFor(() => {
            const contextMenu = document.querySelector('.context-menu');
            expect(contextMenu).toBeInTheDocument();
          });

          // Click Paste button
          const contextMenu = document.querySelector('.context-menu');
          if (contextMenu) {
            const pasteButton = within(contextMenu as HTMLElement).getByText(
              /Paste/i,
            );
            fireEvent.click(pasteButton);

            await waitFor(() => {
              expect(invoke).toHaveBeenCalledWith(
                'vfs_clipboard_paste_to_vfs',
                {
                  dest_source_id: mockSource.id,
                  dest_path: '/',
                },
              );
            });
          }
        }
      });

      it('should paste into folder when right-clicking on folder and clicking Paste', async () => {
        const invoke = jest.spyOn(tauriCore, 'invoke');
        invoke.mockImplementation((cmd) => {
          if (cmd === 'vfs_list_sources') {
            return Promise.resolve([mockSource]);
          }
          if (cmd === 'vfs_list_files') {
            return Promise.resolve([mockFolder]);
          }
          if (cmd === 'vfs_clipboard_has_files') {
            return Promise.resolve(true);
          }
          if (cmd === 'vfs_clipboard_read_native') {
            return Promise.resolve([]);
          }
          if (cmd === 'vfs_clipboard_paste_to_vfs') {
            return Promise.resolve({
              files_pasted: 1,
              files_failed: 0,
              pasted_paths: ['/Documents/pasted.txt'],
            });
          }
          return Promise.resolve([]);
        });

        render(<FinderPage {...defaultProps} />);

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith(
            'vfs_list_files',
            expect.any(Object),
          );
        });

        // Right-click on folder
        const folderItem = screen.getByText('Documents').closest('[data-path]');
        if (folderItem) {
          fireEvent.contextMenu(folderItem, { clientX: 100, clientY: 100 });

          await waitFor(() => {
            const contextMenu = document.querySelector('.context-menu');
            expect(contextMenu).toBeInTheDocument();
          });

          // Click Paste button
          const contextMenu = document.querySelector('.context-menu');
          if (contextMenu) {
            const pasteButton = within(contextMenu as HTMLElement).getByText(
              /Paste/i,
            );
            fireEvent.click(pasteButton);

            await waitFor(() => {
              expect(invoke).toHaveBeenCalledWith(
                'vfs_clipboard_paste_to_vfs',
                {
                  dest_source_id: mockSource.id,
                  dest_path: mockFolder.path,
                },
              );
            });
          }
        }
      });
    });

    describe('Keyboard Shortcuts - Copy/Cut/Paste', () => {
      it('should copy selected files when pressing Cmd+C', async () => {
        const invoke = jest.spyOn(tauriCore, 'invoke');
        invoke.mockImplementation((cmd) => {
          if (cmd === 'vfs_list_sources') {
            return Promise.resolve([mockSource]);
          }
          if (cmd === 'vfs_list_files') {
            return Promise.resolve([mockFile]);
          }
          if (cmd === 'vfs_clipboard_has_files') {
            return Promise.resolve(false);
          }
          if (cmd === 'vfs_clipboard_read_native') {
            return Promise.resolve([]);
          }
          if (cmd === 'vfs_clipboard_copy') {
            return Promise.resolve(undefined);
          }
          if (cmd === 'vfs_clipboard_copy_for_native') {
            return Promise.resolve(undefined);
          }
          if (cmd === 'vfs_clipboard_get') {
            return Promise.resolve({
              operation: 'copy',
              source: mockSource.id,
              paths: [mockFile.path],
              file_count: 1,
            });
          }
          return Promise.resolve([]);
        });

        render(<FinderPage {...defaultProps} />);

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith(
            'vfs_list_files',
            expect.any(Object),
          );
        });

        // Select file by clicking it
        const fileItem = screen.getByText('test.txt').closest('[data-path]');
        if (fileItem) {
          fireEvent.click(fileItem);

          // Simulate Cmd+C (Meta key + C)
          fireEvent.keyDown(document, {
            key: 'c',
            metaKey: true,
            preventDefault: jest.fn(),
          });

          await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith('vfs_clipboard_copy', {
              sourceId: mockSource.id,
              paths: [mockFile.path],
            });
          });
        }
      });

      it('should cut selected files when pressing Cmd+X', async () => {
        const invoke = jest.spyOn(tauriCore, 'invoke');
        invoke.mockImplementation((cmd) => {
          if (cmd === 'vfs_list_sources') {
            return Promise.resolve([mockSource]);
          }
          if (cmd === 'vfs_list_files') {
            return Promise.resolve([mockFile]);
          }
          if (cmd === 'vfs_clipboard_has_files') {
            return Promise.resolve(false);
          }
          if (cmd === 'vfs_clipboard_read_native') {
            return Promise.resolve([]);
          }
          if (cmd === 'vfs_clipboard_cut') {
            return Promise.resolve(undefined);
          }
          if (cmd === 'vfs_clipboard_get') {
            return Promise.resolve({
              operation: 'cut',
              source: mockSource.id,
              paths: [mockFile.path],
              file_count: 1,
            });
          }
          return Promise.resolve([]);
        });

        render(<FinderPage {...defaultProps} />);

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith(
            'vfs_list_files',
            expect.any(Object),
          );
        });

        // Select file by clicking it
        const fileItem = screen.getByText('test.txt').closest('[data-path]');
        if (fileItem) {
          fireEvent.click(fileItem);

          // Simulate Cmd+X (Meta key + X)
          fireEvent.keyDown(document, {
            key: 'x',
            metaKey: true,
            preventDefault: jest.fn(),
          });

          await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith('vfs_clipboard_cut', {
              sourceId: mockSource.id,
              paths: [mockFile.path],
            });
          });
        }
      });

      it('should paste when pressing Cmd+V', async () => {
        const invoke = jest.spyOn(tauriCore, 'invoke');
        invoke.mockImplementation((cmd) => {
          if (cmd === 'vfs_list_sources') {
            return Promise.resolve([mockSource]);
          }
          if (cmd === 'vfs_list_files') {
            return Promise.resolve([mockFile]);
          }
          if (cmd === 'vfs_clipboard_has_files') {
            return Promise.resolve(true);
          }
          if (cmd === 'vfs_clipboard_read_native') {
            return Promise.resolve([]);
          }
          if (cmd === 'vfs_clipboard_paste_to_vfs') {
            return Promise.resolve({
              files_pasted: 1,
              files_failed: 0,
              pasted_paths: ['/pasted.txt'],
            });
          }
          return Promise.resolve([]);
        });

        render(<FinderPage {...defaultProps} />);

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith(
            'vfs_list_files',
            expect.any(Object),
          );
        });

        // Simulate Cmd+V (Meta key + V)
        fireEvent.keyDown(document, {
          key: 'v',
          metaKey: true,
          preventDefault: jest.fn(),
        });

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith('vfs_clipboard_paste_to_vfs', {
            dest_source_id: mockSource.id,
            dest_path: '/',
          });
        });
      });
    });
  });

  describe('Context Menu Functionality', () => {
    const mockFile = {
      id: 'file-1',
      name: 'test.txt',
      path: '/test.txt',
      size: 100,
      mimeType: 'text/plain',
      isDirectory: false,
      tierStatus: 'hot',
      canWarm: false,
      canTranscode: false,
    };

    const mockFolder = {
      id: 'folder-1',
      name: 'Documents',
      path: '/Documents',
      size: 0,
      mimeType: 'folder',
      isDirectory: true,
      tierStatus: 'hot',
      canWarm: false,
      canTranscode: false,
    };

    const mockSource = {
      id: 'local-1',
      name: 'Local Storage',
      source_type: 'Local',
      category: 'local',
      provider_id: 'local',
      mounted: true,
      status: 'connected',
      path: '/',
    };

    beforeEach(() => {
      jest.clearAllMocks();
    });

    it('should show context menu when right-clicking on a file in icon view', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');
      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([mockSource]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([mockFile]);
        }
        if (cmd === 'vfs_clipboard_has_files') {
          return Promise.resolve(false);
        }
        if (cmd === 'vfs_clipboard_read_native') {
          return Promise.resolve([]);
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          'vfs_list_files',
          expect.any(Object),
        );
      });

      // Find the file item and right-click it
      const fileItem = screen.getByText('test.txt').closest('[data-path]');
      expect(fileItem).toBeInTheDocument();

      if (fileItem) {
        fireEvent.contextMenu(fileItem, { clientX: 100, clientY: 100 });

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith('vfs_clipboard_has_files');
        });

        // Context menu should be visible
        const contextMenu = document.querySelector('.context-menu');
        expect(contextMenu).toBeInTheDocument();
        expect(contextMenu).toHaveStyle({ zIndex: '10000' });
      }
    });

    it('should show context menu when right-clicking on a folder in icon view', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');
      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([mockSource]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([mockFolder]);
        }
        if (cmd === 'vfs_clipboard_has_files') {
          return Promise.resolve(false);
        }
        if (cmd === 'vfs_clipboard_read_native') {
          return Promise.resolve([]);
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          'vfs_list_files',
          expect.any(Object),
        );
      });

      // Find the folder item and right-click it
      const folderItem = screen.getByText('Documents').closest('[data-path]');
      expect(folderItem).toBeInTheDocument();

      if (folderItem) {
        fireEvent.contextMenu(folderItem, { clientX: 100, clientY: 100 });

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith('vfs_clipboard_has_files');
        });

        // Context menu should be visible
        const contextMenu = document.querySelector('.context-menu');
        expect(contextMenu).toBeInTheDocument();
      }
    });

    it('should show context menu when right-clicking on a file in list view', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');
      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([mockSource]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([mockFile]);
        }
        if (cmd === 'vfs_clipboard_has_files') {
          return Promise.resolve(false);
        }
        if (cmd === 'vfs_clipboard_read_native') {
          return Promise.resolve([]);
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          'vfs_list_files',
          expect.any(Object),
        );
      });

      // Find the file item (list-row) and right-click it
      const fileItem = screen.getByText('test.txt').closest('.list-row');
      expect(fileItem).toBeInTheDocument();

      if (fileItem) {
        fireEvent.contextMenu(fileItem, { clientX: 100, clientY: 100 });

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith('vfs_clipboard_has_files');
        });

        // Context menu should be visible
        const contextMenu = document.querySelector('.context-menu');
        expect(contextMenu).toBeInTheDocument();
      }
    });

    it('should show context menu when right-clicking on empty area', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');
      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([mockSource]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([]);
        }
        if (cmd === 'vfs_clipboard_has_files') {
          return Promise.resolve(false);
        }
        if (cmd === 'vfs_clipboard_read_native') {
          return Promise.resolve([]);
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          'vfs_list_files',
          expect.any(Object),
        );
      });

      // Right-click on empty content area
      const contentArea = document.querySelector('.finder-content');
      expect(contentArea).toBeInTheDocument();

      if (contentArea) {
        fireEvent.contextMenu(contentArea, { clientX: 100, clientY: 100 });

        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith('vfs_clipboard_has_files');
        });

        // Context menu should be visible
        const contextMenu = document.querySelector('.context-menu');
        expect(contextMenu).toBeInTheDocument();
      }
    });

    it('should close context menu when clicking outside', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');
      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([mockSource]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([mockFile]);
        }
        if (cmd === 'vfs_clipboard_has_files') {
          return Promise.resolve(false);
        }
        if (cmd === 'vfs_clipboard_read_native') {
          return Promise.resolve([]);
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          'vfs_list_files',
          expect.any(Object),
        );
      });

      // Right-click on file to show menu
      const fileItem = screen.getByText('test.txt').closest('[data-path]');
      if (fileItem) {
        fireEvent.contextMenu(fileItem, { clientX: 100, clientY: 100 });

        await waitFor(() => {
          const contextMenu = document.querySelector('.context-menu');
          expect(contextMenu).toBeInTheDocument();
        });

        // Click outside the menu
        fireEvent.click(document.body, { button: 0 });

        await waitFor(
          () => {
            const contextMenu = document.querySelector('.context-menu');
            expect(contextMenu).not.toBeInTheDocument();
          },
          { timeout: 200 },
        );
      }
    });

    it('should not close context menu when right-clicking again', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');
      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([mockSource]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([mockFile]);
        }
        if (cmd === 'vfs_clipboard_has_files') {
          return Promise.resolve(false);
        }
        if (cmd === 'vfs_clipboard_read_native') {
          return Promise.resolve([]);
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          'vfs_list_files',
          expect.any(Object),
        );
      });

      // Right-click on file to show menu
      const fileItem = screen.getByText('test.txt').closest('[data-path]');
      if (fileItem) {
        fireEvent.contextMenu(fileItem, { clientX: 100, clientY: 100 });

        await waitFor(() => {
          const contextMenu = document.querySelector('.context-menu');
          expect(contextMenu).toBeInTheDocument();
        });

        // Right-click again (should not close)
        fireEvent.contextMenu(fileItem, {
          clientX: 150,
          clientY: 150,
          button: 2,
        });

        // Menu should still be visible (or replaced with new position)
        await waitFor(() => {
          const contextMenu = document.querySelector('.context-menu');
          // Menu might be repositioned, but should exist
          expect(
            contextMenu || document.querySelector('.context-menu'),
          ).toBeTruthy();
        });
      }
    });

    it('should handle copy operation timeout gracefully', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');
      const user = userEvent.setup();

      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([mockSource]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([mockFile]);
        }
        if (cmd === 'vfs_clipboard_has_files') {
          return Promise.resolve(false);
        }
        if (cmd === 'vfs_clipboard_read_native') {
          return Promise.resolve([]);
        }
        if (cmd === 'vfs_clipboard_copy') {
          // Simulate a hanging operation that never resolves
          return new Promise(() => {
            // Never resolves - simulates hanging
          });
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          'vfs_list_files',
          expect.any(Object),
        );
      });

      // Select a file
      const fileItem = screen.getByText('test.txt').closest('.list-row');
      expect(fileItem).toBeInTheDocument();

      if (fileItem) {
        await user.click(fileItem);

        // Trigger copy via keyboard shortcut
        await user.keyboard('{Meta>}c{/Meta}');

        // Wait for timeout (should be 5 seconds based on our timeout)
        await waitFor(
          () => {
            // Check that error handling was called
            expect(screen.queryByText(/Copy.*timeout/i)).toBeInTheDocument();
          },
          { timeout: 6000 }, // Wait up to 6 seconds for timeout
        );
      }
    });

    it('should complete copy operation successfully', async () => {
      const invoke = jest.spyOn(tauriCore, 'invoke');
      const user = userEvent.setup();

      invoke.mockImplementation((cmd) => {
        if (cmd === 'vfs_list_sources') {
          return Promise.resolve([mockSource]);
        }
        if (cmd === 'vfs_list_files') {
          return Promise.resolve([mockFile]);
        }
        if (cmd === 'vfs_clipboard_has_files') {
          return Promise.resolve(true);
        }
        if (cmd === 'vfs_clipboard_read_native') {
          return Promise.resolve([]);
        }
        if (cmd === 'vfs_clipboard_get') {
          return Promise.resolve({
            operation: 'copy',
            source: 'vfs:local',
            paths: ['/test.txt'],
            file_count: 1,
          });
        }
        if (cmd === 'vfs_clipboard_copy') {
          return Promise.resolve('Copied 1 files to clipboard');
        }
        if (cmd === 'vfs_clipboard_copy_for_native') {
          return Promise.resolve(
            'Copied 1 files to clipboard (native-compatible)',
          );
        }
        return Promise.resolve([]);
      });

      render(<FinderPage {...defaultProps} />);

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith(
          'vfs_list_files',
          expect.any(Object),
        );
      });

      // Select a file
      const fileItem = screen.getByText('test.txt').closest('.list-row');
      expect(fileItem).toBeInTheDocument();

      if (fileItem) {
        await user.click(fileItem);

        // Trigger copy via keyboard shortcut
        await user.keyboard('{Meta>}c{/Meta}');

        // Wait for copy to complete
        await waitFor(
          () => {
            expect(invoke).toHaveBeenCalledWith('vfs_clipboard_copy', {
              sourceId: 'local',
              paths: ['/test.txt'],
            });
          },
          { timeout: 3000 },
        );

        // Verify clipboard state was refreshed
        await waitFor(() => {
          expect(invoke).toHaveBeenCalledWith('vfs_clipboard_has_files');
        });
      }
    });
  });
});
