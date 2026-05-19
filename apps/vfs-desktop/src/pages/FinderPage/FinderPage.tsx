/**
 * macOS Finder-inspired Virtual File System Browser
 * Supports multiple view modes and hybrid storage backends
 */
import React, {
  useState,
  useEffect,
  useRef,
  useCallback,
  useMemo,
} from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { DialogService } from '../../services/dialog';
import { DialogType } from '../../domain/enums/DialogType';
import type {
  StorageSource,
  StorageCategory,
  FileMetadata,
  WarmProgress,
  GlobalFavorite,
} from '../../types/storage';
import type { BreadcrumbItem } from '../../components/Breadcrumbs';
import {
  IconStar,
  IconHome,
  IconDesktop,
  IconDocuments,
  IconDownloads,
  IconPictures,
  IconMusic,
  IconVolumes,
  IconCloud,
  IconNetwork,
  IconDatabase,
  IconTag,
  IconFolder,
  // getFileIcon as getFileIconComponent, // Unused - kept for potential future use
} from '../../components/CyberpunkIcons';
import { InfoModal } from '../../components/InfoModal';
import { CommentModal } from '../../components/CommentModal';
import { AddStorageModal } from '../../components/AddStorageModal';
import {
  KeyboardShortcutHelper,
  useKeyboardShortcutHelper,
} from '../../components/KeyboardShortcutHelper';
import { useToast } from '../../components/Toast';
import { ShortcutSettings } from '../../components/ShortcutSettings';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { SpotlightSearch } from '../../components/SpotlightSearch';
import { MetricsPreview } from '../../components/MetricsPreview';
import { OperationsPanel } from '../../components/OperationsPanel';
import { TranscriptionProgressPanel } from '../../components/TranscriptionProgress';
import { TransferPanel } from '../../components/TransferPanel';
import { StorageTierDialog } from '../../components/StorageTierDialog';
import { getPlatformInfoSync } from '../../services/platform';
import { supportsFilesystemOperations, getStorageClassBadge } from './utils';
import { getCategoryName } from '../../types/storage';
import type { ColumnWidths } from './types';
import { FinderToolbar } from '../../components/FinderToolbar';
import { FileBrowser } from '../../components/FileBrowser';
import { FinderInfoPanel } from '../../components/FinderInfoPanel';
import { FinderContextMenu } from '../../components/FinderContextMenu';
import { fileCache } from '../../utils/file-cache';
import './FinderPage.css';

type ViewMode = 'icon' | 'list';

interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  targetFile?: FileMetadata;
}

interface FinderPageProps {
  onOpenMetrics?: () => void;
  onOpenSearch?: () => void;
  isSearchOpen?: boolean;
  onCloseSearch?: () => void;
  onOpenSettings?: () => void;
}

export function FinderPage({
  onOpenMetrics,
  onOpenSearch: _onOpenSearch,
  isSearchOpen: externalSearchOpen,
  onCloseSearch: externalCloseSearch,
  onOpenSettings,
}: FinderPageProps) {
  const [sources, setSources] = useState<StorageSource[]>([]);
  const [sourcesLoaded, setSourcesLoaded] = useState(false);
  const [selectedSource, setSelectedSource] = useState<StorageSource | null>(
    null,
  );
  const [currentPath, setCurrentPath] = useState('');
  const [files, setFiles] = useState<FileMetadata[]>([]);

  // Memoize filtered source arrays to prevent unnecessary re-renders
  const localSources = useMemo(
    () => sources.filter((s) => s.category === 'local'),
    [sources],
  );
  const locations = useMemo(
    () => localSources.filter((s) => !s.isEjectable),
    [localSources],
  );
  const volumes = useMemo(
    () => localSources.filter((s) => s.isEjectable),
    [localSources],
  );
  const networkSources = useMemo(
    () =>
      sources.filter(
        (s) => s.category === 'network' || s.category === 'hybrid',
      ),
    [sources],
  );
  const cloudSources = useMemo(
    () => sources.filter((s) => s.category === 'cloud'),
    [sources],
  );
  const blockSources = useMemo(
    () => sources.filter((s) => s.category === 'block'),
    [sources],
  );
  const awsSources = useMemo(
    () =>
      cloudSources.filter(
        (s) =>
          s.providerId === 'aws-s3' ||
          s.providerId === 's3' ||
          s.providerId === 's3-compatible',
      ),
    [cloudSources],
  );
  const azureSources = useMemo(
    () => cloudSources.filter((s) => s.providerId === 'azure-blob'),
    [cloudSources],
  );
  const gcpSources = useMemo(
    () => cloudSources.filter((s) => s.providerId === 'gcs'),
    [cloudSources],
  );
  // Cache for folder sizes to avoid recalculating
  const folderSizeCache = useRef<Map<string, number>>(new Map());
  // Pagination state for object storage
  const [paginationState, setPaginationState] = useState<{
    continuationToken: string | null;
    hasMore: boolean;
    totalCount: number | null;
    isLoadingMore: boolean;
  }>({
    continuationToken: null,
    hasMore: false,
    totalCount: null,
    isLoadingMore: false,
  });
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [viewMode, setViewMode] = useState<ViewMode>('list');
  const [loading, setLoading] = useState(false);
  // Track current request to prevent race conditions when switching sources
  const currentRequestRef = useRef<{ sourceId: string; path: string } | null>(
    null,
  );
  const abortControllerRef = useRef<AbortController | null>(null);
  const [showInfoPanel, setShowInfoPanel] = useState(false);
  const [activeTab, setActiveTab] = useState<'files' | 'transfers'>('files');
  const [showAddStorage, setShowAddStorage] = useState(false);
  const [editingSource, setEditingSource] = useState<StorageSource | null>(
    null,
  );
  const [showHiddenFiles, setShowHiddenFiles] = useState(true);
  // Removed: activeUploads state - UploadProgressPanel now manages its own visibility
  const [showTierDialog, setShowTierDialog] = useState(false);
  const [tierDialogPaths, setTierDialogPaths] = useState<string[]>([]);
  // Warm progress state - kept for event listener but not displayed (tier labels removed)
  const [, setWarmProgress] = useState<Record<string, WarmProgress>>({});
  const [searchQuery, setSearchQuery] = useState('');
  const [fileOperation, setFileOperation] = useState<{
    type: string;
    inProgress: boolean;
  } | null>(null);
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    targetFile: undefined,
  });
  // Clipboard state - tracks whether paste is available
  const [clipboardHasFiles, setClipboardHasFiles] = useState(false);
  const [, setNativeClipboardCount] = useState(0);
  const [cutFiles, setCutFiles] = useState<Set<string>>(new Set()); // Track cut files for visual feedback
  // AI model availability
  const [aiModelsAvailable, setAiModelsAvailable] = useState(false);
  const [favorites, setFavorites] = useState<GlobalFavorite[]>([]);
  const [allTags, setAllTags] = useState<{ name: string; color?: string }[]>(
    [],
  );
  const [filterByTag, setFilterByTag] = useState<string | null>(null);
  const [columnFilters, setColumnFilters] = useState<{
    name: string;
    date: string;
    size: string;
    tier: string;
  }>({
    name: '',
    date: '',
    size: '',
    tier: '',
  });
  const [sortColumn, setSortColumn] = useState<
    'name' | 'modified' | 'size' | 'storage-class'
  >('name');
  const [sortDirection, setSortDirection] = useState<'asc' | 'desc'>('asc');
  const [sidebarWidth, setSidebarWidth] = useState(() => {
    // Load saved sidebar width from localStorage, default to 240px (increased from 200px)
    try {
      const saved = localStorage.getItem('diaspor-sidebar-width');
      if (saved) {
        const width = parseInt(saved, 10);
        if (width >= 180 && width <= 800) {
          return width;
        }
      }
    } catch {
      // Ignore localStorage errors
    }
    return 240; // Increased default width for better visibility
  });
  const [isResizing, setIsResizing] = useState(false);

  // Track if we have saved column widths from localStorage
  const [, setHasSavedColumnWidths] = useState(() => {
    try {
      const saved = localStorage.getItem('diaspor-column-widths');
      if (saved) {
        const widths = JSON.parse(saved);
        // Validate widths
        if (
          widths.name >= 150 &&
          widths.name <= 1000 &&
          widths.modified >= 100 &&
          widths.modified <= 500 &&
          widths.size >= 80 &&
          widths.size <= 300 &&
          widths.tier >= 0 &&
          widths.tier <= 300 && // Tier can be 0 (hidden) or up to 300
          (!widths['storage-class'] ||
            (widths['storage-class'] >= 100 && widths['storage-class'] <= 300))
        ) {
          return true;
        }
      }
    } catch {
      // Ignore localStorage errors
    }
    return false;
  });

  // Column widths for list view (in pixels)
  // Use null initially to trigger calculation based on available space
  const [columnWidths, setColumnWidths] = useState<ColumnWidths | null>(() => {
    try {
      const saved = localStorage.getItem('diaspor-column-widths');
      if (saved) {
        const widths = JSON.parse(saved);
        // Validate widths
        if (
          widths.name >= 150 &&
          widths.name <= 1000 &&
          widths.modified >= 100 &&
          widths.modified <= 500 &&
          widths.size >= 80 &&
          widths.size <= 300 &&
          widths.tier >= 0 &&
          widths.tier <= 300 && // Tier can be 0 (hidden) or up to 300
          (!widths['storage-class'] ||
            (widths['storage-class'] >= 100 && widths['storage-class'] <= 300))
        ) {
          // Ensure storage-class is set if missing
          if (!widths['storage-class']) {
            widths['storage-class'] = 140;
          }
          // Ensure tier is 0 since it's hidden
          widths.tier = 0;
          return widths;
        }
      }
    } catch {
      // Ignore localStorage errors
    }
    // Return null to trigger calculation based on available space
    return null;
  });

  const [resizingColumn, setResizingColumn] = useState<string | null>(null);
  const [resizeStartX, setResizeStartX] = useState(0);
  const [resizeStartWidth, setResizeStartWidth] = useState(0);
  // Sidebar section reordering - prepared for future implementation
  // const [sidebarSectionOrder, setSidebarSectionOrder] = useState<string[]>([
  //   'favorites',
  //   'storage',
  //   'tags',
  // ]);

  // Handle sidebar resizing
  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
  }, []);

  useEffect(() => {
    if (!isResizing) return;

    const handleMouseMove = (e: MouseEvent) => {
      const newWidth = Math.max(180, Math.min(800, e.clientX));
      setSidebarWidth(newWidth);
    };

    const handleMouseUp = () => {
      setIsResizing(false);
      // Save to localStorage
      try {
        localStorage.setItem('diaspor-sidebar-width', sidebarWidth.toString());
      } catch {
        // Ignore localStorage errors
      }
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isResizing, sidebarWidth]);

  // Handle column resizing
  const handleColumnResizeStart = useCallback(
    (e: React.MouseEvent, column: string) => {
      e.preventDefault();
      e.stopPropagation();
      if (!columnWidths) return;
      setResizingColumn(column);
      setResizeStartX(e.clientX);
      const width = columnWidths[column as keyof typeof columnWidths];
      // Default width based on column type
      const defaultWidth = column === 'storage-class' ? 140 : 120;
      setResizeStartWidth(width ?? defaultWidth);
    },
    [columnWidths],
  );

  useEffect(() => {
    if (!resizingColumn) return;

    const handleMouseMove = (e: MouseEvent) => {
      const deltaX = e.clientX - resizeStartX;
      // Define min/max widths for each column
      const minWidths: Record<string, number> = {
        name: 150,
        modified: 100,
        size: 80,
        tier: 80,
        'storage-class': 100,
      };
      const maxWidths: Record<string, number> = {
        name: 1000,
        modified: 500,
        size: 500,
        tier: 300,
        'storage-class': 300,
      };

      const minWidth = minWidths[resizingColumn] || 80;
      const maxWidth = maxWidths[resizingColumn] || 500;
      const newWidth = Math.max(
        minWidth,
        Math.min(maxWidth, resizeStartWidth + deltaX),
      );

      setColumnWidths((prev: ColumnWidths | null) => {
        if (!prev) return null;
        return {
          ...prev,
          [resizingColumn]: newWidth,
        };
      });
    };

    const handleMouseUp = () => {
      setResizingColumn(null);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [resizingColumn, resizeStartX, resizeStartWidth]);

  // Save column widths to localStorage whenever they change (but not during resize)
  useEffect(() => {
    if (!resizingColumn && columnWidths) {
      try {
        localStorage.setItem(
          'diaspor-column-widths',
          JSON.stringify(columnWidths),
        );
        setHasSavedColumnWidths(true);
      } catch {
        // Ignore localStorage errors
      }
    }
  }, [columnWidths, resizingColumn]);

  // Handle column sort toggle
  const handleSortChange = useCallback(
    (column: 'name' | 'modified' | 'size' | 'storage-class') => {
      if (sortColumn === column) {
        // Toggle direction if same column
        setSortDirection((prev) => (prev === 'asc' ? 'desc' : 'asc'));
      } else {
        // New column, default to ascending (except for modified which defaults to desc for most recent first)
        setSortColumn(column);
        setSortDirection(column === 'modified' ? 'desc' : 'asc');
      }
    },
    [sortColumn],
  );

  // Calculate column widths based on available space if no saved widths exist
  useEffect(() => {
    // Calculate if we don't have saved widths (columnWidths is null) and we're in list view
    if (columnWidths !== null || viewMode !== 'list' || !listViewRef.current) {
      return;
    }

    const calculateColumnWidths = () => {
      const container = listViewRef.current;
      if (!container) return;

      // Get container width, accounting for padding (24px total: 12px on each side)
      // and gaps between columns (4 gaps of 8px each = 32px for 5 columns)
      const containerWidth = container.clientWidth;
      const horizontalPadding = 24; // 12px on each side
      const columnGaps = 32; // 4 gaps of 8px each between 5 columns
      const availableWidth = containerWidth - horizontalPadding - columnGaps;

      // Minimum widths for each column
      const minName = 150;
      const minDate = 100;
      const minSize = 80;
      const minTier = 80;
      const minTotal = minName + minDate + minSize + minTier;

      // Storage class column minimum and default width
      const minStorageClass = 100;
      const defaultStorageClass = 140; // Increased from 120 for better visibility
      const minTotalWithStorageClass =
        minName + minDate + minSize + minTier + minStorageClass;

      // If container is too small, use minimums
      if (availableWidth < minTotalWithStorageClass) {
        setColumnWidths({
          name: minName,
          modified: minDate,
          size: minSize,
          tier: 0, // Tier column is hidden, always set to 0
          'storage-class': minStorageClass,
        });
        return;
      }

      // Calculate proportional widths to use full available width
      // Name gets ~40%, Date gets ~20%, Size gets ~12%, Tier gets ~8%, Storage Class gets ~20%
      const nameRatio = 0.4;
      const modifiedRatio = 0.2;
      const sizeRatio = 0.12;
      const tierRatio = 0.08;
      const storageClassRatio = 0.2;

      let nameWidth = Math.floor(availableWidth * nameRatio);
      let modifiedWidth = Math.floor(availableWidth * modifiedRatio);
      let sizeWidth = Math.floor(availableWidth * sizeRatio);
      let tierWidth = Math.floor(availableWidth * tierRatio);
      let storageClassWidth = Math.floor(availableWidth * storageClassRatio);

      // Ensure minimums
      nameWidth = Math.max(minName, nameWidth);
      modifiedWidth = Math.max(minDate, modifiedWidth);
      sizeWidth = Math.max(minSize, sizeWidth);
      tierWidth = Math.max(minTier, tierWidth);
      storageClassWidth = Math.max(minStorageClass, storageClassWidth);

      // Adjust to use full available width
      // Note: tierWidth is calculated but set to 0 in the grid template (column is hidden)
      const total = nameWidth + modifiedWidth + sizeWidth + storageClassWidth; // Don't include tierWidth since it's 0
      if (total > availableWidth) {
        const excess = total - availableWidth;
        // Reduce from name column first (it's the most flexible)
        nameWidth = Math.max(minName, nameWidth - excess);
      } else if (total < availableWidth) {
        // Distribute remaining space to name column to maximize usage
        const remaining = availableWidth - total;
        nameWidth += remaining;
      }

      setColumnWidths({
        name: nameWidth,
        modified: modifiedWidth,
        size: sizeWidth,
        tier: 0, // Tier column is hidden, always set to 0
        'storage-class': storageClassWidth,
      });
    };

    // Calculate initial widths with a small delay to ensure container is rendered
    const timeoutId = setTimeout(() => {
      calculateColumnWidths();
    }, 0);

    // Use ResizeObserver to recalculate when container resizes
    const resizeObserver = new ResizeObserver(() => {
      calculateColumnWidths();
    });

    resizeObserver.observe(listViewRef.current);

    return () => {
      clearTimeout(timeoutId);
      resizeObserver.disconnect();
    };
  }, [columnWidths, viewMode]);

  // Handle sidebar section reordering - prepared for future implementation
  // const handleSectionReorder = useCallback(
  //   (fromIndex: number, toIndex: number) => {
  //     setSidebarSectionOrder((prev) => {
  //       const newOrder = [...prev];
  //       const [removed] = newOrder.splice(fromIndex, 1);
  //       newOrder.splice(toIndex, 0, removed);
  //       // Persist to localStorage
  //       try {
  //         localStorage.setItem(
  //           'diaspor-sidebar-section-order',
  //           JSON.stringify(newOrder),
  //         );
  //       } catch {
  //         // Ignore localStorage errors
  //       }
  //       return newOrder;
  //     });
  //   },
  //   [],
  // );

  // Load sidebar section order from localStorage on mount
  // useEffect(() => {
  //   try {
  //     const saved = localStorage.getItem('diaspor-sidebar-section-order');
  //     if (saved) {
  //       const parsed = JSON.parse(saved);
  //       if (Array.isArray(parsed) && parsed.length > 0) {
  //         setSidebarSectionOrder(parsed);
  //       }
  //     }
  //   } catch {
  //     // Ignore localStorage errors
  //   }
  // }, []);

  // Info modal state
  const [infoModal, setInfoModal] = useState<{
    visible: boolean;
    file: FileMetadata | null;
  }>({
    visible: false,
    file: null,
  });

  // Comment modal state
  const [commentModal, setCommentModal] = useState<{
    visible: boolean;
    file: FileMetadata | null;
  }>({
    visible: false,
    file: null,
  });

  // Spotlight search state - use external control if provided, otherwise internal
  const [internalSpotlightOpen, setInternalSpotlightOpen] = useState(false);
  const spotlightOpen =
    externalSearchOpen !== undefined
      ? externalSearchOpen
      : internalSpotlightOpen;
  const handleCloseSpotlight =
    externalCloseSearch || (() => setInternalSpotlightOpen(false));

  // Navigation history
  const [navigationHistory, setNavigationHistory] = useState<string[]>(['']);
  const [historyIndex, setHistoryIndex] = useState(0);

  // Storage context menu state
  const [storageContextMenu, setStorageContextMenu] = useState<{
    source: StorageSource;
    x: number;
    y: number;
  } | null>(null);

  // Inline renaming state
  const [renamingFile, setRenamingFile] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const renameInputRef = useRef<HTMLInputElement>(null);
  const iconViewRef = useRef<HTMLDivElement>(null);
  const listViewRef = useRef<HTMLDivElement>(null);

  // Drag and drop state
  const [draggedFiles, setDraggedFiles] = useState<string[]>([]);
  const [draggedFileObjects, setDraggedFileObjects] = useState<FileMetadata[]>(
    [],
  );
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const [isDraggingOver, setIsDraggingOver] = useState(false);
  const [dragSourceId, setDragSourceId] = useState<string | null>(null);
  const [nativeFileDropPaths, setNativeFileDropPaths] = useState<string[]>([]);
  // Cross-storage drag state for StorageTierDialog
  const [crossStorageDrag, setCrossStorageDrag] = useState<{
    sourceId: string;
    destSourceId: string;
    paths: string[];
    isMove: boolean;
    destPath?: string;
  } | null>(null);

  // Keyboard shortcut helper
  const shortcutHelper = useKeyboardShortcutHelper();

  // Toast notifications for action feedback
  const toast = useToast();

  // Configurable keyboard shortcuts
  const shortcuts = useKeyboardShortcuts();
  const [showShortcutSettings, setShowShortcutSettings] = useState(false);

  // Collapsed storage groups - start with all groups collapsed to prevent flicker
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
    () =>
      new Set([
        'local',
        'locations',
        'volumes',
        'network',
        'network-shares',
        'cloud',
        'aws',
        'azure',
        'gcp',
        'block',
      ]),
  );

  // Track operation IDs to detect new operations
  // Removed unused refs: trackedOperationIdsRef, isPollingRef
  // Track files being processed to prevent infinite loops
  const processingFilesRef = useRef<Set<string>>(new Set());

  // Note: Removed auto-switch to Operations tab
  // Transfer modal (OperationsPanel) shows progress instead
  // Users can click "View Details" in the modal to navigate to Operations tab if needed
  const toggleGroup = (group: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      const wasCollapsed = next.has(group);
      if (wasCollapsed) {
        next.delete(group);
      } else {
        next.add(group);
      }

      return next;
    });
  };

  // Centralized function to refresh clipboard state
  // Must be defined before clipboard handlers that use it
  const refreshClipboardState = useCallback(async () => {
    try {
      // Check VFS clipboard (pass state to ensure proper initialization)
      const hasVfsFiles = (await invoke('vfs_clipboard_has_files')) as boolean;
      setClipboardHasFiles(Boolean(hasVfsFiles));

      // Also get clipboard content to check if it's a cut operation
      try {
        const clipboardContent = await invoke<{
          operation: string;
          source: string;
          paths: string[];
          file_count: number;
        } | null>('vfs_clipboard_get_content');

        if (clipboardContent) {
          console.log('[Clipboard State] Content:', clipboardContent);
          // Update clipboard state based on actual content
          setClipboardHasFiles(clipboardContent.file_count > 0);

          // Track cut files for visual feedback
          if (clipboardContent.operation === 'cut') {
            setCutFiles(new Set(clipboardContent.paths));
          } else {
            // Clear cut files if it's a copy operation
            setCutFiles(new Set());
          }
        } else {
          // Clear cut files if clipboard is empty
          setCutFiles(new Set());
        }
      } catch (err) {
        // Ignore errors getting clipboard content - has_files is the primary check
        console.debug(
          '[Clipboard State] Could not get clipboard content:',
          err,
        );
      }

      // Check native clipboard for files from system file manager (Finder/Explorer/Nautilus)
      try {
        const nativeFiles = (await invoke(
          'vfs_clipboard_read_native',
        )) as string[];
        setNativeClipboardCount(nativeFiles.length);
        console.log(
          '[Clipboard State] Native clipboard files:',
          nativeFiles.length,
        );

        // Also update clipboardHasFiles if native clipboard has files
        if (nativeFiles.length > 0) {
          setClipboardHasFiles(true);
        }
      } catch (err) {
        console.debug(
          '[Clipboard State] Could not read native clipboard:',
          err,
        );
      }
    } catch (err) {
      console.error('Failed to refresh clipboard state:', err);
      // On error, don't assume clipboard is empty - let user try paste
    }
  }, [setClipboardHasFiles, setCutFiles, setNativeClipboardCount]);

  // =========================================================================
  // Clipboard Operations - Cross-platform file manager clipboard support
  // These handlers must be defined before the keyboard handler useEffect
  //
  // Cross-platform implementation:
  // - All clipboard operations go through Rust/Tauri backend (invoke commands)
  // - Backend handles platform-specific clipboard APIs:
  //   * macOS: NSPasteboard with file URLs
  //   * Windows: PowerShell with CF_HDROP
  //   * Linux: xclip/xsel with text/uri-list
  // - Keyboard shortcuts use e.metaKey || e.ctrlKey (Cmd on macOS, Ctrl on Windows/Linux)
  // - No direct OS clipboard API calls in frontend - all via Tauri
  // =========================================================================

  const handleCopy = useCallback(
    async (targetFile?: FileMetadata) => {
      if (!selectedSource) {
        console.warn('[VFS Copy] No source selected');
        return;
      }

      // Copy should work from any storage type (object storage, filesystem, etc.)
      // The clipboard can handle cross-storage operations

      // Use targetFile if provided, otherwise use selectedFiles
      const pathsToCopy = targetFile
        ? [targetFile.path]
        : Array.from(selectedFiles);

      if (pathsToCopy.length === 0) {
        console.warn('[VFS Copy] No files to copy');
        return;
      }

      // Validate paths
      const validPaths = pathsToCopy.filter((path) => {
        if (!path || typeof path !== 'string' || path.trim() === '') {
          console.warn('[VFS Copy] Invalid path:', path);
          return false;
        }
        return true;
      });

      if (validPaths.length === 0) {
        console.error('[VFS Copy] No valid paths to copy');
        setFileOperation({
          type: 'No valid files selected',
          inProgress: false,
        });
        setTimeout(() => setFileOperation(null), 2000);
        return;
      }

      console.log('[VFS Copy] Starting copy', {
        sourceId: selectedSource.id,
        paths: validPaths,
        targetFile: targetFile?.name,
        originalPathsCount: pathsToCopy.length,
        validPathsCount: validPaths.length,
      });

      setFileOperation({ type: 'Copying to clipboard...', inProgress: true });

      // Timeout is handled by Promise.race below (30 seconds)
      // No need for separate timeout here
      const timeoutId: NodeJS.Timeout | null = null;

      try {
        // Use vfs_clipboard_copy for VFS-to-VFS copy operations
        // This stores files in the VFS clipboard for pasting within VFS
        console.log('[VFS Copy] Calling vfs_clipboard_copy', {
          sourceId: selectedSource.id,
          paths: validPaths,
          pathsCount: validPaths.length,
        });

        // Wrap main copy operation in timeout to prevent hanging
        // Increased timeout to 30 seconds - copy should be fast (just storing paths in memory),
        // but operation tracking or file metadata checks might take time for large directories
        // Tauri automatically converts JavaScript camelCase (sourceId) to Rust snake_case (source_id)
        const copyPromise = invoke('vfs_clipboard_copy', {
          sourceId: selectedSource.id,
          paths: validPaths,
        });

        try {
          await Promise.race([
            copyPromise,
            new Promise<never>((_, reject) =>
              setTimeout(
                () =>
                  reject(
                    new Error('VFS clipboard copy timeout after 30 seconds'),
                  ),
                30000, // 30 second timeout - should be more than enough for clipboard operations
              ),
            ),
          ]);
          console.log('[VFS Copy] vfs_clipboard_copy succeeded');

          // Don't dispatch event here - operation will be created and tracked when paste happens
          // This ensures the operation modal shows the actual copy operation with progress
        } catch (err) {
          console.error('[VFS Copy] vfs_clipboard_copy failed:', err);
          const errorMessage = err instanceof Error ? err.message : String(err);
          console.error('[VFS Copy] Error details:', {
            error: errorMessage,
            source_id: selectedSource.id,
            paths: pathsToCopy,
            validPaths: validPaths,
            errorType: err instanceof Error ? err.constructor.name : typeof err,
          });
          if (timeoutId) {
            clearTimeout(timeoutId);
          }
          setFileOperation({
            type: `Copy failed: ${errorMessage}`,
            inProgress: false,
          });
          setTimeout(() => setFileOperation(null), 5000);
          DialogService.error(
            'Copy Failed',
            `Failed to copy files to clipboard: ${errorMessage}`,
          );
          return;
        }

        // Also copy to native clipboard for compatibility with Finder
        // This allows pasting VFS files to native filesystem if needed
        // Use a shorter timeout for native clipboard as it's optional
        // Tauri automatically converts JavaScript camelCase (sourceId) to Rust snake_case (source_id)
        try {
          const nativePromise = invoke('vfs_clipboard_copy_for_native', {
            sourceId: selectedSource.id,
            paths: validPaths,
          });

          // Race between native clipboard and a 5 second timeout
          await Promise.race([
            nativePromise,
            new Promise((_, reject) =>
              setTimeout(
                () => reject(new Error('Native clipboard timeout')),
                5000,
              ),
            ),
          ]);
        } catch (nativeErr) {
          // Log but don't fail - native clipboard is optional
          console.debug(
            '[VFS Copy] Native clipboard copy failed (non-critical):',
            nativeErr,
          );
        }

        // Timeout is handled by Promise.race, no need to clear here

        // Brief delay to ensure backend clipboard is set before refreshing state
        await new Promise((resolve) => setTimeout(resolve, 200));

        // Verify clipboard was actually set by checking directly
        try {
          const hasFiles = (await invoke('vfs_clipboard_has_files')) as boolean;
          console.log(
            '[VFS Copy] Clipboard verification - hasFiles:',
            hasFiles,
          );
          if (!hasFiles) {
            console.error(
              '[VFS Copy] WARNING: Clipboard is empty after copy operation!',
            );
            setFileOperation({
              type: 'Copy failed: Clipboard was not set',
              inProgress: false,
            });
            setTimeout(() => setFileOperation(null), 3000);
            return;
          }
        } catch (verifyErr) {
          console.error('[VFS Copy] Failed to verify clipboard:', verifyErr);
          setFileOperation({
            type: 'Copy may have failed - please verify',
            inProgress: false,
          });
          setTimeout(() => setFileOperation(null), 3000);
          return;
        }

        // Refresh clipboard state to ensure UI updates (with timeout)
        try {
          await Promise.race([
            refreshClipboardState(),
            new Promise((_, reject) =>
              setTimeout(() => reject(new Error('Refresh timeout')), 2000),
            ),
          ]);
          console.log('[VFS Copy] Clipboard state refreshed successfully');
        } catch (refreshErr) {
          console.warn(
            '[VFS Copy] Refresh clipboard state failed (non-critical):',
            refreshErr,
          );
          // Manually set clipboard state since refresh failed but we verified it's set
          setClipboardHasFiles(true);
        }

        // Brief visual feedback
        setFileOperation({
          type: `${validPaths.length} item(s) copied`,
          inProgress: false,
        });
        setTimeout(() => setFileOperation(null), 1500);
      } catch (err) {
        // Timeout is handled by Promise.race, no need to clear here

        console.error('[VFS Copy] Failed:', err);
        const errorMessage = err instanceof Error ? err.message : String(err);

        // Check if it's a timeout error
        if (errorMessage.includes('timeout')) {
          // Timeout - clipboard might still be set, try to verify
          console.warn(
            '[VFS Copy] Operation timed out, but clipboard may still be set',
          );
          try {
            await refreshClipboardState();
            setFileOperation({
              type: 'Copy operation timed out, but clipboard may be set',
              inProgress: false,
            });
          } catch {
            setFileOperation({
              type: 'Copy operation timed out',
              inProgress: false,
            });
          }
        } else {
          setFileOperation({
            type: `Copy failed: ${errorMessage}`,
            inProgress: false,
          });
        }
        setTimeout(() => setFileOperation(null), 2000);
      }
    },
    [selectedSource, selectedFiles, refreshClipboardState],
  );

  const handleCut = useCallback(
    async (targetFile?: FileMetadata) => {
      if (!selectedSource) {
        console.warn('[VFS Cut] No source selected');
        return;
      }

      // Cut works on all storage types (local, network, cloud object storage)
      // Object storage supports cut via clipboard operations

      // Use targetFile if provided, otherwise use selectedFiles
      const pathsToCut = targetFile
        ? [targetFile.path]
        : Array.from(selectedFiles);

      // Validate paths
      const validPaths = pathsToCut.filter((path) => {
        if (!path || typeof path !== 'string' || path.trim() === '') {
          console.warn('[VFS Cut] Invalid path:', path);
          return false;
        }
        return true;
      });

      if (validPaths.length === 0) {
        console.error('[VFS Cut] No valid paths to cut');
        setFileOperation({
          type: 'No valid files selected',
          inProgress: false,
        });
        setTimeout(() => setFileOperation(null), 2000);
        return;
      }

      console.log('[VFS Cut] Starting cut', {
        sourceId: selectedSource.id,
        paths: validPaths,
        targetFile: targetFile?.name,
        originalPathsCount: pathsToCut.length,
        validPathsCount: validPaths.length,
      });

      setFileOperation({ type: 'Cutting to clipboard...', inProgress: true });

      // Add timeout to prevent hanging
      const timeoutId = setTimeout(() => {
        console.error('[VFS Cut] Operation timed out after 10 seconds');
        setFileOperation({
          type: 'Cut timed out - please try again',
          inProgress: false,
        });
        setTimeout(() => setFileOperation(null), 2000);
      }, 10000);

      try {
        console.log('[VFS Cut] Calling vfs_clipboard_cut', {
          sourceId: selectedSource.id,
          paths: validPaths,
          pathsCount: validPaths.length,
        });

        // Use snake_case to match Rust parameter name exactly
        // Tauri automatically converts JavaScript camelCase (sourceId) to Rust snake_case (source_id)
        const cutPromise = invoke('vfs_clipboard_cut', {
          sourceId: selectedSource.id,
          paths: validPaths,
        });

        try {
          const cutResult = await Promise.race([
            cutPromise,
            new Promise<never>((_, reject) =>
              setTimeout(
                () =>
                  reject(
                    new Error('VFS clipboard cut timeout after 30 seconds'),
                  ),
                30000,
              ),
            ),
          ]);
          console.log('[VFS Cut] vfs_clipboard_cut succeeded', {
            result: cutResult,
            resultType: typeof cutResult,
          });
        } catch (cutErr) {
          console.error('[VFS Cut] vfs_clipboard_cut failed:', cutErr);
          const errorMessage =
            cutErr instanceof Error ? cutErr.message : String(cutErr);
          console.error('[VFS Cut] Error details:', {
            error: errorMessage,
            source_id: selectedSource.id,
            paths: pathsToCut,
            validPaths: validPaths,
            errorType:
              cutErr instanceof Error ? cutErr.constructor.name : typeof cutErr,
          });
          clearTimeout(timeoutId);
          setFileOperation({
            type: `Cut failed: ${errorMessage}`,
            inProgress: false,
          });
          setTimeout(() => setFileOperation(null), 5000);
          DialogService.error(
            'Cut Failed',
            `Failed to cut files to clipboard: ${errorMessage}`,
          );
          return;
        }

        // Don't dispatch event here - operation will be created and tracked when paste happens
        // This ensures the operation modal shows the actual move operation with progress

        // Clear timeout since we succeeded
        clearTimeout(timeoutId);

        // Brief delay to ensure backend clipboard is set before refreshing state
        await new Promise((resolve) => setTimeout(resolve, 200));

        // Verify clipboard was actually set by checking directly
        try {
          const hasFiles = (await invoke('vfs_clipboard_has_files')) as boolean;
          console.log('[VFS Cut] Clipboard verification - hasFiles:', hasFiles);
          if (!hasFiles) {
            console.error(
              '[VFS Cut] WARNING: Clipboard is empty after cut operation!',
            );
            setFileOperation({
              type: 'Cut failed: Clipboard was not set',
              inProgress: false,
            });
            setTimeout(() => setFileOperation(null), 3000);
            return;
          }
        } catch (verifyErr) {
          console.error('[VFS Cut] Failed to verify clipboard:', verifyErr);
          setFileOperation({
            type: 'Cut may have failed - please verify',
            inProgress: false,
          });
          setTimeout(() => setFileOperation(null), 3000);
          return;
        }

        // Refresh clipboard state to ensure UI updates (with timeout)
        try {
          await Promise.race([
            refreshClipboardState(),
            new Promise((_, reject) =>
              setTimeout(() => reject(new Error('Refresh timeout')), 2000),
            ),
          ]);
          console.log('[VFS Cut] Clipboard state refreshed successfully');
        } catch (refreshErr) {
          console.warn(
            '[VFS Cut] Refresh clipboard state failed (non-critical):',
            refreshErr,
          );
          // Manually set clipboard state since refresh failed but we verified it's set
          setClipboardHasFiles(true);
        }

        // Brief visual feedback
        setFileOperation({
          type: `${validPaths.length} item(s) cut`,
          inProgress: false,
        });
        setTimeout(() => setFileOperation(null), 1500);
      } catch (err) {
        // Clear timeout on error
        clearTimeout(timeoutId);

        console.error('[VFS Cut] Failed:', err);
        const errorMessage = err instanceof Error ? err.message : String(err);
        setFileOperation({
          type: `Cut failed: ${errorMessage}`,
          inProgress: false,
        });
        setTimeout(() => setFileOperation(null), 2000);
      }
    },
    [selectedSource, selectedFiles, refreshClipboardState],
  );

  const handlePaste = useCallback(
    async (targetPath?: string) => {
      console.log('[VFS Paste] handlePaste called', {
        hasSelectedSource: !!selectedSource,
        selectedSourceId: selectedSource?.id,
        targetPath,
        currentPath,
      });

      if (!selectedSource) {
        console.warn('[VFS Paste] No source selected');
        DialogService.error(
          'No storage source selected',
          'Please select a storage source before pasting.',
        );
        return;
      }

      // Paste should work to any storage type that supports writing
      // Object storage supports paste via upload, filesystem supports direct paste

      // Normalize destination path
      const destination = targetPath || currentPath || '/';
      const normalizedDestination = destination.trim() || '/';

      console.log(
        '[VFS Paste] Starting paste to:',
        normalizedDestination,
        'in source:',
        selectedSource.id,
      );

      setFileOperation({ type: 'Pasting...', inProgress: true });

      // Get clipboard content BEFORE paste (paste may clear clipboard for cut operations)
      // Declare and initialize outside try block so it's accessible in catch block
      let clipboardOperationType: 'copy' | 'cut' = 'copy'; // Default to 'copy'

      try {
        // Check clipboard state before attempting paste
        console.log('[VFS Paste] Checking clipboard state...');
        const hasFiles = (await invoke('vfs_clipboard_has_files')) as boolean;
        console.log('[VFS Paste] Clipboard has files:', hasFiles);

        if (!hasFiles) {
          setFileOperation({
            type: 'No files in clipboard',
            inProgress: false,
          });
          setTimeout(() => setFileOperation(null), 2000);
          DialogService.info(
            'Clipboard is empty',
            'Copy or cut files first, then paste.',
          );
          return;
        }

        // Get clipboard content BEFORE paste (paste may clear clipboard for cut operations)
        let clipboardContent: {
          operation: string;
          source: string;
          paths: string[];
        } | null = null;
        try {
          clipboardContent = await invoke<{
            operation: string;
            source: string;
            paths: string[];
          } | null>('vfs_clipboard_get_content');
          clipboardOperationType =
            clipboardContent?.operation === 'cut' ? 'cut' : 'copy';
        } catch (err) {
          console.warn('[VFS Paste] Could not get clipboard content:', err);
          // Keep default 'copy' if we can't determine
        }

        // Paste operation - backend handles both VFS and native clipboard
        console.log('[VFS Paste] Starting paste operation', {
          dest_source_id: selectedSource.id,
          dest_path: normalizedDestination,
          clipboardOperationType,
          timestamp: new Date().toISOString(),
        });

        // Calculate dynamic timeout based on file count
        // Estimate: ~30 seconds per file (accounts for large files, network delays, etc.)
        // Minimum: 1 minute for small operations
        // Maximum: 24 hours for very large operations (TB-scale transfers)
        const fileCount = clipboardContent?.paths?.length || 1;
        const estimatedSecondsPerFile = 30;
        const minTimeoutMs = 60 * 1000; // 1 minute minimum
        const maxTimeoutMs = 24 * 60 * 60 * 1000; // 24 hours maximum
        const calculatedTimeoutMs = Math.min(
          maxTimeoutMs,
          Math.max(minTimeoutMs, fileCount * estimatedSecondsPerFile * 1000),
        );

        console.log('[VFS Paste] Calculated timeout:', {
          fileCount,
          estimatedSecondsPerFile,
          calculatedTimeoutMs,
          calculatedTimeoutMinutes: Math.round(calculatedTimeoutMs / 60000),
        });

        // Tauri automatically converts JavaScript camelCase to Rust snake_case
        const pastePromise = invoke('vfs_clipboard_paste_to_vfs', {
          destSourceId: selectedSource.id,
          destPath: normalizedDestination,
        }).catch((error) => {
          console.error('[VFS Paste] Paste promise rejected:', error);
          // Check if error contains operation_id (some errors might include it)
          if (error && typeof error === 'object' && 'operation_id' in error) {
            console.log(
              '[VFS Paste] Error contains operation_id:',
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              (error as any).operation_id,
            );
          }
          throw error;
        });

        // Use dynamic timeout based on file count - scales from 1 minute to 24 hours
        // Note: Operations are tracked via operation_id and shown in OperationsPanel,
        // so this timeout is just a safety net to prevent Promise from hanging forever
        const pasteTimeout = calculatedTimeoutMs;

        console.log('[VFS Paste] Waiting for paste operation to complete...');
        let result: {
          files_pasted: number;
          files_failed: number;
          pasted_paths?: string[];
          errors?: string[];
          operation_id?: string;
        };

        try {
          result = (await Promise.race([
            pastePromise,
            new Promise((_, reject) =>
              setTimeout(
                () =>
                  reject(
                    new Error(
                      `Paste operation timed out after ${Math.round(pasteTimeout / 60000)} minutes. The operation may still be in progress - check the Operations panel for status.`,
                    ),
                  ),
                pasteTimeout,
              ),
            ),
          ])) as typeof result;
          console.log('[VFS Paste] Paste operation completed:', result);
        } catch (pasteError) {
          console.error('[VFS Paste] Paste operation failed:', pasteError);

          // Extract operation_id from error message if present (backend appends it on failure)
          const errorMessage =
            pasteError instanceof Error
              ? pasteError.message
              : String(pasteError);
          let operationId: string | undefined;
          if (errorMessage.includes('|OPERATION_ID:')) {
            const match = errorMessage.match(/\|OPERATION_ID:([^|]+)/);
            if (match) {
              operationId = match[1];
              console.log(
                '[VFS Paste] Extracted operation_id from error:',
                operationId,
              );
            }
          }

          // Dispatch event even for failed operations so they appear in OperationsPanel
          if (operationId) {
            try {
              const eventName =
                clipboardOperationType === 'cut'
                  ? 'move-started'
                  : 'copy-started';
              const event = new CustomEvent(eventName, {
                detail: { operationId },
              });
              console.log(
                '[VFS Paste] Dispatching event for failed operation:',
                eventName,
                'with operationId:',
                operationId,
              );
              window.dispatchEvent(event);
            } catch (eventError) {
              console.error(
                '[VFS Paste] Failed to dispatch event for failed operation:',
                eventError,
              );
            }
          }

          throw pasteError;
        }

        // Paste operations always create an operation_id for tracking
        // Dispatch appropriate event based on clipboard operation type
        // Use the operation type we got BEFORE paste (paste may have cleared clipboard)
        // Copy operations should show as copy-started, cut operations as move-started
        // IMPORTANT: Dispatch event IMMEDIATELY (not in setTimeout) so modal shows operation
        // even if it completes quickly
        if (result.operation_id) {
          console.log(
            '[VFS Paste] ✅ Operation tracked:',
            result.operation_id,
            {
              operationType:
                clipboardOperationType === 'cut'
                  ? 'move-started'
                  : 'copy-started',
              filesPasted: result.files_pasted,
              filesFailed: result.files_failed,
              clipboardOperationType,
            },
          );

          // Dispatch event immediately to trigger OperationsModal/OperationsPanel to show the operation
          // This ensures the modal appears even for fast operations
          try {
            const eventName =
              clipboardOperationType === 'cut'
                ? 'move-started'
                : 'copy-started';
            const event = new CustomEvent(eventName, {
              detail: { operationId: result.operation_id },
            });

            console.log(
              '[VFS Paste] 📤 Dispatching event:',
              eventName,
              'with operationId:',
              result.operation_id,
              'event detail:',
              event.detail,
            );
            window.dispatchEvent(event);
            console.log('[VFS Paste] ✅ Event dispatched successfully');
          } catch (eventError) {
            console.error(
              '[VFS Paste] ❌ Failed to dispatch event:',
              eventError,
            );
          }

          // Don't auto-switch - transfer modal (OperationsPanel) will show progress
          setFileOperation(null);
        } else {
          // Fallback: show simple status if no operation tracking (shouldn't happen)
          console.warn(
            '[VFS Paste] No operation_id returned from paste operation. Result:',
            result,
          );
          setFileOperation({
            type:
              result.files_failed > 0
                ? `Pasted ${result.files_pasted} item(s), ${result.files_failed} failed`
                : `Pasted ${result.files_pasted} item(s)`,
            inProgress: false,
          });
          setTimeout(() => setFileOperation(null), 2000);
        }

        // Refresh file list after paste completes
        const refreshPath = targetPath || currentPath || '';
        const normalizedRefreshPath = refreshPath.trim();

        // Single refresh after a short delay to allow backend to complete
        setTimeout(async () => {
          try {
            await loadFilesList(selectedSource.id, normalizedRefreshPath);
          } catch (refreshErr) {
            console.error('[VFS Paste] Refresh failed:', refreshErr);
          }
        }, 500);

        // Update UI based on result
        if (result.files_pasted > 0) {
          const successMessage = `${result.files_pasted} item(s) pasted successfully${result.files_failed > 0 ? ` (${result.files_failed} failed)` : ''}`;
          setFileOperation({
            type: successMessage,
            inProgress: false,
          });
          setTimeout(() => setFileOperation(null), 2500);

          // Show success dialog if some files failed
          if (
            result.files_failed > 0 &&
            result.errors &&
            result.errors.length > 0
          ) {
            const errorDetails = result.errors.slice(0, 3).join('\n');
            DialogService.warning(
              'Paste completed with errors',
              `${result.files_pasted} files pasted successfully, but ${result.files_failed} failed:\n\n${errorDetails}${result.errors.length > 3 ? `\n... and ${result.errors.length - 3} more errors` : ''}`,
            );
          }
        } else if (result.errors && result.errors.length > 0) {
          const errorMessage =
            result.errors.length === 1
              ? result.errors[0]
              : `${result.errors.length} errors occurred. First: ${result.errors[0]}`;
          setFileOperation({
            type: `Paste failed: ${errorMessage}`,
            inProgress: false,
          });
          setTimeout(() => setFileOperation(null), 3000);

          // Show detailed error dialog
          const errorDetails = result.errors.slice(0, 5).join('\n');
          DialogService.error(
            'Paste failed',
            `Failed to paste files:\n\n${errorDetails}${result.errors.length > 5 ? `\n... and ${result.errors.length - 5} more errors` : ''}`,
          );
        } else {
          setFileOperation({
            type: 'No files were pasted',
            inProgress: false,
          });
          setTimeout(() => setFileOperation(null), 2000);
          DialogService.info(
            'Paste completed',
            'No files were pasted. Clipboard may be empty or files may not be accessible.',
          );
        }

        // Clear cut files visual feedback after successful paste
        // For cut operations, clipboard is cleared by backend after paste
        // So we should clear the visual feedback immediately
        if (result.files_pasted > 0) {
          setCutFiles(new Set());
          // Also clear clipboard state since cut operations clear the clipboard
          if (clipboardOperationType === 'cut') {
            setClipboardHasFiles(false);
          }
        }

        // Refresh clipboard state after paste (especially important for cut operations)
        // This will detect if clipboard was cleared (for cut operations)
        await refreshClipboardState();

        // If paste was successful, clipboard might be cleared (for cut operations)
        // Refresh again after a short delay to ensure state is updated
        setTimeout(async () => {
          await refreshClipboardState();
        }, 500);
      } catch (err) {
        console.error('[VFS Paste] Exception:', err);
        const errorMsg = err instanceof Error ? err.message : String(err);

        // Extract operation_id from error message if present
        // Format: "error_message|OPERATION_ID:operation_id"
        let operationId: string | undefined;
        if (errorMsg.includes('|OPERATION_ID:')) {
          const parts = errorMsg.split('|OPERATION_ID:');
          operationId = parts[1]?.trim();
          console.log(
            '[VFS Paste] Extracted operation_id from error:',
            operationId,
          );

          // Dispatch event even for failed operations so they appear in OperationsPanel
          if (operationId) {
            try {
              const eventName =
                clipboardOperationType === 'cut'
                  ? 'move-started'
                  : 'copy-started';
              const event = new CustomEvent(eventName, {
                detail: { operationId },
              });
              console.log(
                '[VFS Paste] Dispatching event for failed operation:',
                eventName,
                'with operationId:',
                operationId,
              );
              window.dispatchEvent(event);
            } catch (eventError) {
              console.error(
                '[VFS Paste] Failed to dispatch event for failed operation:',
                eventError,
              );
            }
          }
        }

        // Provide user-friendly error messages
        // Remove operation_id from error message for display
        const cleanErrorMsg = errorMsg.split('|OPERATION_ID:')[0];
        let userFriendlyError = cleanErrorMsg;
        if (cleanErrorMsg.includes('timeout')) {
          userFriendlyError =
            'Paste operation timed out. The files may still be pasting in the background.';
        } else if (
          cleanErrorMsg.includes('Permission Denied') ||
          cleanErrorMsg.includes('Operation not permitted')
        ) {
          userFriendlyError =
            'Permission denied. Please check file permissions and try again.';
        } else if (cleanErrorMsg.includes('Clipboard is empty')) {
          userFriendlyError = 'Clipboard is empty. Copy or cut files first.';
        } else if (cleanErrorMsg.includes('not initialized')) {
          userFriendlyError =
            'File operations not available. Please restart the application.';
        }

        setFileOperation({
          type: `Paste failed: ${userFriendlyError}`,
          inProgress: false,
        });
        setTimeout(() => setFileOperation(null), 3000);

        // Show error dialog for critical errors
        if (!cleanErrorMsg.includes('timeout')) {
          DialogService.error('Paste failed', userFriendlyError);
        }

        // Still refresh clipboard state even on error
        await refreshClipboardState();
      }
    },
    [selectedSource, currentPath, refreshClipboardState],
  );

  // Initialize
  useEffect(() => {
    initAndLoadSources();

    const unlisten = listen<WarmProgress>('warm-progress', (event) => {
      setWarmProgress((prev) => ({
        ...prev,
        [event.payload.filePath]: event.payload,
      }));
    });

    // Keyboard shortcuts
    const handleKeyDown = async (e: KeyboardEvent) => {
      const isMeta = e.metaKey || e.ctrlKey;

      // Context menu keyboard shortcut (Shift+F10 or Menu key) - show context menu for selected file
      if ((e.shiftKey && e.key === 'F10') || e.key === 'ContextMenu') {
        e.preventDefault();
        // If a file is selected, show context menu for it
        if (selectedFiles.size > 0 && selectedSource) {
          const selectedPath = Array.from(selectedFiles)[0];
          const selectedFile = files.find((f) => f.path === selectedPath);
          if (selectedFile) {
            // Get the file item element position
            const fileItemElement = document.querySelector(
              `[data-path="${selectedPath}"]`,
            ) as HTMLElement;
            if (fileItemElement) {
              const rect = fileItemElement.getBoundingClientRect();
              const syntheticEvent = {
                clientX: rect.left + rect.width / 2,
                clientY: rect.top + rect.height / 2,
                preventDefault: () => {
                  // Empty function for synthetic event
                },
                stopPropagation: () => {
                  // Empty function for synthetic event
                },
              } as React.MouseEvent;
              handleContextMenu(syntheticEvent, selectedFile);
            }
          }
        }
        return;
      }

      // Escape - cancel rename, clear cut state, deselect, or close modals
      if (e.key === 'Escape') {
        e.preventDefault();
        if (renamingFile) {
          cancelRename();
          return;
        }
        // Close modals first (if any are open)
        if (infoModal.visible) {
          setInfoModal({ visible: false, file: null });
          return;
        }
        if (showAddStorage) {
          setShowAddStorage(false);
          return;
        }
        if (showTierDialog) {
          setShowTierDialog(false);
          return;
        }
        if (showShortcutSettings) {
          setShowShortcutSettings(false);
          return;
        }
        if (spotlightOpen) {
          if (externalCloseSearch) {
            externalCloseSearch();
          } else {
            setInternalSpotlightOpen(false);
          }
          return;
        }
        // Close context menu if open
        if (contextMenu.visible) {
          setContextMenu({ visible: false, x: 0, y: 0, targetFile: undefined });
          return;
        }
        // Clear selection
        setSelectedFiles(new Set());
        return;
      }

      // Navigation shortcuts (work even without source)
      if (isMeta && e.key === '[') {
        e.preventDefault();
        await goBack();
        return;
      } else if (isMeta && e.key === ']') {
        e.preventDefault();
        await goForward();
        return;
      } else if (isMeta && e.key === 'ArrowUp') {
        e.preventDefault();
        await goUp();
        return;
      }

      if (!selectedSource) return;

      // Select All - Cmd/Ctrl+A
      // Only intercept if NOT in a text input, textarea, or contenteditable element
      if (shortcuts.matchesShortcut(e, 'select-all')) {
        const activeElement = document.activeElement;
        const isTextInput =
          activeElement &&
          (activeElement.tagName === 'INPUT' ||
            activeElement.tagName === 'TEXTAREA' ||
            activeElement.getAttribute('contenteditable') === 'true' ||
            activeElement.getAttribute('contenteditable') === '');

        // If in a text input, let the browser handle it (select all text)
        if (isTextInput) {
          return; // Don't prevent default, let browser handle text selection
        }

        // Otherwise, select all files
        e.preventDefault();
        setSelectedFiles(new Set(files.map((f) => f.path)));
        toast.showActionToast(
          `Selected ${files.length} items`,
          shortcuts.formatShortcut('select-all'),
        );
        return;
      }

      // New Folder - Cmd/Ctrl+Shift+N
      if (shortcuts.matchesShortcut(e, 'new-folder')) {
        e.preventDefault();
        toast.showActionToast(
          'New Folder',
          shortcuts.formatShortcut('new-folder'),
        );
        handleNewFolder();
        return;
      }

      // Asset Details - Cmd/Ctrl+I
      if (
        shortcuts.matchesShortcut(e, 'get-info') &&
        selectedFiles.size === 1
      ) {
        e.preventDefault();
        const selectedPath = Array.from(selectedFiles)[0];
        const selectedFile = files.find((f) => f.path === selectedPath);
        if (selectedFile) {
          toast.showActionToast(
            'Asset Details',
            shortcuts.formatShortcut('get-info'),
          );
          setInfoModal({ visible: true, file: selectedFile });
        }
        return;
      }

      // Rename - F2 (Windows) or Enter when already selected (we'll use F2 for all OS)
      // Only works for single file selection - bulk rename not supported
      if (e.key === 'F2' && selectedFiles.size === 1) {
        // Don't trigger rename if we're in a modal
        const activeElement = document.activeElement;
        const isInModal =
          activeElement &&
          (activeElement.closest('[role="dialog"]') ||
            activeElement.closest('.modal') ||
            activeElement.closest('[data-modal]') ||
            activeElement.closest('.add-storage-modal'));

        if (!isInModal) {
          e.preventDefault();
          const selectedPath = Array.from(selectedFiles)[0];
          const selectedFile = files.find((f) => f.path === selectedPath);
          if (selectedFile) {
            handleRename(selectedFile);
          }
        }
        return;
      }

      // Show toast if user tries to rename multiple files (not supported)
      if (e.key === 'F2' && selectedFiles.size > 1) {
        e.preventDefault();
        toast.showToast({
          message:
            'Rename only works for single file. Select one file to rename.',
          type: 'warning',
        });
        return;
      }

      // Quick Look / Preview - Space
      if (e.key === ' ' && selectedFiles.size === 1) {
        // Check if we're in a text input, button, or modal - don't capture Space in those cases
        const activeElement = document.activeElement;
        const isInteractiveElement =
          activeElement &&
          (activeElement.tagName === 'INPUT' ||
            activeElement.tagName === 'TEXTAREA' ||
            activeElement.tagName === 'BUTTON' ||
            activeElement.tagName === 'SELECT' ||
            activeElement.getAttribute('contenteditable') === 'true');
        const isInModal =
          activeElement &&
          (activeElement.closest('[role="dialog"]') ||
            activeElement.closest('.modal') ||
            activeElement.closest('[data-modal]') ||
            activeElement.closest('.add-storage-modal'));

        if (!isInteractiveElement && !isInModal) {
          e.preventDefault();
          const selectedPath = Array.from(selectedFiles)[0];
          const selectedFile = files.find((f) => f.path === selectedPath);
          if (selectedFile) {
            setInfoModal({ visible: true, file: selectedFile });
          }
        }
        return;
      }

      // Refresh - Cmd/Ctrl+R or F5
      if ((isMeta && e.key === 'r') || e.key === 'F5') {
        e.preventDefault();
        if (selectedSource) {
          toast.showActionToast('Refreshing...', isMeta ? '⌘R' : 'F5');
          loadFilesList(selectedSource.id, currentPath);
        }
        return;
      }

      // File operation shortcuts
      if (shortcuts.matchesShortcut(e, 'copy')) {
        // Check if user is typing in a text input field - allow native copy for text
        const activeElement = document.activeElement;
        const isTextInput =
          activeElement &&
          (activeElement.tagName === 'INPUT' ||
            activeElement.tagName === 'TEXTAREA' ||
            activeElement.getAttribute('contenteditable') === 'true' ||
            (activeElement as HTMLElement).isContentEditable);

        // Only handle file copy if files are selected AND NOT in a text input field
        if (selectedFiles.size > 0 && !isTextInput) {
          e.preventDefault();
          e.stopPropagation();
          console.log('[Keyboard] Copy shortcut triggered', {
            selectedFiles: Array.from(selectedFiles),
          });
          toast.showActionToast(
            `Copied ${selectedFiles.size} item(s)`,
            shortcuts.formatShortcut('copy'),
          );
          await handleCopy();
        } else if (selectedFiles.size === 0) {
          console.log(
            '[Keyboard] Copy shortcut triggered but no files selected',
          );
        }
        // If in text input, let the default copy behavior proceed (don't preventDefault)
      } else if (shortcuts.matchesShortcut(e, 'cut')) {
        // Check if user is typing in a text input field - allow native cut for text
        const activeElement = document.activeElement;
        const isTextInput =
          activeElement &&
          (activeElement.tagName === 'INPUT' ||
            activeElement.tagName === 'TEXTAREA' ||
            activeElement.getAttribute('contenteditable') === 'true' ||
            (activeElement as HTMLElement).isContentEditable);

        // Only handle file cut if files are selected AND NOT in a text input field
        if (selectedFiles.size > 0 && !isTextInput) {
          e.preventDefault();
          e.stopPropagation();
          console.log('[Keyboard] Cut shortcut triggered', {
            selectedFiles: Array.from(selectedFiles),
          });
          toast.showActionToast(
            `Cut ${selectedFiles.size} item(s)`,
            shortcuts.formatShortcut('cut'),
          );
          await handleCut();
        } else if (selectedFiles.size === 0) {
          console.log(
            '[Keyboard] Cut shortcut triggered but no files selected',
          );
        }
        // If in text input, let the default cut behavior proceed (don't preventDefault)
      } else if (shortcuts.matchesShortcut(e, 'paste')) {
        // Check if user is typing in a text input field - allow native paste for text
        const activeElement = document.activeElement;
        const isTextInput =
          activeElement &&
          (activeElement.tagName === 'INPUT' ||
            activeElement.tagName === 'TEXTAREA' ||
            activeElement.getAttribute('contenteditable') === 'true' ||
            (activeElement as HTMLElement).isContentEditable);

        // Only handle file paste if NOT in a text input field AND clipboard has files.
        // Silent no-op when clipboard is empty — matches the grayed-out menu item.
        if (!isTextInput && clipboardHasFiles) {
          e.preventDefault();
          e.stopPropagation();
          console.log('[Keyboard] Paste shortcut triggered');
          toast.showActionToast('Paste', shortcuts.formatShortcut('paste'));
          // If context menu has a folder target, paste into it
          const targetPath =
            contextMenu.targetFile &&
            (contextMenu.targetFile.mimeType === 'folder' ||
              contextMenu.targetFile.isDirectory)
              ? contextMenu.targetFile.path
              : undefined;
          await handlePaste(targetPath);
        } else if (!isTextInput && !clipboardHasFiles) {
          // Swallow the keystroke so the OS doesn't try to paste raw text
          // into the file area, but don't toast — silent gate per UX spec.
          e.preventDefault();
          e.stopPropagation();
          console.log('[Keyboard] Paste shortcut ignored: clipboard empty');
        }
        // If in text input, let the default paste behavior proceed (don't preventDefault)
      } else if (
        shortcuts.matchesShortcut(e, 'delete') &&
        selectedFiles.size > 0
      ) {
        // Check if user is typing in a text input field - allow native delete/backspace for text
        const activeElement = document.activeElement;
        const isTextInput =
          activeElement &&
          (activeElement.tagName === 'INPUT' ||
            activeElement.tagName === 'TEXTAREA' ||
            activeElement.getAttribute('contenteditable') === 'true' ||
            (activeElement as HTMLElement).isContentEditable ||
            activeElement.classList.contains('rename-input'));

        // Also check if we're inside a modal (dialog, modal overlay, etc.)
        const isInModal =
          activeElement &&
          (activeElement.closest('[role="dialog"]') ||
            activeElement.closest('.modal') ||
            activeElement.closest('[data-modal]') ||
            activeElement.closest('.add-storage-modal') ||
            activeElement.closest('.info-modal'));

        // Don't trigger delete if renaming a file
        const isRenaming = renamingFile !== null;

        // Only handle file delete if NOT in a text input field, NOT in a modal, and NOT renaming
        if (!isTextInput && !isInModal && !isRenaming) {
          e.preventDefault();
          await handleDelete();
        }
        // If in text input or modal, let the default delete/backspace behavior proceed
      } else if (
        shortcuts.matchesShortcut(e, 'open') &&
        selectedFiles.size === 1
      ) {
        // Check if we're in a text input or modal - don't capture Enter in those cases
        const activeElement = document.activeElement;
        const isTextInput =
          activeElement &&
          (activeElement.tagName === 'INPUT' ||
            activeElement.tagName === 'TEXTAREA' ||
            activeElement.tagName === 'BUTTON' ||
            activeElement.getAttribute('contenteditable') === 'true');
        const isInModal =
          activeElement &&
          (activeElement.closest('[role="dialog"]') ||
            activeElement.closest('.modal') ||
            activeElement.closest('[data-modal]') ||
            activeElement.closest('.add-storage-modal'));

        // Enter to open folder or file - but not in text inputs or modals
        if (!isTextInput && !isInModal) {
          e.preventDefault();
          const selectedPath = Array.from(selectedFiles)[0];
          const selectedFile = files.find((f) => f.path === selectedPath);
          if (selectedFile) {
            handleFileDoubleClick(selectedFile);
          }
        }
      } else if (
        shortcuts.matchesShortcut(e, 'toggle-favorite') &&
        selectedFiles.size > 0
      ) {
        // Cmd+Shift+F to toggle favorite
        e.preventDefault();
        const selectedPath = Array.from(selectedFiles)[0];
        if (selectedPath) {
          await handleToggleFavorite(selectedPath);
        }
      } else if (
        shortcuts.matchesShortcut(e, 'download') &&
        selectedFiles.size === 1 &&
        isObjectStorage(selectedSource)
      ) {
        // Cmd+D to download (object storage only, but duplicate takes precedence)
        // Note: Duplicate handler above takes precedence, so this only triggers
        // if duplicate didn't match (e.g., directory selected or duplicate failed)
        e.preventDefault();
        e.stopPropagation();
        const selectedPath = Array.from(selectedFiles)[0];
        const selectedFile = files.find((f) => f.path === selectedPath);
        if (selectedFile && !selectedFile.isDirectory) {
          await handleDownloadFile(selectedFile);
        }
      }

      // Spotlight Search - Cmd/Ctrl+K
      if (shortcuts.matchesShortcut(e, 'spotlight')) {
        e.preventDefault();
        e.stopPropagation();
        if (_onOpenSearch) {
          // Use external handler if provided
          _onOpenSearch();
        } else {
          // Use internal state if no external control
          setInternalSpotlightOpen(true);
        }
        return; // Exit early to prevent other handlers
      }
    };

    window.addEventListener('keydown', handleKeyDown);

    return () => {
      unlisten.then((fn) => fn());
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [
    selectedSource,
    selectedFiles,
    currentPath,
    files,
    renamingFile,
    handleCopy,
    handleCut,
    handlePaste,
    shortcuts,
    contextMenu,
    toast,
    clipboardHasFiles,
  ]);

  // Listen for Tauri file-drop events to capture native file paths
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupFileDropListener = async () => {
      try {
        // Listen for file-drop events from Tauri
        unlisten = await listen<string[]>('tauri://file-drop', (event) => {
          // Tauri provides an array of file paths
          const paths = event.payload || [];
          setNativeFileDropPaths(paths);

          // Clear paths after 5 seconds to prevent stale data
          setTimeout(() => {
            setNativeFileDropPaths([]);
          }, 5000);
        });
      } catch (err) {
        // File-drop event might not be available in all Tauri versions
        // or might require additional configuration
        console.warn('File-drop event listener not available:', err);
      }
    };

    setupFileDropListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // Disable scrolling when content fits within container
  useEffect(() => {
    const checkScroll = () => {
      // Check icon view
      if (iconViewRef.current && viewMode === 'icon') {
        const element = iconViewRef.current;
        const hasScroll = element.scrollHeight > element.clientHeight;
        if (hasScroll) {
          element.classList.add('has-scroll');
        } else {
          element.classList.remove('has-scroll');
        }
      }

      // Check list view
      if (listViewRef.current && viewMode === 'list') {
        const element = listViewRef.current;
        const hasScroll = element.scrollHeight > element.clientHeight;
        if (hasScroll) {
          element.classList.add('has-scroll');
        } else {
          element.classList.remove('has-scroll');
        }
      }
    };

    // Check immediately
    checkScroll();

    // Check after multiple delays to ensure DOM is fully updated
    // React may batch updates, so we need to check after render cycles complete
    const timeoutId1 = setTimeout(checkScroll, 50);
    const timeoutId2 = setTimeout(checkScroll, 150);
    const timeoutId3 = setTimeout(checkScroll, 300);

    // Also check on window resize
    window.addEventListener('resize', checkScroll);

    // Use requestAnimationFrame for more accurate timing
    const rafId = requestAnimationFrame(() => {
      setTimeout(checkScroll, 0);
    });

    return () => {
      clearTimeout(timeoutId1);
      clearTimeout(timeoutId2);
      clearTimeout(timeoutId3);
      cancelAnimationFrame(rafId);
      window.removeEventListener('resize', checkScroll);
    };
  }, [viewMode, files.length]); // Use files.length to detect content changes

  // Refresh clipboard state when window gains focus (detect native clipboard changes)
  useEffect(() => {
    const handleFocus = () => {
      refreshClipboardState();
    };

    window.addEventListener('focus', handleFocus);

    // Initial clipboard check
    refreshClipboardState();

    return () => {
      window.removeEventListener('focus', handleFocus);
    };
  }, [refreshClipboardState]);

  // Handle spotlight quick actions
  useEffect(() => {
    const handleSpotlightAction = (e: CustomEvent<string>) => {
      const actionId = e.detail;
      switch (actionId) {
        case 'new-folder':
          handleNewFolder();
          break;
        case 'toggle-hidden':
          setShowHiddenFiles((prev) => !prev);
          break;
        case 'icon-view':
          setViewMode('icon');
          break;
        case 'list-view':
          setViewMode('list');
          break;
        case 'refresh':
          if (selectedSource) {
            loadFilesList(selectedSource.id, currentPath);
          }
          break;
      }
    };

    window.addEventListener(
      'spotlight-action',
      handleSpotlightAction as EventListener,
    );

    return () => {
      window.removeEventListener(
        'spotlight-action',
        handleSpotlightAction as EventListener,
      );
    };
  }, [selectedSource, currentPath]);

  const initAndLoadSources = async () => {
    try {
      // Initialize VFS first
      await invoke('vfs_init');

      // Load sources from backend (includes system locations and all sources)
      await loadSourcesList();

      // Only load persisted sources if backend didn't return any (fallback)
      // This prevents setting partial sources that would cause Local section to disappear

      // Note: We now show hidden files by default in VFS, so we don't load OS preferences
      // for this setting. Users can toggle visibility using the toolbar button.
    } catch (err) {
      console.error('Failed to initialize VFS:', err);
      // Only load persisted sources as fallback if backend completely failed
      // But don't set sourcesLoaded until we have a complete list
      const persisted = loadPersistedSources();
      if (persisted && persisted.length > 0) {
        // Only set sources if we don't have any yet (initial load failure)
        setSources((prevSources) => {
          if (prevSources.length === 0) {
            setSourcesLoaded(true);
            return persisted;
          }
          return prevSources;
        });
      }
    }
  };

  // Load files when path changes (source changes are handled by selectSource)
  // This useEffect handles navigation within a source (folder changes)
  const prevSourceIdRef = useRef<string | null>(null);

  // Listen for upload completion events to refresh file list
  useEffect(() => {
    const handleUploadComplete = () => {
      // Refresh file list when upload completes
      if (selectedSource && !loading) {
        // Small delay to ensure backend has updated
        setTimeout(() => {
          loadFilesList(selectedSource.id, currentPath).catch((err) => {
            console.error('[VFS] Error refreshing files after upload:', err);
          });
        }, 500);
      }
    };

    // Listen for custom upload completion events
    window.addEventListener('upload-completed', handleUploadComplete);

    return () => {
      window.removeEventListener('upload-completed', handleUploadComplete);
    };
  }, [selectedSource?.id, currentPath, loading]);

  useEffect(() => {
    if (selectedSource?.id) {
      // Cancel any in-flight request when switching sources or paths
      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
        abortControllerRef.current = null;
      }
      // Reset loading state and pagination when switching
      // Set loading to true, but ensure it's cleared even if loadFilesList fails
      setLoading(true);
      setPaginationState({
        continuationToken: null,
        hasMore: false,
        totalCount: null,
        isLoadingMore: false,
      });
      // Only load if path changed within the same source
      // Source changes are handled directly by selectSource to avoid race conditions
      if (prevSourceIdRef.current === selectedSource.id) {
        loadFilesList(selectedSource.id, currentPath).catch((err) => {
          console.error('[VFS] Error loading files after source switch:', err);
          setLoading(false);
        });
      } else {
        // New source selected - load immediately
        loadFilesList(selectedSource.id, currentPath).catch((err) => {
          console.error('[VFS] Error loading files after source switch:', err);
          setLoading(false);
        });
      }
      prevSourceIdRef.current = selectedSource.id;
    } else {
      // Clear files and loading when no source selected
      setFiles([]);
      setLoading(false);
      setPaginationState({
        continuationToken: null,
        hasMore: false,
        totalCount: null,
        isLoadingMore: false,
      });
      // Cancel any in-flight request
      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
        abortControllerRef.current = null;
      }
    }
  }, [selectedSource?.id, currentPath]);

  // Load thumbnails for initial visible items when switching to grid view
  // Only load a small batch initially to prevent app crash
  useEffect(() => {
    if (viewMode === 'icon' && selectedSource?.id && files.length > 0) {
      // Only load thumbnails for the first visible batch (first 20 items)
      // This prevents the app from crashing when switching to grid view with many files
      const filesNeedingThumbnails = files
        .filter(
          (f) => !f.isDirectory && !f.thumbnail && canHaveThumbnail(f.name),
        )
        .slice(0, 20); // Small initial batch to prevent crash

      if (filesNeedingThumbnails.length > 0) {
        // Add a delay to prevent blocking the UI when switching views
        const timeoutId = setTimeout(() => {
          loadThumbnailsForFiles(selectedSource.id, filesNeedingThumbnails);
        }, 300);

        return () => clearTimeout(timeoutId);
      }
    }
  }, [viewMode, selectedSource?.id]); // Removed 'files' from deps to prevent reload on every file change

  // Helper to check if file can have a thumbnail
  const canHaveThumbnail = (filename: string): boolean => {
    const thumbnailTypes = [
      // Images
      'jpg',
      'jpeg',
      'png',
      'gif',
      'bmp',
      'tiff',
      'tif',
      'webp',
      'heic',
      'heif',
      'svg',
      'ico',
      'raw',
      'cr2',
      'nef',
      'arw',
      'dng',
      'orf',
      'rw2',
      'pef',
      'srw',
      'psd',
      'ai',
      'eps',
      // PDF
      'pdf',
      // Video
      'mp4',
      'mov',
      'avi',
      'mkv',
      'wmv',
      'flv',
      'webm',
      'm4v',
      'mpg',
      'mpeg',
      '3gp',
      'mxf',
      'prores',
      'r3d',
      'braw',
    ];
    const ext = filename.split('.').pop()?.toLowerCase() || '';
    return thumbnailTypes.includes(ext);
  };

  // Load storage sources from localStorage on mount
  const loadPersistedSources = () => {
    try {
      const stored = localStorage.getItem('diaspor-storage-sources');
      if (stored) {
        const parsed = JSON.parse(stored) as StorageSource[];
        if (Array.isArray(parsed) && parsed.length > 0) {
          // Return parsed sources - DO NOT set state here as it would cause flicker
          // State should only be set in loadSourcesList after merging with backend sources
          return parsed;
        }
      }
    } catch (err) {
      console.error('Failed to load persisted sources:', err);
    }
    return null;
  };

  // Save storage sources to localStorage
  // SECURITY: Never persist credentials (accessKeyId, secretAccessKey, passwords, tokens)
  const savePersistedSources = (sourcesList: StorageSource[]) => {
    try {
      // Don't persist system locations or ejectable volumes
      const toPersist = sourcesList
        .filter((s) => !s.isSystemLocation && !s.isEjectable)
        .map((source) => {
          // Remove sensitive credentials before persisting
          const { config, ...sourceWithoutConfig } = source;
          const safeConfig: Record<string, unknown> = { ...config };

          // Remove all credential fields
          const credentialFields = [
            'accessKeyId',
            'secretAccessKey',
            'password',
            'token',
            'apiKey',
            'secret',
            'credential',
            'accessKey',
            'secretKey',
            'authToken',
            'bearerToken',
          ];

          credentialFields.forEach((field) => {
            delete safeConfig[field];
            delete safeConfig[field.toLowerCase()];
            delete safeConfig[field.toUpperCase()];
          });

          return {
            ...sourceWithoutConfig,
            config: safeConfig,
          };
        });

      localStorage.setItem('diaspor-storage-sources', JSON.stringify(toPersist));
    } catch (err) {
      console.error('Failed to save persisted sources:', err);
    }
  };

  // Helper function to infer providerId from source data
  const inferProviderId = (source: StorageSource): string | null => {
    // If providerId exists, use it
    if (source.providerId) {
      return source.providerId;
    }

    // Try to infer from category and config
    if (source.category === 'cloud') {
      // Check config for hints
      const config = source.config || {};
      const endpoint = config.endpoint as string | undefined;
      if (
        endpoint &&
        typeof endpoint === 'string' &&
        endpoint.includes('googleapis.com')
      ) {
        return 'gcs';
      }
      if (
        endpoint &&
        typeof endpoint === 'string' &&
        endpoint.includes('blob.core.windows.net')
      ) {
        return 'azure-blob';
      }
      // Default to S3 for cloud storage
      return 'aws-s3';
    }

    // Try to infer from deprecated type field
    if (source.type) {
      const typeMap: Record<string, string> = {
        s3: 'aws-s3',
        'aws-s3': 'aws-s3',
        gcs: 'gcs',
        'azure-blob': 'azure-blob',
        local: 'local',
        nfs: 'nfs',
        smb: 'smb',
        sftp: 'sftp',
      };
      return typeMap[source.type] || source.type;
    }

    // Try to infer from category
    const categoryMap: Record<string, string> = {
      cloud: 'aws-s3',
      local: 'local',
      network: 'smb',
    };
    return categoryMap[source.category] || null;
  };

  // Helper function to generate a unique key for a source (for deduplication)
  const getSourceUniqueKey = (source: StorageSource): string => {
    const providerId =
      source.providerId || inferProviderId(source) || 'unknown';

    // For object storage (S3, GCS, Azure), use bucket + region + endpoint
    if (
      source.category === 'cloud' ||
      providerId === 's3' ||
      providerId === 'aws-s3' ||
      providerId === 'gcs' ||
      providerId === 'azure-blob'
    ) {
      const bucket =
        source.config?.bucket || source.bucket || source.config?.path || '';
      const region = source.config?.region || source.region || '';
      const endpoint = source.config?.endpoint || '';
      return `${providerId}:${bucket}:${region}:${endpoint}`.toLowerCase();
    }
    // For local/network storage, use path
    const path = source.config?.path || source.path || '';
    return `${providerId}:${path}`.toLowerCase();
  };

  // Helper function to map backend response to frontend StorageSource format
  const mapBackendSourceToFrontend = (backendSource: {
    id: string;
    name: string;
    source_type: string;
    mounted: boolean;
    status: string;
    path?: string | null;
    bucket?: string | null;
    region?: string | null;
    category?: string; // Backend-provided category (preferred)
    provider_id?: string; // Backend-provided provider ID (preferred)
    is_ejectable?: boolean;
    is_system_location?: boolean;
  }): StorageSource => {
    // Use backend-provided category and provider_id if available (preferred)
    let category: StorageCategory;
    let providerId: string;

    if (backendSource.category && backendSource.provider_id) {
      // Use backend-provided values
      category = backendSource.category as StorageCategory;
      providerId = backendSource.provider_id;
    } else {
      // Fallback to inferring from source_type (for backward compatibility)
      const sourceTypeMap: Record<
        string,
        { providerId: string; category: StorageCategory }
      > = {
        Local: { providerId: 'local', category: 'local' },
        S3: { providerId: 'aws-s3', category: 'cloud' },
        S3Compatible: { providerId: 's3-compatible', category: 'cloud' },
        Gcs: { providerId: 'gcs', category: 'cloud' },
        AzureBlob: { providerId: 'azure-blob', category: 'cloud' },
        FsxOntap: { providerId: 'fsx-ontap', category: 'hybrid' },
        FsxN: { providerId: 'fsx-ontap', category: 'hybrid' },
        Nfs: { providerId: 'nfs', category: 'network' },
        Smb: { providerId: 'smb', category: 'network' },
        Nas: { providerId: 'smb', category: 'network' },
        Sftp: { providerId: 'sftp', category: 'network' },
        WebDav: { providerId: 'webdav', category: 'network' },
        Block: { providerId: 'block', category: 'block' },
      };

      // Handle Custom types (e.g., "Custom(\"provider-id\")")
      let mapping: { providerId: string; category: StorageCategory };
      if (backendSource.source_type.startsWith('Custom(')) {
        // Extract provider ID from Custom("provider-id") format
        const match = backendSource.source_type.match(/Custom\("([^"]+)"\)/);
        const extractedProviderId = match ? match[1] : 'unknown';

        mapping = {
          providerId: extractedProviderId,
          category: 'custom',
        };
      } else {
        mapping = sourceTypeMap[backendSource.source_type] || {
          providerId: 'unknown',
          category: 'custom' as StorageCategory,
        };
      }

      category = mapping.category;
      providerId = mapping.providerId;
    }

    // Map status string to frontend status
    // Handle both enum format (e.g., "Connected") and string format
    const statusStr = backendSource.status || 'Disconnected';
    const statusMap: Record<
      string,
      'connected' | 'connecting' | 'disconnected' | 'error'
    > = {
      Connected: 'connected',
      Connecting: 'connecting',
      Disconnected: 'disconnected',
      Error: 'error',
      // Handle lowercase variants
      connected: 'connected',
      connecting: 'connecting',
      disconnected: 'disconnected',
      error: 'error',
    };
    const status = statusMap[statusStr] || 'disconnected';

    // Build config from backend response
    const config: Record<string, unknown> = {};
    if (backendSource.bucket) {
      config.bucket = backendSource.bucket;
    }
    if (backendSource.path) {
      config.path = backendSource.path;
    }
    if (backendSource.region) {
      config.region = backendSource.region;
    }

    return {
      id: backendSource.id,
      name: backendSource.name,
      providerId,
      category,
      config,
      status,
      isEjectable: backendSource.is_ejectable || false,
      isSystemLocation: backendSource.is_system_location || false,
    };
  };

  const loadSourcesList = async () => {
    try {
      // Load persisted sources to merge with backend sources
      const persisted = loadPersistedSources();

      // Load from backend (this includes system locations and all sources)
      const backendList = (await invoke('vfs_list_sources')) as Array<{
        id: string;
        name: string;
        source_type: string;
        category?: string;
        provider_id?: string;
        mounted: boolean;
        status: string;
        path?: string | null;
        bucket?: string | null;
        region?: string | null;
        is_ejectable?: boolean;
        is_system_location?: boolean;
      }>;

      console.log(
        '[FinderPage] Backend returned',
        backendList.length,
        'sources:',
        backendList,
      );

      if (backendList.length === 0) {
        console.warn(
          '[FinderPage] WARNING: Backend returned 0 sources! vfs_init may not have completed or sources were not created.',
        );
      }

      backendList.forEach((source, idx) => {
        console.log(`[FinderPage] Source ${idx}:`, {
          id: source.id,
          name: source.name,
          category: source.category,
          provider_id: source.provider_id,
          source_type: source.source_type,
          mounted: source.mounted,
          status: source.status,
          path: source.path,
        });
      });

      // Map backend sources to frontend format with error handling
      const list = backendList.map((backendSource) => {
        try {
          const mapped = mapBackendSourceToFrontend(backendSource);
          console.log('[FinderPage] Mapped source:', backendSource.name, '->', {
            category: mapped.category,
            providerId: mapped.providerId,
            status: mapped.status,
          });
          return mapped;
        } catch (err) {
          console.error('Failed to map backend source:', backendSource, err);
          // Return a fallback source to prevent crashes
          return {
            id: backendSource.id,
            name: backendSource.name || 'Unknown',
            providerId: 'unknown',
            category: 'custom' as StorageCategory,
            config: {},
            status: 'disconnected' as const,
            isEjectable: backendSource.is_ejectable || false,
            isSystemLocation: backendSource.is_system_location || false,
          };
        }
      });

      // Deduplicate backend sources first (in case backend has duplicates)
      const backendUniqueKeys = new Map<string, StorageSource>();
      const duplicateKeys: string[] = [];
      for (const source of list) {
        const key = getSourceUniqueKey(source);
        // Keep the first occurrence, or prefer non-system locations
        if (
          !backendUniqueKeys.has(key) ||
          (!source.isSystemLocation &&
            backendUniqueKeys.get(key)?.isSystemLocation)
        ) {
          if (backendUniqueKeys.has(key)) {
            duplicateKeys.push(key);
            console.log(
              `[FinderPage] Deduplicating source with key "${key}": keeping ${source.name} (id: ${source.id}), removing ${backendUniqueKeys.get(key)?.name} (id: ${backendUniqueKeys.get(key)?.id})`,
            );
          }
          backendUniqueKeys.set(key, source);
        } else {
          duplicateKeys.push(key);
          console.log(
            `[FinderPage] Skipping duplicate source with key "${key}": ${source.name} (id: ${source.id})`,
          );
        }
      }
      const deduplicatedBackend = Array.from(backendUniqueKeys.values());
      if (duplicateKeys.length > 0) {
        console.log(
          `[FinderPage] Found ${duplicateKeys.length} duplicate keys during backend deduplication`,
        );
      }
      const backendIds = new Set(deduplicatedBackend.map((s) => s.id));

      // Re-add persisted sources that aren't in backend (user-added sources)
      // This ensures they're available for operations like upload

      const merged: StorageSource[] = [...deduplicatedBackend];
      const mergedKeys = new Set(
        deduplicatedBackend.map((s) => getSourceUniqueKey(s)),
      );

      // CRITICAL: Ensure we always have backend sources (which include local sources) before merging persisted
      // If backend sources are empty, something went wrong - log and preserve existing sources
      if (deduplicatedBackend.length === 0) {
        console.warn(
          '[FinderPage] Backend returned 0 sources - this should not happen if VFS is initialized',
        );
        // Don't update sources if backend is empty - preserve existing
        return;
      }

      if (persisted) {
        // Deduplicate persisted sources first
        const persistedUnique = new Map<string, StorageSource>();
        for (const source of persisted) {
          const key = getSourceUniqueKey(source);
          if (!persistedUnique.has(key)) {
            persistedUnique.set(key, source);
          }
        }

        for (const source of persistedUnique.values()) {
          const sourceKey = getSourceUniqueKey(source);

          // Skip if already in merged list (by unique key)
          if (mergedKeys.has(sourceKey)) {
            console.log(
              '[FinderPage] Skipping duplicate persisted source:',
              source.name,
            );
            continue;
          }

          // Skip if already in backend (by ID)
          if (backendIds.has(source.id)) {
            console.log(
              '[FinderPage] Skipping persisted source already in backend:',
              source.name,
            );
            continue;
          }

          // Infer providerId if missing
          const providerId = source.providerId || inferProviderId(source);
          if (!providerId) {
            console.warn(
              '[FinderPage] Cannot re-add persisted source (missing providerId):',
              source.name,
              'Category:',
              source.category,
            );
            // Skip this source - it can't be re-added without a providerId
            continue;
          }

          // Validate required config fields based on provider
          const config = source.config || {};
          if (
            providerId === 'aws-s3' ||
            providerId === 'gcs' ||
            providerId === 'azure-blob'
          ) {
            const bucket = config.bucket || config.path;
            if (!bucket) {
              console.warn(
                '[FinderPage] Cannot re-add persisted source (missing bucket/path):',
                source.name,
              );
              continue;
            }
          }

          try {
            // Re-add the source to backend with inferred providerId
            const reAddedSource = (await invoke('vfs_add_source', {
              source: {
                providerId,
                name: source.name,
                category: source.category,
                config: source.config || {},
              },
            })) as StorageSource;

            // Check if re-added source is a duplicate
            const reAddedKey = getSourceUniqueKey(reAddedSource);
            if (!mergedKeys.has(reAddedKey)) {
              merged.push(reAddedSource);
              mergedKeys.add(reAddedKey);
              console.log(
                '[FinderPage] Re-added persisted source to backend:',
                reAddedSource.name,
                'ProviderId:',
                providerId,
                // Note: Credentials are not logged for security
              );
            } else {
              console.log(
                '[FinderPage] Re-added source is duplicate, skipping:',
                reAddedSource.name,
              );
            }
          } catch (err) {
            console.error(
              '[FinderPage] Failed to re-add persisted source:',
              source.name,
              'ProviderId:',
              providerId,
              'Error:',
              err,
            );
            // Don't add failed sources to merged list - they're invalid
          }
        }
      }

      // Final deduplication pass to ensure no duplicates
      // First deduplicate by unique key (bucket+region+endpoint for cloud, path for local)
      const finalDeduplicated = new Map<string, StorageSource>();
      const finalDuplicateKeys: string[] = [];
      for (const source of merged) {
        const key = getSourceUniqueKey(source);
        // Prefer sources that are not system locations
        if (
          !finalDeduplicated.has(key) ||
          (!source.isSystemLocation &&
            finalDeduplicated.get(key)?.isSystemLocation)
        ) {
          if (finalDeduplicated.has(key)) {
            finalDuplicateKeys.push(key);
            console.log(
              `[FinderPage] Final deduplication: keeping ${source.name} (id: ${source.id}), removing ${finalDeduplicated.get(key)?.name} (id: ${finalDeduplicated.get(key)?.id})`,
            );
          }
          finalDeduplicated.set(key, source);
        } else {
          finalDuplicateKeys.push(key);
          console.log(
            `[FinderPage] Final deduplication: skipping duplicate ${source.name} (id: ${source.id})`,
          );
        }
      }
      if (finalDuplicateKeys.length > 0) {
        console.log(
          `[FinderPage] Found ${finalDuplicateKeys.length} duplicate keys during final deduplication`,
        );
      }

      // Then deduplicate by ID to ensure no duplicate IDs (in case unique key logic fails)
      const finalById = new Map<string, StorageSource>();
      for (const source of finalDeduplicated.values()) {
        if (!finalById.has(source.id)) {
          finalById.set(source.id, source);
        } else {
          console.warn(
            '[FinderPage] Duplicate source ID detected, keeping first occurrence:',
            source.id,
            source.name,
          );
        }
      }

      const finalSources = Array.from(finalById.values());

      // Log any remaining duplicates by ID for debugging
      const sourceIds = new Set<string>();
      const duplicateIds: string[] = [];
      for (const source of finalSources) {
        if (sourceIds.has(source.id)) {
          duplicateIds.push(source.id);
        } else {
          sourceIds.add(source.id);
        }
      }
      if (duplicateIds.length > 0) {
        console.error(
          '[FinderPage] WARNING: Found duplicate source IDs after deduplication:',
          duplicateIds,
        );
      }

      console.log(
        `[FinderPage] Loaded ${finalSources.length} unique sources (removed ${merged.length - finalSources.length} duplicates)`,
      );
      console.log('[FinderPage] Final sources by category:', {
        local: finalSources.filter((s) => s.category === 'local').length,
        cloud: finalSources.filter((s) => s.category === 'cloud').length,
        network: finalSources.filter((s) => s.category === 'network').length,
        hybrid: finalSources.filter((s) => s.category === 'hybrid').length,
        block: finalSources.filter((s) => s.category === 'block').length,
        custom: finalSources.filter((s) => s.category === 'custom').length,
      });
      finalSources.forEach((source) => {
        console.log(
          `[FinderPage] Final source: ${source.name} (${source.category}/${source.providerId})`,
        );
        // Log cloud sources specifically for debugging
        if (source.category === 'cloud') {
          console.log(
            `[FinderPage] Cloud source found: ${source.name} (provider: ${source.providerId}, bucket: ${source.config?.bucket || 'N/A'})`,
          );
        }
      });

      // CRITICAL: Ensure finalSources always includes backend sources (which have local sources)
      // If finalSources doesn't have local sources but backend does, something went wrong
      const backendHasLocal = deduplicatedBackend.some(
        (s) => s.category === 'local',
      );
      const finalHasLocal = finalSources.some((s) => s.category === 'local');
      if (backendHasLocal && !finalHasLocal) {
        console.error(
          '[FinderPage] ERROR: Backend has local sources but finalSources does not! This should never happen.',
        );
        console.error(
          '[FinderPage] Backend local sources:',
          deduplicatedBackend
            .filter((s) => s.category === 'local')
            .map((s) => s.name),
        );
        console.error(
          '[FinderPage] Final sources categories:',
          finalSources.map((s) => s.category),
        );
        // Don't update sources - preserve existing to prevent flicker
        return;
      }

      // Only update sources if they actually changed (by ID) to prevent unnecessary re-renders
      // CRITICAL: Never clear sources once loaded - always preserve existing sources if new list is empty
      // CRITICAL: Never set sources to a partial list (e.g., only persisted sources without local)
      setSources((prevSources) => {
        // If we have no sources yet but finalSources has content, always update
        if (prevSources.length === 0 && finalSources.length > 0) {
          savePersistedSources(finalSources);
          setSourcesLoaded(true);
          return finalSources;
        }

        // If finalSources is empty but we have existing sources, preserve them
        if (finalSources.length === 0 && prevSources.length > 0) {
          return prevSources;
        }

        // CRITICAL: If prevSources has local sources but finalSources doesn't, preserve prevSources
        // This prevents Local section from disappearing when persisted sources (without local) are loaded
        const prevHasLocal = prevSources.some((s) => s.category === 'local');
        const finalHasLocal = finalSources.some((s) => s.category === 'local');
        const prevLocalCount = prevSources.filter(
          (s) => s.category === 'local',
        ).length;
        const finalLocalCount = finalSources.filter(
          (s) => s.category === 'local',
        ).length;

        // Prevent flicker: preserve existing sources if we're losing local sources
        if (
          prevHasLocal &&
          (prevLocalCount > finalLocalCount ||
            (!finalHasLocal && prevLocalCount > 0))
        ) {
          return prevSources;
        }

        const prevIds = new Set(prevSources.map((s) => s.id));
        const newIds = new Set(finalSources.map((s) => s.id));

        // Check if IDs changed
        if (
          prevIds.size !== newIds.size ||
          !Array.from(prevIds).every((id) => newIds.has(id)) ||
          !Array.from(newIds).every((id) => prevIds.has(id))
        ) {
          // Sources changed - update and persist
          savePersistedSources(finalSources);
          setSourcesLoaded(true);
          return finalSources;
        }

        // IDs are the same - check if any source properties changed
        const sourcesById = new Map(prevSources.map((s) => [s.id, s]));
        const hasPropertyChanges = finalSources.some((newSource) => {
          const oldSource = sourcesById.get(newSource.id);
          if (!oldSource) return true; // New source

          // Compare key properties that affect rendering
          return (
            oldSource.name !== newSource.name ||
            oldSource.category !== newSource.category ||
            oldSource.status !== newSource.status ||
            oldSource.isEjectable !== newSource.isEjectable ||
            oldSource.isSystemLocation !== newSource.isSystemLocation
          );
        });

        if (hasPropertyChanges) {
          savePersistedSources(finalSources);
          setSourcesLoaded(true);
          return finalSources;
        }

        // No changes - return previous array to prevent re-render
        // But still mark as loaded if this is the first load
        if (!sourcesLoaded && finalSources.length > 0) {
          setSourcesLoaded(true);
        }

        // Always return prevSources to maintain reference stability
        return prevSources;
      });

      // Only auto-select first valid source if nothing is selected
      // Prioritize local storage sources by default
      if (finalSources.length > 0 && !selectedSource) {
        // Helper function to check if a source is valid
        const isValidSource = (s: StorageSource): boolean => {
          // For cloud storage, ensure bucket is configured
          if (
            s.category === 'cloud' ||
            s.providerId === 'aws-s3' ||
            s.providerId === 'gcs' ||
            s.providerId === 'azure-blob'
          ) {
            const config = s.config || {};
            return !!(config.bucket || config.path);
          }
          // For local/network storage, ensure path is configured
          if (s.category === 'local' || s.category === 'network') {
            const config = s.config || {};
            const isValid = !!config.path;
            return isValid;
          }
          return true; // Assume valid for other types
        };

        // First, try to find a local storage source
        let validSource = finalSources.find(
          (s) => s.category === 'local' && isValidSource(s),
        );

        // If no local source found, try network storage
        if (!validSource) {
          validSource = finalSources.find(
            (s) => s.category === 'network' && isValidSource(s),
          );
        }

        // If still no source found, fall back to any valid source
        if (!validSource) {
          validSource = finalSources.find(isValidSource);
        }

        if (validSource) {
          console.log(
            '[FinderPage] Auto-selecting source:',
            validSource.name,
            'category:',
            validSource.category,
          );
          setSelectedSource(validSource);
          // Explicitly load files for initial source
          // (since useEffect won't trigger - prevSourceIdRef is null initially)
          // Pass source and sources array directly since state hasn't updated yet
          prevSourceIdRef.current = validSource.id;
          await loadFilesList(validSource.id, '', validSource, finalSources);
        } else {
          console.warn('[FinderPage] No valid sources found to auto-select');
        }
      }
    } catch (err) {
      console.error('Failed to load sources:', err);
      // Set empty sources array to prevent crashes
      setSources([]);
    }
  };

  const loadFilesList = async (
    sourceId: string,
    path: string,
    providedSource?: StorageSource,
    providedSources?: StorageSource[],
  ) => {
    // Cancel any in-flight request
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }

    // Create new abort controller for this request
    const abortController = new AbortController();
    abortControllerRef.current = abortController;

    // Track this request
    const requestId = { sourceId, path };
    currentRequestRef.current = requestId;

    // Validate source exists before trying to list files
    // Use provided source/sources if available (for initial load before state updates)
    const source =
      providedSource ||
      providedSources?.find((s) => s.id === sourceId) ||
      sources.find((s) => s.id === sourceId);
    if (!source) {
      console.warn('[VFS] Cannot load files: source not found:', sourceId);
      setFiles([]);
      setLoading(false);
      return;
    }

    // Validate source has required config for cloud storage
    if (
      source.category === 'cloud' ||
      source.providerId === 'aws-s3' ||
      source.providerId === 'gcs' ||
      source.providerId === 'azure-blob'
    ) {
      const config = source.config || {};
      const bucket = config.bucket || config.path;
      if (!bucket) {
        // SECURITY: Never log full source object as it may contain credentials
        console.error(
          '[VFS] Cannot load files: source missing bucket/path:',
          source.name,
        );
        DialogService.error(
          `Storage source "${source.name}" is not properly configured. Please edit or remove it.`,
          'Configuration Error',
        );
        setFiles([]);
        setLoading(false);
        return;
      }
    }

    setLoading(true);
    try {
      // Check if this request was cancelled
      if (abortController.signal.aborted) {
        console.log('[VFS] Request cancelled before starting');
        return;
      }

      const normalizedPath = path || '';
      console.log(
        '[VFS] Loading files for source:',
        sourceId,
        'Path:',
        normalizedPath || '(root)',
      );

      // Log source info for debugging
      const source = sources.find((s) => s.id === sourceId);
      if (source) {
        console.log(
          `[VFS] Source details: name=${source.name}, category=${source.category}, providerId=${source.providerId}`,
        );
        if (source.category === 'cloud') {
          console.log(
            `[VFS] Cloud source config: bucket=${source.config?.bucket || 'N/A'}, region=${source.config?.region || 'N/A'}`,
          );
        }
      }

      // Check again if cancelled before making the request
      if (
        abortController.signal.aborted ||
        currentRequestRef.current?.sourceId !== sourceId
      ) {
        console.log('[VFS] Request cancelled before invoke');
        return;
      }

      // Check cache first (only for non-paginated requests)
      if (!paginationState.isLoadingMore) {
        const cachedFiles = fileCache.get(sourceId, normalizedPath);
        if (cachedFiles) {
          console.log(
            '[VFS] Using cached file listing:',
            cachedFiles.length,
            'files',
          );
          setFiles(cachedFiles);
          setLoading(false);
          return;
        }
      }

      // Handle paginated response - extract files array
      // For object storage, limit to 50 items by default
      const isObjectStorage = source?.category === 'cloud';
      const response = await invoke<
        | FileMetadata[]
        | {
            files: FileMetadata[];
            continuation_token?: string | null;
            total_count?: number | null;
          }
      >('vfs_list_files', {
        sourceId,
        path: normalizedPath,
        ...(isObjectStorage
          ? {
              limit: 50,
              continuation_token: paginationState.isLoadingMore
                ? paginationState.continuationToken
                : null,
            }
          : {}), // Only apply limit and pagination for object storage
      });

      // Check if this response is still valid (user may have switched sources)
      if (
        abortController.signal.aborted ||
        currentRequestRef.current?.sourceId !== sourceId
      ) {
        console.log('[VFS] Request cancelled after invoke, ignoring response');
        return;
      }

      console.log(
        '[VFS] Response type:',
        Array.isArray(response) ? 'array' : 'object',
        'Response:',
        response,
      );

      // Backend always returns an object now, but handle both for compatibility
      const list = Array.isArray(response)
        ? response
        : response && typeof response === 'object' && 'files' in response
          ? response.files || []
          : [];

      // Extract pagination info for object storage
      const continuationToken = Array.isArray(response)
        ? null
        : response &&
            typeof response === 'object' &&
            'continuation_token' in response
          ? response.continuation_token || null
          : null;
      const totalCount = Array.isArray(response)
        ? null
        : response && typeof response === 'object' && 'total_count' in response
          ? response.total_count || null
          : null;
      const hasMore =
        continuationToken !== null && continuationToken !== undefined;

      console.log(
        '[VFS] Extracted list length:',
        list.length,
        'hasMore:',
        hasMore,
      );

      // Map backend response to frontend FileMetadata format
      // Ensure isDirectory is correctly set from is_directory
      const mappedFiles: FileMetadata[] = list.map((f) => ({
        ...f,
        isDirectory:
          f.isDirectory ??
          (f as { is_directory?: boolean }).is_directory ??
          false,
        lastModified:
          f.lastModified ??
          (f as { last_modified?: string }).last_modified ??
          '',
        mimeType: f.mimeType ?? (f as { mime_type?: string }).mime_type,
        isHidden:
          f.isHidden ?? (f as { is_hidden?: boolean }).is_hidden ?? false,
        isCached:
          f.isCached ?? (f as { is_cached?: boolean }).is_cached ?? false,
        canWarm: f.canWarm ?? (f as { can_warm?: boolean }).can_warm ?? false,
        canTranscode:
          f.canTranscode ??
          (f as { can_transcode?: boolean }).can_transcode ??
          false,
        tierStatus:
          f.tierStatus ?? (f as { tier_status?: string }).tier_status ?? 'hot',
      }));

      // Final check before updating state
      if (
        abortController.signal.aborted ||
        currentRequestRef.current?.sourceId !== sourceId
      ) {
        console.log(
          '[VFS] Request cancelled before setting files, ignoring response',
        );
        return;
      }

      // Update files (append if loading more, replace if new load)
      if (
        paginationState.isLoadingMore &&
        currentRequestRef.current?.sourceId === sourceId
      ) {
        // Append to existing files when loading more
        setFiles((prevFiles) => [...prevFiles, ...mappedFiles]);
      } else {
        // Replace files for new load
        setFiles(mappedFiles);
        // Store in cache for faster subsequent loads (only cache initial load, not pagination)
        if (!isObjectStorage) {
          fileCache.set(
            sourceId,
            normalizedPath,
            mappedFiles,
            continuationToken,
          );
        }
      }

      // Calculate folder sizes lazily for mounted storage (local, NAS, FSx)
      if (supportsFilesystemOperations(selectedSource)) {
        // Calculate folder sizes in background for visible folders
        mappedFiles
          .filter((f) => f.isDirectory)
          .forEach((folder) => {
            const cacheKey = `${sourceId}:${folder.path}`;

            // Always recalculate folder size if it's 0 or if cache entry exists but size is 0
            // This ensures copied folders get their size recalculated
            const shouldRecalculate =
              folder.size === 0 ||
              !folderSizeCache.current.has(cacheKey) ||
              (folderSizeCache.current.has(cacheKey) &&
                folderSizeCache.current.get(cacheKey) === 0);

            if (shouldRecalculate) {
              // Mark as calculating to prevent duplicate requests
              folderSizeCache.current.set(cacheKey, -1); // -1 means calculating

              // Calculate folder size asynchronously
              invoke<number>('vfs_get_folder_size', {
                sourceId: sourceId,
                path: folder.path,
              })
                .then((size) => {
                  // Update cache with actual size
                  folderSizeCache.current.set(cacheKey, size);

                  // Update file in state with calculated size
                  setFiles((prevFiles) =>
                    prevFiles.map((f) =>
                      f.path === folder.path && f.isDirectory
                        ? {
                            ...f,
                            size,
                            size_human: formatSize(size),
                          }
                        : f,
                    ),
                  );
                })
                .catch((err) => {
                  console.warn(
                    `[VFS] Failed to calculate folder size for ${folder.path}:`,
                    err,
                  );
                  // Remove from cache on error so we can retry later
                  folderSizeCache.current.delete(cacheKey);
                });
            } else {
              // Use cached size if available and valid
              const cachedSize = folderSizeCache.current.get(cacheKey);
              if (cachedSize !== undefined && cachedSize >= 0) {
                setFiles((prevFiles) =>
                  prevFiles.map((f) =>
                    f.path === folder.path && f.isDirectory
                      ? {
                          ...f,
                          size: cachedSize,
                          size_human: formatSize(cachedSize),
                        }
                      : f,
                  ),
                );
              }
            }
          });
      }

      // Update pagination state
      setPaginationState({
        continuationToken,
        hasMore,
        totalCount,
        isLoadingMore: false,
      });

      console.log(
        '[VFS] Loaded',
        mappedFiles.length,
        'files',
        hasMore ? '(more available)' : '(all loaded)',
      );

      // Load thumbnails for image/video files in the background
      if (viewMode === 'icon') {
        loadThumbnailsForFiles(sourceId, mappedFiles);
      }
    } catch (err) {
      // Ignore errors from cancelled requests
      if (abortController.signal.aborted) {
        console.log('[VFS] Request was cancelled, ignoring error');
        return;
      }
      console.error('[VFS] Failed to load files:', err);
      const errorMessage = err instanceof Error ? err.message : String(err);

      // Check if it's a "directory doesn't exist" error - this can happen with newly created folders
      // In this case, just show empty list instead of error dialog
      if (
        errorMessage.includes('Failed to read directory') ||
        errorMessage.includes('does not exist') ||
        errorMessage.includes('No such file')
      ) {
        console.warn(
          '[VFS] Directory may not exist yet (newly created?), showing empty list',
        );
        setFiles([]);
        setLoading(false);
        return;
      }

      // Check for permission errors - these should show the detailed error message from backend
      if (
        errorMessage.includes('Permission Denied') ||
        errorMessage.includes('Operation not permitted') ||
        errorMessage.includes('os error 1') ||
        errorMessage.includes('Full Disk Access')
      ) {
        // Backend already provides detailed permission error message, show it as-is
        DialogService.error(errorMessage, 'Permission Error');
        setFiles([]);
        setLoading(false);
        return;
      }

      // Check for credential-related errors
      if (
        errorMessage.includes('ExpiredToken') ||
        errorMessage.includes('InvalidAccessKeyId') ||
        errorMessage.includes('InvalidToken') ||
        errorMessage.includes('SignatureDoesNotMatch') ||
        errorMessage.includes('Credential Error')
      ) {
        if (errorMessage.includes('ExpiredToken') && selectedSource?.id) {
          console.log(
            '[VFS] ExpiredToken detected, attempting to refresh S3 credentials...',
          );
          try {
            // Refresh credentials (will re-read environment variables)
            await invoke('vfs_refresh_s3_credentials', {
              sourceId: selectedSource.id,
              accessKey: null,
              secretKey: null,
              sessionToken: null,
            });
            console.log('[VFS] Credentials refreshed, retrying file load...');
            // Retry loading files after refresh
            await loadFilesList(sourceId, path);
            return;
          } catch (refreshErr) {
            console.error('[VFS] Failed to refresh credentials:', refreshErr);
            // Fall through to show error dialog
          }
        }

        // Show credential error with helpful message
        DialogService.error(
          `Credential Error: ${errorMessage}\n\n` +
            'Please check:\n' +
            '1. AWS_ACCESS_KEY_ID is set correctly\n' +
            '2. AWS_SECRET_ACCESS_KEY is set correctly\n' +
            '3. AWS_SESSION_TOKEN is set (if using temporary credentials)\n' +
            '4. Credentials are not expired\n' +
            '5. Credentials have s3:ListBucket permission',
          'AWS Credentials Error',
        );
        setFiles([]);
        setLoading(false);
        return;
      }

      // Check for permission errors
      if (
        errorMessage.includes('AccessDenied') ||
        errorMessage.includes('Forbidden') ||
        errorMessage.includes('Permission Error')
      ) {
        DialogService.error(
          `Permission Error: ${errorMessage}\n\n` +
            'Please check IAM permissions:\n' +
            '1. s3:ListBucket on the bucket\n' +
            '2. s3:GetObject on objects',
          'AWS Permission Error',
        );
        setFiles([]);
        setLoading(false);
        return;
      }

      setFiles([]);
      // Show user-friendly error message for other errors
      if (!errorMessage.includes('Missing providerId')) {
        // Don't show error for providerId issues (already handled above)
        DialogService.error(
          `Failed to load files: ${errorMessage}`,
          'Load Error',
        );
      }
    } finally {
      // Always clear loading state, but only update files if this is still the current request
      // This prevents loading spinner from getting stuck
      if (currentRequestRef.current?.sourceId === sourceId) {
        setLoading(false);
        // Clear isLoadingMore flag
        if (paginationState.isLoadingMore) {
          setPaginationState((prev) => ({ ...prev, isLoadingMore: false }));
        }
      } else {
        // Request was cancelled/switched, but still clear loading to prevent stuck spinner
        setLoading(false);
      }
      // Clean up abort controller if this was the current request
      if (abortControllerRef.current === abortController) {
        abortControllerRef.current = null;
      }
    }
  };

  // Load more files (pagination)
  const handleLoadMore = useCallback(() => {
    if (
      !selectedSource?.id ||
      !paginationState.hasMore ||
      paginationState.isLoadingMore ||
      loading
    ) {
      return;
    }

    setPaginationState((prev) => ({ ...prev, isLoadingMore: true }));
    loadFilesList(selectedSource.id, currentPath);
  }, [
    selectedSource?.id,
    currentPath,
    paginationState.hasMore,
    paginationState.isLoadingMore,
    loading,
  ]);

  // Load thumbnails for files that support them
  const loadThumbnailsForFiles = async (
    sourceId: string,
    fileList: FileMetadata[],
  ) => {
    const thumbnailTypes = [
      // Images
      'jpg',
      'jpeg',
      'png',
      'gif',
      'bmp',
      'tiff',
      'tif',
      'webp',
      'heic',
      'heif',
      'svg',
      'ico',
      'raw',
      'cr2',
      'nef',
      'arw',
      'dng',
      'orf',
      'rw2',
      'pef',
      'srw',
      'psd',
      'ai',
      'eps',
      // PDF
      'pdf',
      // Video
      'mp4',
      'mov',
      'avi',
      'mkv',
      'wmv',
      'flv',
      'webm',
      'm4v',
      'mpg',
      'mpeg',
      '3gp',
      'mxf',
      'prores',
      'r3d',
      'braw',
    ];

    // Filter files that can have thumbnails
    const filesToLoad = fileList.filter((f) => {
      if (f.isDirectory || f.thumbnail) return false;
      const ext = f.name.split('.').pop()?.toLowerCase() || '';
      return thumbnailTypes.includes(ext);
    });

    // Load thumbnails in very small batches with delays to prevent app crash
    const batchSize = 5; // Very small batches to prevent overwhelming the system
    let processedCount = 0;

    for (let i = 0; i < filesToLoad.length; i += batchSize) {
      const batch = filesToLoad.slice(i, i + batchSize);
      processedCount += batch.length;

      try {
        // Use batch thumbnail API for better performance
        const filePaths = batch.map((f) => f.path);
        const results = (await invoke('vfs_get_thumbnails_batch', {
          sourceId,
          filePaths,
          size: 128,
        })) as Array<[string, string | null]>;

        // Update files with thumbnails
        setFiles((prev) => {
          const updated = [...prev];
          const pathMap = new Map(results);

          return updated.map((f) => {
            const thumbnail = pathMap.get(f.path);
            return thumbnail ? { ...f, thumbnail } : f;
          });
        });
      } catch (error) {
        // Silently ignore thumbnail errors
        console.debug('Batch thumbnail request failed, skipping:', error);
      }

      // Longer delay between batches to prevent UI blocking and system overload
      if (i + batchSize < filesToLoad.length) {
        await new Promise((resolve) => setTimeout(resolve, 300));

        // Every 20 items, yield longer to prevent app freeze
        if (processedCount % 20 === 0) {
          await new Promise((resolve) => setTimeout(resolve, 200));
        }
      }
    }
  };

  // Load global favorites from localStorage
  const loadGlobalFavorites = () => {
    try {
      const stored = localStorage.getItem('diaspor-global-favorites');
      if (stored) {
        const parsed = JSON.parse(stored) as GlobalFavorite[];
        setFavorites(parsed);
      }
    } catch (err) {
      console.error('Failed to load global favorites:', err);
      setFavorites([]);
    }
  };

  // Save global favorites to localStorage
  const saveGlobalFavorites = (favs: GlobalFavorite[]) => {
    try {
      localStorage.setItem('diaspor-global-favorites', JSON.stringify(favs));
    } catch (err) {
      console.error('Failed to save global favorites:', err);
    }
  };

  // Add a file/folder to global favorites
  const addToGlobalFavorites = (file: FileMetadata, source: StorageSource) => {
    const newFavorite: GlobalFavorite = {
      id: `${source.id}:${file.path}`,
      name: file.name,
      path: file.path,
      sourceId: source.id,
      sourceName: source.name,
      isDirectory:
        file.isDirectory ||
        file.mimeType === 'folder' ||
        file.path.endsWith('/'),
      addedAt: new Date().toISOString(),
      order: favorites.length,
    };

    // Check if already exists
    if (favorites.some((f) => f.id === newFavorite.id)) {
      return;
    }

    const updated = [...favorites, newFavorite];
    setFavorites(updated);
    saveGlobalFavorites(updated);
  };

  // Remove from global favorites
  const removeFromGlobalFavorites = (favoriteId: string) => {
    const updated = favorites.filter((f) => f.id !== favoriteId);
    setFavorites(updated);
    saveGlobalFavorites(updated);
  };

  const loadTags = async (sourceId: string) => {
    try {
      const tagList = (await invoke('vfs_list_all_tags', {
        sourceId: sourceId,
      })) as {
        name: string;
        color?: string;
      }[];
      setAllTags(tagList);
    } catch (err) {
      console.error('Failed to load tags:', err);
      setAllTags([]);
    }
  };

  // Load global favorites on mount
  useEffect(() => {
    loadGlobalFavorites();
  }, []);

  // Check AI model availability
  const checkAIModels = useCallback(async () => {
    try {
      // Check if Ollama is running
      let isOllamaRunning = false;
      try {
        const result = await invoke<boolean>('check_ollama_running');
        isOllamaRunning = result === true;
      } catch (ollamaErr) {
        console.debug('[FinderPage] Ollama check failed:', ollamaErr);
        isOllamaRunning = false;
      }

      if (!isOllamaRunning) {
        setAiModelsAvailable(false);
        return;
      }

      // Check if there are any models CURRENTLY SERVING (not just installed)
      // Use ollama_ps to check for actively running models
      const servingModels = await invoke<{
        models: Array<{ name: string; model: string }>;
      }>('ollama_ps');

      const hasServingModels =
        servingModels?.models &&
        Array.isArray(servingModels.models) &&
        servingModels.models.length > 0;

      if (hasServingModels) {
        const modelNames = servingModels.models
          .map((m) => m.name || m.model)
          .join(', ');
        console.log(
          '[FinderPage] AI features enabled - Models serving:',
          modelNames,
        );
      }

      setAiModelsAvailable(hasServingModels);
    } catch (err) {
      console.debug('[FinderPage] AI models check failed:', err);
      setAiModelsAvailable(false);
    }
  }, []);

  // Check AI model availability on mount
  useEffect(() => {
    checkAIModels();
  }, [checkAIModels]);

  // Re-check AI models when window gains focus (user may have enabled models in another app)
  useEffect(() => {
    const handleFocus = () => {
      console.log('[FinderPage] Window focused, re-checking AI models...');
      checkAIModels();
    };

    window.addEventListener('focus', handleFocus);
    return () => window.removeEventListener('focus', handleFocus);
  }, [checkAIModels]);

  // Listen for AI settings changes
  useEffect(() => {
    const handleAISettingsChange = () => {
      console.log('[FinderPage] AI settings changed, re-checking models...');
      setTimeout(() => checkAIModels(), 1000); // Wait 1s for models to start
    };

    window.addEventListener('ai-settings-changed', handleAISettingsChange);
    return () =>
      window.removeEventListener('ai-settings-changed', handleAISettingsChange);
  }, [checkAIModels]);

  // Periodically check AI model availability (every 30 seconds)
  useEffect(() => {
    const interval = setInterval(() => {
      console.log('[FinderPage] Periodic AI models check...');
      checkAIModels();
    }, 30000); // 30 seconds

    return () => clearInterval(interval);
  }, [checkAIModels]);

  // Load tags when source changes
  useEffect(() => {
    if (selectedSource) {
      loadTags(selectedSource.id);
    }
  }, [selectedSource]);

  // Scroll selected source into view when it changes
  useEffect(() => {
    if (selectedSource) {
      // Use setTimeout to ensure DOM is updated after render
      setTimeout(() => {
        const selectedElement = document.querySelector(
          `[data-source-id="${selectedSource.id}"]`,
        ) as HTMLElement;
        if (selectedElement) {
          selectedElement.scrollIntoView({
            behavior: 'smooth',
            block: 'nearest',
            inline: 'nearest',
          });
        }
      }, 150);
    }
  }, [selectedSource?.id]);

  // Select a source (storage location) and navigate to its root
  const selectSource = useCallback(
    async (source: StorageSource, initialPath = '') => {
      // Always update selection to ensure focus remains on clicked item
      // This ensures the selection persists until user explicitly clicks another source
      const isSameSource = selectedSource?.id === source.id;

      setSelectedFiles(new Set());

      // Update source and path first to prevent layout shift
      // Always set selectedSource to maintain visual focus/selection
      // Use source from sources array to ensure stable reference
      const stableSource = sources.find((s) => s.id === source.id) || source;
      setSelectedSource(stableSource);

      // Only navigate/reset if path changed or source changed
      if (!isSameSource || currentPath !== initialPath) {
        setCurrentPath(initialPath);

        // Reset navigation history when switching sources
        if (!isSameSource) {
          setNavigationHistory([initialPath]);
          setHistoryIndex(0);
        }

        // Clear files after a microtask to allow React to update source/path first
        // This prevents MetricsPreview from shifting position
        await Promise.resolve();
        setFiles([]);

        // Explicitly load files for the new source (don't rely on useEffect)
        // This ensures files load immediately even if currentPath was already ''
        // For system locations, the mount point is already set in the source config,
        // so passing '' will list the contents of that mount point
        await loadFilesList(source.id, initialPath);
      }
      // If same source and same path, just ensure selection is maintained (no-op)
    },
    [selectedSource?.id, currentPath, sources],
  );

  // Navigate to a path and update history
  const navigateTo = async (path: string, addToHistory = true) => {
    // Normalize path
    const normalizedPath = path === '/' ? '' : path;

    // Don't navigate if already at this path
    if (normalizedPath === currentPath && selectedSource) return;

    setCurrentPath(normalizedPath);
    setSelectedFiles(new Set());

    // Add to history if this is a new navigation (not back/forward)
    if (addToHistory) {
      setNavigationHistory((prev) => {
        // Remove any forward history
        const newHistory = prev.slice(0, historyIndex + 1);
        // Add new path
        newHistory.push(normalizedPath);
        // Keep history manageable (max 50 entries)
        if (newHistory.length > 50) newHistory.shift();
        return newHistory;
      });
      setHistoryIndex((prev) => Math.min(prev + 1, 49));
    }

    // Load files for the new path if source is selected
    if (selectedSource) {
      await loadFilesList(selectedSource.id, normalizedPath);
    }
  };

  // Go back in navigation history
  const goBack = async () => {
    if (historyIndex > 0 && selectedSource) {
      const newIndex = historyIndex - 1;
      setHistoryIndex(newIndex);
      const path = navigationHistory[newIndex] || '';
      setCurrentPath(path);
      setSelectedFiles(new Set());
      await loadFilesList(selectedSource.id, path);
    }
  };

  // Go forward in navigation history
  const goForward = async () => {
    if (historyIndex < navigationHistory.length - 1 && selectedSource) {
      const newIndex = historyIndex + 1;
      setHistoryIndex(newIndex);
      const path = navigationHistory[newIndex] || '';
      setCurrentPath(path);
      setSelectedFiles(new Set());
      await loadFilesList(selectedSource.id, path);
    }
  };

  // Go up one directory level
  const goUp = async () => {
    if (!currentPath || !selectedSource) return;

    // Handle different path formats
    let parentPath = '';

    if (currentPath.includes('/')) {
      const parts = currentPath.split('/').filter(Boolean);
      parts.pop();
      parentPath = parts.length > 0 ? '/' + parts.join('/') : '';
    } else if (currentPath.includes('\\')) {
      const parts = currentPath.split('\\').filter(Boolean);
      parts.pop();
      parentPath = parts.length > 0 ? parts.join('\\') : '';
    }

    await navigateTo(parentPath);
  };

  // Check if we can go back/forward
  const canGoBack = historyIndex > 0;
  const canGoForward = historyIndex < navigationHistory.length - 1;
  const canGoUp = currentPath !== '';

  const handleFileClick = (file: FileMetadata, e: React.MouseEvent) => {
    // Don't handle right-click in click handler - let contextmenu handle it
    if (e.button === 2) {
      return;
    }

    if (e.metaKey || e.ctrlKey) {
      setSelectedFiles((prev) => {
        const next = new Set(prev);

        if (next.has(file.path)) next.delete(file.path);
        else next.add(file.path);
        return next;
      });
    } else if (e.shiftKey && selectedFiles.size > 0) {
      // Range selection
      const allPaths = files.map((f) => f.path);
      const lastSelected = Array.from(selectedFiles).pop();
      if (!lastSelected) return;
      const lastIdx = allPaths.indexOf(lastSelected);
      const currentIdx = allPaths.indexOf(file.path);
      const [start, end] = [
        Math.min(lastIdx, currentIdx),
        Math.max(lastIdx, currentIdx),
      ];
      const range = allPaths.slice(start, end + 1);
      setSelectedFiles(new Set(range));
    } else {
      setSelectedFiles(new Set([file.path]));
    }
  };

  const handleFileDoubleClick = async (file: FileMetadata) => {
    // Prevent infinite loops - if this file is already being processed, skip
    const fileKey = `${file.path}`;
    if (processingFilesRef.current.has(fileKey)) {
      console.warn('[VFS] File already being processed, skipping:', file.path);
      return;
    }

    processingFilesRef.current.add(fileKey);

    try {
      const isFolder =
        file.mimeType === 'folder' ||
        file.path.endsWith('/') ||
        file.isDirectory;

      if (isFolder) {
        // Build the full path for the folder
        let targetPath = file.path;

        // Remove trailing slash if present
        if (targetPath.endsWith('/')) {
          targetPath = targetPath.slice(0, -1);
        }

        // If the path is relative (doesn't start with / or drive letter), make it absolute
        if (
          currentPath &&
          !targetPath.startsWith('/') &&
          !targetPath.match(/^[A-Za-z]:\\/)
        ) {
          targetPath = currentPath + '/' + file.name;
        }

        await navigateTo(targetPath);
      } else {
        // Open file with default application
        await handleOpenFile(file);
      }
    } finally {
      // Remove from processing set after a short delay to allow navigation to complete
      setTimeout(() => {
        processingFilesRef.current.delete(fileKey);
      }, 1000);
    }
  };

  // Open file with default application (only for files, not folders)
  const handleOpenFile = async (file: FileMetadata) => {
    if (!selectedSource) return;

    // Prevent infinite loops - if this file is already being processed, skip
    const fileKey = `${file.path}`;
    if (processingFilesRef.current.has(fileKey)) {
      console.warn(
        '[VFS] File already being processed in handleOpenFile, skipping:',
        file.path,
      );
      return;
    }

    processingFilesRef.current.add(fileKey);

    try {
      const isFolder =
        file.mimeType === 'folder' ||
        file.path.endsWith('/') ||
        file.isDirectory;

      if (isFolder) {
        processingFilesRef.current.delete(fileKey);
        handleFileDoubleClick(file);
        return;
      }

      // Auto-tag and auto-transcode video/image files
      const mimeType = file.mimeType || '';
      const isVideo =
        mimeType.startsWith('video/') ||
        ['mp4', 'mov', 'avi', 'mkv', 'webm', 'm4v'].some((ext) =>
          file.name.toLowerCase().endsWith(`.${ext}`),
        );
      const isImage =
        mimeType.startsWith('image/') ||
        ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp'].some((ext) =>
          file.name.toLowerCase().endsWith(`.${ext}`),
        );

      if (isVideo || isImage) {
        // Ensure models are running
        try {
          await invoke('vfs_ensure_models_running', {
            operationType: isVideo ? 'tagging' : 'tagging',
          });

          // Auto-tag the file
          try {
            await invoke('vfs_auto_tag_file', {
              sourceId: selectedSource.id,
              filePath: file.path,
            });
          } catch (tagErr) {
            // Silently fail - tagging is optional
            console.log('Auto-tagging failed (non-critical):', tagErr);
          }

          // Auto-transcode video files
          if (isVideo) {
            try {
              await invoke('vfs_auto_transcode', {
                sourceId: selectedSource.id,
                filePath: file.path,
              });
            } catch (transcodeErr) {
              // Silently fail - transcoding is optional
              console.log(
                'Auto-transcoding failed (non-critical):',
                transcodeErr,
              );
            }
          }
        } catch (modelErr) {
          // Silently fail - model operations are optional
          console.log('Model operations failed (non-critical):', modelErr);
        }
      }

      await invoke('vfs_open_file', {
        sourceId: selectedSource.id,
        path: file.path,
      });
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);

      // If error is about directories, silently navigate instead
      // This handles cases where isDirectory flag wasn't set correctly
      if (
        errorMsg.includes('directory') ||
        errorMsg.includes('Cannot open directory')
      ) {
        // Remove from processing set before calling handleFileDoubleClick to allow navigation
        processingFilesRef.current.delete(fileKey);
        // Silently navigate - no error dialog needed
        handleFileDoubleClick(file);
        return;
      }

      console.error('Failed to open file:', err);
      DialogService.error(`Failed to open file: ${errorMsg}`, 'Open Error');
    } finally {
      setTimeout(() => {
        processingFilesRef.current.delete(fileKey);
      }, 500);
    }
  };

  // Open file with specific application
  const handleOpenFileWith = async (file: FileMetadata, appPath: string) => {
    if (!selectedSource) {
      DialogService.error('No storage source selected', 'Open Error');
      return;
    }

    if (!appPath || appPath.trim() === '') {
      DialogService.error('No application selected', 'Open Error');
      return;
    }

    try {
      // On macOS, normalize .app bundle paths
      // The dialog might return the executable path inside the bundle, but we need the .app bundle path
      let normalizedAppPath = appPath.trim();

      const platformInfo = getPlatformInfoSync();
      if (platformInfo.isMac) {
        // If path contains .app/Contents/MacOS, extract just the .app bundle path
        const appBundleMatch = normalizedAppPath.match(/^(.+\.app)/);
        if (appBundleMatch) {
          normalizedAppPath = appBundleMatch[1];
        }
        // Ensure it ends with .app
        if (!normalizedAppPath.endsWith('.app')) {
          // Try to find the .app bundle in the path
          const parts = normalizedAppPath.split('/');
          const appIndex = parts.findIndex((part) => part.endsWith('.app'));
          if (appIndex !== -1) {
            normalizedAppPath = parts.slice(0, appIndex + 1).join('/');
          }
        }
      }

      await invoke('vfs_open_file_with', {
        sourceId: selectedSource.id,
        path: file.path,
        appPath: normalizedAppPath,
      });
    } catch (err) {
      console.error('Failed to open file with app:', err);
      const errorMessage = err instanceof Error ? err.message : String(err);
      DialogService.error(
        `Failed to open file with application: ${errorMessage}`,
        'Open Error',
      );
    }
  };

  // Get available apps for a file
  const [availableApps, setAvailableApps] = useState<
    { name: string; path: string }[]
  >([]);
  const [appsLoading, setAppsLoading] = useState(false);
  const [showOpenWith, setShowOpenWith] = useState(false);

  const loadAppsForFile = async (file: FileMetadata) => {
    setAppsLoading(true);
    try {
      const apps = (await invoke('vfs_get_apps_for_file', {
        filePath: file.path,
      })) as { name: string; path: string; bundle_id?: string }[];

      // Deduplicate apps on frontend as safety measure
      // Check by bundle_id, path, and name (case-insensitive)
      const seen = new Set<string>();
      const deduplicated = apps.filter((app) => {
        // Check by bundle_id first (most reliable)
        if (app.bundle_id) {
          const key = `bundle:${app.bundle_id}`;
          if (seen.has(key)) return false;
          seen.add(key);
        }

        // Check by normalized path
        const normalizedPath = app.path.toLowerCase().replace(/\/$/, '');
        const pathKey = `path:${normalizedPath}`;
        if (seen.has(pathKey)) return false;
        seen.add(pathKey);

        // Check by name (case-insensitive) as fallback
        const nameKey = `name:${app.name.toLowerCase()}`;
        if (seen.has(nameKey)) return false;
        seen.add(nameKey);

        return true;
      });

      setAvailableApps(deduplicated);
    } catch (err) {
      console.error('Failed to get apps:', err);
      setAvailableApps([]);
    } finally {
      setAppsLoading(false);
    }
  };

  const handleWarm = async (file: FileMetadata) => {
    if (!selectedSource) return;
    try {
      await invoke('vfs_warm_file', {
        sourceId: selectedSource.id,
        path: file.path,
        priority: 'high',
      });
    } catch (err) {
      console.error(err);
    }
  };

  const handleTranscribe = async (file: FileMetadata) => {
    if (!selectedSource) return;

    // Use both mimeType and extension for detection
    const extension = file.name.split('.').pop()?.toLowerCase() || '';
    const videoExts = [
      'mp4',
      'mov',
      'avi',
      'mkv',
      'webm',
      'wmv',
      'flv',
      'm4v',
      'mpg',
      'mpeg',
    ];
    const audioExts = [
      'mp3',
      'wav',
      'aiff',
      'flac',
      'm4a',
      'aac',
      'ogg',
      'wma',
    ];
    const imageExts = [
      'jpg',
      'jpeg',
      'png',
      'gif',
      'webp',
      'bmp',
      'tiff',
      'tif',
      'heic',
      'heif',
    ];

    const isMediaFile =
      file.mimeType?.startsWith('video/') ||
      file.mimeType?.startsWith('audio/') ||
      videoExts.includes(extension) ||
      audioExts.includes(extension);
    const isImageFile =
      file.mimeType?.startsWith('image/') || imageExts.includes(extension);
    const isPdfFile =
      file.mimeType === 'application/pdf' || extension === 'pdf';

    // Support video, audio, PDF, and images
    if (!isMediaFile && !isPdfFile && !isImageFile) {
      // For unsupported types, redirect to AI settings
      if (onOpenSettings) {
        window.dispatchEvent(new CustomEvent('open-ai-settings'));
        onOpenSettings();
      }
      return;
    }

    try {
      // Check if transcription is available
      const isAvailable = await invoke<boolean>(
        'vfs_is_transcription_available',
      );
      if (!isAvailable) {
        // Redirect to AI settings
        if (onOpenSettings) {
          window.dispatchEvent(new CustomEvent('open-ai-settings'));
          onOpenSettings();
        }
        return;
      }

      // Get available models
      const models = await invoke<string[]>('vfs_get_transcription_models');
      if (models.length === 0) {
        // Show dialog with option to go to Settings
        const goToSettings = await DialogService.confirm({
          title: 'No Transcription Models',
          message:
            'No transcription models found. Would you like to go to Settings to install a transcription model (e.g., whisper)?',
          okLabel: 'Go to Settings',
          cancelLabel: 'Cancel',
          type: DialogType.Warning,
        });

        if (goToSettings && onOpenSettings) {
          onOpenSettings();
        }
        return;
      }

      // Use the first available model (or let user choose in future)
      const model = models[0];

      // Transcribe the file - TranscriptionProgressPanel will show progress
      setFileOperation({ type: 'Transcribing...', inProgress: true });

      const result = await invoke<{
        operation_id: string;
        segments: Array<{
          text: string;
          start_time: number;
          end_time: number;
          confidence?: number;
        }>;
      }>('vfs_transcribe_file', {
        sourceId: selectedSource.id,
        path: file.path,
        model: model,
        language: null, // Auto-detect language
      });

      // Dispatch transcribe-started event for OperationsPanel
      if (result.operation_id) {
        console.log(
          '[Transcribe] ✅ Dispatching transcribe-started event:',
          result.operation_id,
        );
        window.dispatchEvent(
          new CustomEvent('transcribe-started', {
            detail: {
              operationId: result.operation_id,
              fileName: file.name,
              filePath: file.path,
              sourceId: selectedSource.id,
            },
          }),
        );
      }

      if (result.segments.length === 0) {
        DialogService.error(
          'Transcription Complete',
          'No speech detected in file.',
        );
        setFileOperation(null);
        return;
      }

      // Save transcription next to the original file
      const basePath = file.path.substring(0, file.path.lastIndexOf('.'));
      const transcriptPath = `${basePath}.srt`;

      await invoke('vfs_save_transcription', {
        operation_id: result.operation_id,
        dest_path: transcriptPath,
        format: 'srt',
      });

      setFileOperation({
        type: 'Transcription saved successfully',
        inProgress: false,
      });
      setTimeout(() => setFileOperation(null), 2000);

      // Refresh file list to show the new transcript file
      setTimeout(async () => {
        await loadFilesList(selectedSource.id, currentPath);
      }, 500);
    } catch (err) {
      console.error('Transcription error:', err);
      const errorMsg = err instanceof Error ? err.message : String(err);
      DialogService.error(
        'Transcription Failed',
        `Failed to transcribe file: ${errorMsg}`,
      );
      setFileOperation({
        type: `Transcription failed: ${errorMsg}`,
        inProgress: false,
      });
      setTimeout(() => setFileOperation(null), 3000);
    }
  };

  // Handle AI auto-tagging using LLaVA
  const handleAutoTag = async (file: FileMetadata) => {
    if (!selectedSource) return;

    // Check file type - use both mimeType and extension as fallback
    const extension = file.name.split('.').pop()?.toLowerCase() || '';
    const videoExtensions = [
      'mp4',
      'mov',
      'avi',
      'mkv',
      'webm',
      'wmv',
      'flv',
      'm4v',
      'mpg',
      'mpeg',
    ];
    const imageExtensions = [
      'jpg',
      'jpeg',
      'png',
      'gif',
      'webp',
      'bmp',
      'tiff',
      'tif',
      'heic',
      'heif',
      'svg',
    ];
    const audioExtensions = [
      'mp3',
      'wav',
      'aiff',
      'flac',
      'm4a',
      'aac',
      'ogg',
      'wma',
    ];

    const isMediaFile =
      file.mimeType?.startsWith('video/') ||
      file.mimeType?.startsWith('audio/') ||
      videoExtensions.includes(extension) ||
      audioExtensions.includes(extension);
    const isImageFile =
      file.mimeType?.startsWith('image/') ||
      imageExtensions.includes(extension);

    if (!isMediaFile && !isImageFile) {
      DialogService.info(
        'AI Tagging',
        'AI tagging is only available for images and videos.',
      );
      return;
    }

    // Check if tagging is available
    try {
      const isOllamaRunning = await invoke<boolean>('check_ollama_running');
      if (!isOllamaRunning) {
        const goToSettings = await DialogService.confirm({
          title: 'Ollama Not Running',
          message:
            'Ollama is required for AI tagging. Would you like to go to Settings to start it?',
          okLabel: 'Go to Settings',
          cancelLabel: 'Cancel',
          type: DialogType.Warning,
        });

        if (goToSettings && onOpenSettings) {
          window.dispatchEvent(new CustomEvent('open-ai-settings'));
          onOpenSettings();
        }
        return;
      }

      // Check if LLaVA model is available
      const response = await fetch('http://localhost:11434/api/tags');
      if (response.ok) {
        const data = (await response.json()) as {
          models: Array<{ name: string }>;
        };
        const modelNames = data.models.map((m) => m.name.toLowerCase());
        const hasLlava = modelNames.some((n) => n.includes('llava'));

        if (!hasLlava) {
          const install = await DialogService.confirm({
            title: 'LLaVA Model Required',
            message:
              'LLaVA model is required for AI tagging but not installed. Would you like to install it?',
            okLabel: 'Install LLaVA',
            cancelLabel: 'Cancel',
            type: DialogType.Info,
          });

          if (install) {
            setFileOperation({
              type: 'Installing LLaVA model...',
              inProgress: true,
            });
            try {
              await invoke('ollama_pull', { model: 'llava' });
              setFileOperation({ type: 'LLaVA installed!', inProgress: false });
              setTimeout(() => setFileOperation(null), 2000);
            } catch (pullErr) {
              const errMsg =
                pullErr instanceof Error ? pullErr.message : String(pullErr);
              DialogService.error('Installation Failed', errMsg);
              setFileOperation(null);
            }
          }
          return;
        }
      }

      // Perform AI tagging (runs as background operation)
      setFileOperation({ type: 'Starting AI tagging...', inProgress: true });

      const result = await invoke<{
        success: boolean;
        tags: string[];
        message: string;
        operation_id?: string;
      }>('vfs_auto_tag_file', {
        sourceId: selectedSource.id,
        filePath: file.path,
      });

      // Dispatch autotag-started event for OperationsPanel
      if (result.operation_id) {
        console.log(
          '[AutoTag] ✅ Dispatching autotag-started event:',
          result.operation_id,
        );
        window.dispatchEvent(
          new CustomEvent('autotag-started', {
            detail: {
              operationId: result.operation_id,
              fileName: file.name,
              filePath: file.path,
              sourceId: selectedSource.id,
            },
          }),
        );
      }

      if (result.success && result.tags.length > 0) {
        setFileOperation({
          type: `Generated ${result.tags.length} tags`,
          inProgress: false,
        });
        setTimeout(() => setFileOperation(null), 2000);

        // Update InfoModal if open with the new tags
        if (infoModal.visible && infoModal.file?.path === file.path) {
          const newTags = result.tags.map((t) => ({ name: t }));
          const existingTags = infoModal.file.tags || [];
          const existingTagNames = existingTags.map((t) =>
            typeof t === 'string' ? t : t.name,
          );
          const mergedTags = [
            ...existingTags,
            ...newTags.filter((t) => !existingTagNames.includes(t.name)),
          ];
          setInfoModal({
            visible: true,
            file: { ...infoModal.file, tags: mergedTags },
          });
        }

        // Refresh the file list to show updated tags
        setTimeout(async () => {
          await loadFilesList(selectedSource.id, currentPath);
        }, 500);
      } else {
        setFileOperation({
          type: 'No tags generated',
          inProgress: false,
        });
        setTimeout(() => setFileOperation(null), 2000);
      }
    } catch (err) {
      console.error('Auto-tag error:', err);
      const errorMsg = err instanceof Error ? err.message : String(err);
      setFileOperation({
        type: `Tagging failed: ${errorMsg}`,
        inProgress: false,
      });
      setTimeout(() => setFileOperation(null), 3000);
    }
  };

  const handleTranscode = async (file: FileMetadata) => {
    if (!selectedSource) return;
    try {
      await invoke('vfs_transcode_video', {
        sourceId: selectedSource.id,
        path: file.path,
        format: 'hls',
      });
    } catch (err) {
      console.error(err);
    }
  };

  // =========================================================================
  // File Operations - Delete, Rename, Duplicate, etc.
  // =========================================================================

  const handleDelete = async () => {
    if (!selectedSource || selectedFiles.size === 0) return;

    // Delete is supported on all storage types (local, network, cloud object storage)
    // Object storage (S3, GCS, Azure Blob) fully supports delete operations

    const paths = Array.from(selectedFiles);

    // Get file names from the files array for better confirmation message
    const fileNames = paths.map((path) => {
      const file = files.find((f) => f.path === path);
      if (file) {
        return file.name;
      }
      // Fallback: extract filename from path if not found in files array
      return path.split('/').filter(Boolean).pop() || path;
    });

    // Build confirmation message with file names
    let confirmMessage: string;
    if (fileNames.length === 1) {
      confirmMessage = `Delete "${fileNames[0]}"?\n\nThis cannot be undone.`;
    } else if (fileNames.length <= 5) {
      confirmMessage = `Delete ${fileNames.length} file(s)?\n\n${fileNames.map((name) => `• ${name}`).join('\n')}\n\nThis cannot be undone.`;
    } else {
      confirmMessage = `Delete ${fileNames.length} file(s)?\n\n${fileNames
        .slice(0, 5)
        .map((name) => `• ${name}`)
        .join(
          '\n',
        )}\n... and ${fileNames.length - 5} more\n\nThis cannot be undone.`;
    }

    const confirmDelete = await DialogService.confirm({
      title: 'Delete File(s)',
      message: confirmMessage,
      type: DialogType.Warning,
      okLabel: 'Delete',
      cancelLabel: 'Cancel',
    });
    if (!confirmDelete) return;

    setFileOperation({
      inProgress: true,
      type: `Deleting ${paths.length} item(s)`,
    });

    // Add timeout to prevent hanging
    const timeoutId = setTimeout(() => {
      console.warn('[VFS Delete] Operation timed out after 30 seconds');
      setFileOperation({
        inProgress: false,
        type: 'Delete operation timed out',
      });
      setTimeout(() => setFileOperation(null), 3000);
      DialogService.error(
        'Delete operation timed out. Some files may still be deleting in the background.',
        'Delete Timeout',
      );
    }, 30000); // 30 second timeout

    try {
      // Normalize all paths
      const normalizedPaths = paths.map((path) => path.replace(/\/+/g, '/'));

      // Use bulk delete command for multiple files (single operation with progress tracking)
      if (normalizedPaths.length > 1) {
        // Bulk delete - creates a single operation with progress tracking
        const operationId = await invoke<string>('vfs_batch_delete', {
          sourceId: selectedSource.id,
          paths: normalizedPaths,
        });

        // Trigger delete-started event for OperationsPanel and TransferPanel
        // IMPORTANT: Dispatch immediately (not in setTimeout) so modal shows operation even if it completes quickly
        console.log('[VFS Delete] ✅ Operation tracked:', operationId, {
          paths: normalizedPaths,
          count: normalizedPaths.length,
        });
        console.log(
          '[VFS Delete] 📤 Dispatching delete-started event immediately:',
          operationId,
        );
        window.dispatchEvent(
          new CustomEvent('delete-started', {
            detail: { operationId },
          }),
        );
        console.log('[VFS Delete] ✅ Event dispatched successfully');
      } else {
        // Single file delete - still use batch for consistency and progress tracking
        const operationId = await invoke<string>('vfs_batch_delete', {
          sourceId: selectedSource.id,
          paths: normalizedPaths,
        });

        // Trigger delete-started event for OperationsPanel and TransferPanel
        // IMPORTANT: Dispatch immediately (not in setTimeout) so modal shows operation even if it completes quickly
        console.log('[VFS Delete] ✅ Operation tracked:', operationId, {
          paths: normalizedPaths,
          count: normalizedPaths.length,
        });
        console.log(
          '[VFS Delete] 📤 Dispatching delete-started event immediately:',
          operationId,
        );
        window.dispatchEvent(
          new CustomEvent('delete-started', {
            detail: { operationId },
          }),
        );
        console.log('[VFS Delete] ✅ Event dispatched successfully');
      }

      // Clear timeout since operation completed
      clearTimeout(timeoutId);

      // Clear selection
      setSelectedFiles(new Set());

      // Refresh file list (with timeout to prevent hanging)
      // Note: OperationsPanel will show progress, so we refresh after a delay
      setTimeout(async () => {
        try {
          const refreshPromise = loadFilesList(selectedSource.id, currentPath);
          const refreshTimeout = new Promise((_, reject) => {
            setTimeout(() => reject(new Error('Refresh timeout')), 5000);
          });
          await Promise.race([refreshPromise, refreshTimeout]);
        } catch (refreshErr) {
          console.warn(
            '[VFS Delete] File list refresh failed or timed out:',
            refreshErr,
          );
        }
      }, 1000); // Delay refresh to allow operation to start

      // Clear file operation state - OperationsPanel handles progress display
      setFileOperation({
        inProgress: false,
        type: `Deleting ${paths.length} item(s)`,
      });
      setTimeout(() => setFileOperation(null), 2000);
    } catch (err) {
      // Clear timeout on error
      clearTimeout(timeoutId);

      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[VFS Delete] Batch delete failed:', errorMessage);

      setFileOperation({
        inProgress: false,
        type: `Delete failed: ${errorMessage}`,
      });
      setTimeout(() => setFileOperation(null), 3000);

      DialogService.error(
        `Failed to delete files: ${errorMessage}`,
        'Delete Error',
      );
    }
  };

  // Handle move operation (for drag and drop)
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const _handleMoveFiles = async (
    sourceId: string,
    moves: Array<{ from_path: string; to_path: string }>,
  ) => {
    if (!selectedSource || moves.length === 0) return;

    if (!supportsFilesystemOperations(selectedSource)) {
      DialogService.error(
        'Move not supported',
        'Move is only available for file system storage (local, network, hybrid).',
      );
      return;
    }

    setFileOperation({
      inProgress: true,
      type: `Moving ${moves.length} item(s)`,
    });

    try {
      // Use bulk move command for multiple files (single operation with progress tracking)
      const operationId = await invoke<string>('vfs_batch_move', {
        sourceId: sourceId,
        moves: moves,
      });

      // Trigger move-started event for OperationsPanel
      // IMPORTANT: Dispatch immediately (not in setTimeout) so modal shows operation even if it completes quickly
      console.log(
        '[VFS Move] 📤 Dispatching move-started event immediately:',
        operationId,
      );
      window.dispatchEvent(
        new CustomEvent('move-started', {
          detail: { operationId },
        }),
      );
      console.log('[VFS Move] ✅ Event dispatched successfully');

      // Clear file operation state - OperationsPanel handles progress display
      setFileOperation({
        inProgress: false,
        type: `Moving ${moves.length} item(s)`,
      });
      setTimeout(() => setFileOperation(null), 2000);

      // Refresh file list after a delay
      setTimeout(async () => {
        try {
          await loadFilesList(selectedSource.id, currentPath);
        } catch (refreshErr) {
          console.warn('[VFS Move] File list refresh failed:', refreshErr);
        }
      }, 1000);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error('[VFS Move] Batch move failed:', errorMessage);

      setFileOperation({
        inProgress: false,
        type: `Move failed: ${errorMessage}`,
      });
      setTimeout(() => setFileOperation(null), 3000);

      DialogService.error(
        `Failed to move files: ${errorMessage}`,
        'Move Error',
      );
    }
  };

  // Rename file/folder
  // Start inline rename (like macOS Finder)
  const handleRename = (file: FileMetadata) => {
    if (!selectedSource) {
      return;
    }

    // Rename works for files on all storage types (local, network, cloud object storage)
    // Prevent folder rename for object storage (folders are prefixes, not real objects)
    const isFolder =
      file.isDirectory || file.mimeType === 'folder' || file.path.endsWith('/');
    if (isFolder && isObjectStorage(selectedSource)) {
      DialogService.error(
        'Folder rename not supported',
        'Object storage does not support renaming folders. Please rename individual files.',
      );
      return;
    }

    // Get file name without extension for pre-selection
    const name = file.name;
    const dotIndex = name.lastIndexOf('.');

    setRenamingFile(file.path);
    setRenameValue(name);
    setSelectedFiles(new Set([file.path]));

    // Focus and select the name (without extension for files)
    setTimeout(() => {
      if (renameInputRef.current) {
        renameInputRef.current.focus();
        if (!isFolder && dotIndex > 0) {
          renameInputRef.current.setSelectionRange(0, dotIndex);
        } else {
          renameInputRef.current.select();
        }
      }
    }, 10);
  };

  // Commit the rename
  const commitRename = async () => {
    if (!selectedSource || !renamingFile) return;

    const file = files.find((f) => f.path === renamingFile);
    if (!file) {
      setRenamingFile(null);
      return;
    }

    const newName = renameValue.trim();
    if (!newName || newName === file.name) {
      setRenamingFile(null);
      return;
    }

    // Validate name
    if (newName.includes('/') || newName.includes('\\')) {
      DialogService.warning(
        'File names cannot contain slashes',
        'Invalid Name',
      );
      return;
    }

    setFileOperation({ type: 'Renaming...', inProgress: true });

    try {
      // Construct the new path
      const pathParts = file.path.split('/');
      pathParts.pop();
      const newPath =
        pathParts.length > 0
          ? `${pathParts.join('/')}/${newName}`
          : `/${newName}`;

      const result = await invoke<string>('vfs_rename', {
        sourceId: selectedSource.id,
        oldPath: file.path,
        newPath: newPath,
      });

      // vfs_rename returns operation_id as string on success, or error with |OPERATION_ID: on failure
      // Extract operation_id from result
      let operationId: string | null = null;
      if (result.includes('|OPERATION_ID:')) {
        // Error case: extract operation_id from error message
        const parts = result.split('|OPERATION_ID:');
        operationId = parts[1]?.trim() || null;
      } else {
        // Success case: result IS the operation_id
        operationId = result.trim() || null;
      }

      // Dispatch rename-started event for OperationsPanel and TransferPanel
      // This ensures rename operations are tracked and displayed in both panels
      // IMPORTANT: Dispatch immediately (not in setTimeout) so modal shows operation even if it completes quickly
      if (operationId) {
        console.log(
          '[VFS Rename] Dispatching rename-started event immediately:',
          operationId,
        );
        window.dispatchEvent(
          new CustomEvent('rename-started', {
            detail: { operationId },
          }),
        );
      } else {
        console.warn(
          '[VFS Rename] No operation_id returned from rename operation',
        );
      }

      setRenamingFile(null);

      // Refresh and select the renamed file
      await loadFilesList(selectedSource.id, currentPath);
      setSelectedFiles(new Set([newPath]));
    } catch (err) {
      console.error('Rename failed:', err);

      // Extract operation_id from error message if present
      // Format: "error_message|OPERATION_ID:operation_id"
      const errorMessage = err instanceof Error ? err.message : String(err);
      let operationId: string | null = null;
      if (errorMessage.includes('|OPERATION_ID:')) {
        const parts = errorMessage.split('|OPERATION_ID:');
        operationId = parts[1] || null;
        // Extract clean error message (without operation_id)
        const cleanError = parts[0] || errorMessage;
        DialogService.error(cleanError, 'Rename Error');

        // Dispatch rename-started event even for failed operations so they appear in OperationsPanel and TransferPanel
        // IMPORTANT: Dispatch immediately (not in setTimeout) so modal shows operation even if it completes quickly
        if (operationId) {
          console.log(
            '[VFS Rename] Dispatching rename-started event for failed operation immediately:',
            operationId,
          );
          window.dispatchEvent(
            new CustomEvent('rename-started', {
              detail: { operationId },
            }),
          );
        }
      } else {
        DialogService.error(`Rename failed: ${errorMessage}`, 'Rename Error');
      }
    } finally {
      setFileOperation(null);
    }
  };

  // Cancel rename
  const cancelRename = () => {
    setRenamingFile(null);
    setRenameValue('');
  };

  // Handle rename input keydown
  const handleRenameKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitRename();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      cancelRename();
    }
  };

  // Create new folder with inline naming (like macOS Finder)
  // Optional targetPath for creating inside a specific folder
  const handleNewFolder = async (targetPath?: string) => {
    if (!selectedSource) return;

    // Object storage (S3, GCS, Azure) supports creating folders via directory markers
    // Only block folder creation for unsupported storage types
    // Note: supportsFilesystemOperations returns false for object storage, but mkdir is still supported
    const isObjectStorageType = isObjectStorage(selectedSource);

    if (!isObjectStorageType && !supportsFilesystemOperations(selectedSource)) {
      DialogService.error(
        'Create folder not supported',
        'Create folder is only available for file system storage (local, network, hybrid) or object storage (S3, GCS, Azure).',
      );
      return;
    }

    setFileOperation({ type: 'Creating folder...', inProgress: true });

    try {
      // Determine the parent directory
      const parentPath = targetPath || currentPath;

      // Find a unique name
      let folderName = 'untitled folder';
      let counter = 1;
      const existingNames = new Set(files.map((f) => f.name.toLowerCase()));

      while (existingNames.has(folderName.toLowerCase())) {
        counter++;
        folderName = `untitled folder ${counter}`;
      }

      const newPath =
        parentPath === '/' || parentPath === ''
          ? `/${folderName}`
          : `${parentPath.replace(/\/$/, '')}/${folderName}`;

      const operationId = await invoke<string>('vfs_mkdir', {
        sourceId: selectedSource.id,
        path: newPath,
      });

      // Trigger mkdir-started event for OperationsPanel and TransferPanel
      // IMPORTANT: Dispatch immediately (not in setTimeout) so modal shows operation even if it completes quickly
      if (operationId) {
        console.log(
          '[VFS Mkdir] Dispatching mkdir-started event immediately:',
          operationId,
        );
        window.dispatchEvent(
          new CustomEvent('mkdir-started', {
            detail: { operationId },
          }),
        );
      }

      // Refresh file list and wait for it to complete
      await loadFilesList(selectedSource.id, currentPath);

      // Set state for renaming immediately
      setRenamingFile(newPath);
      setRenameValue(folderName);
      setSelectedFiles(new Set([newPath]));

      // Use requestAnimationFrame for proper DOM timing
      // This ensures React has re-rendered with the new folder
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          // Scroll the new folder into view
          const folderElement = document.querySelector(
            `[data-path="${CSS.escape(newPath)}"]`,
          );
          if (folderElement) {
            folderElement.scrollIntoView({
              behavior: 'smooth',
              block: 'center',
            });
          }

          // Focus and select the rename input
          if (renameInputRef.current) {
            renameInputRef.current.focus();
            renameInputRef.current.select();
          }
        });
      });
    } catch (err) {
      console.error('Create folder failed:', err);
      DialogService.error(`Create folder failed: ${err}`, 'Folder Error');
    } finally {
      setFileOperation(null);
    }
  };

  // ============================================================================
  // Drag and Drop Handlers - Native FS-like behavior
  // ============================================================================

  // Start dragging file(s)
  const handleDragStart = (e: React.DragEvent, file: FileMetadata) => {
    console.log('[handleDragStart] Starting drag for file:', file.path);

    // If dragging a non-selected file, select only that file
    const filesToDrag = selectedFiles.has(file.path)
      ? Array.from(selectedFiles)
      : [file.path];

    // Get full file objects for the dragged files
    const fileObjects = filesToDrag
      .map((path) => files.find((f) => f.path === path))
      .filter((f): f is FileMetadata => f !== undefined);

    // If dragging a single non-selected file, use that file directly
    if (!selectedFiles.has(file.path)) {
      fileObjects.length = 0;
      fileObjects.push(file);
    }

    console.log('[handleDragStart] Setting drag state:', {
      filesToDrag,
      fileObjectsCount: fileObjects.length,
      sourceId: selectedSource?.id,
    });

    setDraggedFiles(filesToDrag);
    setDraggedFileObjects(fileObjects);
    setDragSourceId(selectedSource?.id || null);

    // Set drag data for native drop targets (Finder/Explorer)
    e.dataTransfer.effectAllowed = 'copyMove';
    e.dataTransfer.setData('text/plain', filesToDrag.join('\n'));

    const vfsData = JSON.stringify({
      sourceId: selectedSource?.id,
      paths: filesToDrag,
    });
    e.dataTransfer.setData('application/x-vfs-files', vfsData);

    console.log('[handleDragStart] Set drag data:', {
      textPlain: filesToDrag.join('\n'),
      vfsData,
    });

    // Create custom drag image showing file count (uses CSS from finder.css)
    const dragImage = document.createElement('div');
    dragImage.className = 'drag-ghost';
    dragImage.innerHTML = `
      ${filesToDrag.length > 1 ? `<span class="drag-count">${filesToDrag.length}</span>` : ''}
      <span class="drag-label">${filesToDrag.length === 1 ? file.name : `${filesToDrag.length} items`}</span>
    `;
    document.body.appendChild(dragImage);
    e.dataTransfer.setDragImage(dragImage, 20, 20);

    // Clean up after a short delay (must stay in DOM for setDragImage to work)
    requestAnimationFrame(() => {
      setTimeout(() => dragImage.remove(), 0);
    });
  };

  // Drag over a folder or content area
  const handleDragOver = (
    e: React.DragEvent,
    targetPath?: string,
    isFolder?: boolean,
  ) => {
    e.preventDefault();
    e.stopPropagation();

    // Only log occasionally to avoid spam
    if (Math.random() < 0.01) {
      console.log('[handleDragOver] Drag over:', {
        targetPath,
        isFolder,
        draggedFilesCount: draggedFiles.length,
        dragSourceId,
      });
    }

    // Determine drop effect (like native file systems):
    // - Same source = MOVE by default (like dragging within same drive)
    // - Different source = COPY by default (like dragging between drives)
    // - Shift key = Force MOVE
    // - Ctrl/Cmd key = Force COPY
    const isSameSource = dragSourceId === selectedSource?.id;
    const forceMove = e.shiftKey;
    const forceCopy = e.ctrlKey || e.metaKey; // Ctrl on Windows/Linux, Cmd on macOS

    let isMove: boolean;
    if (forceCopy) {
      isMove = false; // Force copy
    } else if (forceMove) {
      isMove = true; // Force move
    } else {
      isMove = isSameSource; // Default: move if same source, copy if different
    }

    e.dataTransfer.dropEffect = isMove ? 'move' : 'copy';

    setIsDraggingOver(true);

    // Only set folder as drop target (not individual files)
    if (isFolder && targetPath !== undefined) {
      setDropTarget(targetPath);
    } else if (targetPath === undefined) {
      // Dragging over empty area in the content view
      setDropTarget(null);
    }
  };

  // Drag leaves the drop zone
  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    // Only clear drop target when truly leaving (not entering a child element)
    const relatedTarget = e.relatedTarget as HTMLElement;
    const currentTarget = e.currentTarget as HTMLElement;

    // Check if we're leaving to an element outside the current target
    if (!relatedTarget || !currentTarget.contains(relatedTarget)) {
      setDropTarget(null);
      setIsDraggingOver(false);
    }
  };

  // Drop files onto target
  const handleDrop = async (
    e: React.DragEvent,
    targetPath: string = currentPath,
  ) => {
    e.preventDefault();
    e.stopPropagation();

    console.log('[handleDrop] Drop event triggered on:', targetPath);
    console.log('[handleDrop] Drag state:', {
      draggedFiles,
      draggedFileObjects: draggedFileObjects.length,
      dragSourceId,
    });

    setDropTarget(null);
    setIsDraggingOver(false);

    if (!selectedSource) {
      console.warn('[handleDrop] No selected source');
      setDraggedFiles([]);
      setDraggedFileObjects([]);
      setDragSourceId(null);
      return;
    }

    // Try to get drag data from multiple sources
    let paths: string[] = [];
    let sourceId: string | null = null;

    // First, try to get from state (most reliable)
    if (draggedFiles.length > 0) {
      paths = draggedFiles;
      sourceId = dragSourceId || selectedSource.id;
      console.log('[handleDrop] Using draggedFiles state:', {
        paths,
        sourceId,
      });
    } else {
      // Fallback: try to get from dataTransfer
      const vfsData = e.dataTransfer.getData('application/x-vfs-files');
      if (vfsData) {
        try {
          const parsed = JSON.parse(vfsData) as {
            sourceId: string;
            paths: string[];
          };
          paths = parsed.paths;
          sourceId = parsed.sourceId || selectedSource.id;
          console.log('[handleDrop] Using dataTransfer data:', {
            paths,
            sourceId,
          });
        } catch (err) {
          console.error('[handleDrop] Failed to parse vfsData:', err);
        }
      }
    }

    // If still no paths, try text/plain fallback
    if (paths.length === 0) {
      const textData = e.dataTransfer.getData('text/plain');
      if (textData) {
        paths = textData.split('\n').filter((p) => p.trim().length > 0);
        sourceId = dragSourceId || selectedSource.id;
        console.log('[handleDrop] Using text/plain fallback:', {
          paths,
          sourceId,
        });
      }
    }

    if (paths.length === 0 || !sourceId) {
      console.warn('[handleDrop] No valid paths or sourceId found');
      setDraggedFiles([]);
      setDraggedFileObjects([]);
      setDragSourceId(null);
      return;
    }

    // Process the drop operation
    try {
      if (paths.length > 0 && sourceId) {
        // Check if dropping into a folder (targetPath corresponds to a folder)
        // When dropping onto a folder item, targetPath will be the folder's path
        const targetFile = files.find((f) => f.path === targetPath);
        const isDroppingIntoFolder =
          targetFile &&
          (targetFile.isDirectory || targetFile.mimeType === 'folder');

        // Also check if targetPath is different from currentPath (indicating drop onto a specific folder)
        // This handles cases where the folder might not be in the current view's files array
        const isDroppingOntoSpecificFolder = targetPath !== currentPath;

        // Check if destination storage supports filesystem operations (mounted storage)
        const destSupportsFilesystemOps =
          supportsFilesystemOperations(selectedSource);

        // Determine operation type (like native file systems):
        // - Dropping into folder on mounted storage = MOVE by default
        // - Same source = MOVE by default
        // - Different source = COPY by default
        // - Shift key = Force MOVE
        // - Ctrl/Cmd key = Force COPY
        const isSameSource = sourceId === selectedSource.id;
        const forceMove = e.shiftKey;
        const forceCopy = e.ctrlKey || e.metaKey; // Ctrl on Windows/Linux, Cmd on macOS

        let isMove: boolean;
        if (forceCopy) {
          isMove = false; // Force copy
        } else if (forceMove) {
          isMove = true; // Force move
        } else if (
          (isDroppingIntoFolder || isDroppingOntoSpecificFolder) &&
          destSupportsFilesystemOps
        ) {
          // Dropping into folder on mounted storage defaults to MOVE
          // This ensures drag-and-drop onto folders always triggers move operations
          isMove = true;
        } else {
          isMove = isSameSource; // Default: move if same source, copy if different
        }

        console.log('[handleDrop] Operation type:', {
          isMove,
          isSameSource,
          isDroppingIntoFolder,
          isDroppingOntoSpecificFolder,
          destSupportsFilesystemOps,
          forceMove,
          forceCopy,
          sourceId,
          destSourceId: selectedSource.id,
          targetPath,
          currentPath,
          targetFile: targetFile?.name,
        });

        // Filter out invalid paths (dropping on self or parent)
        const validPaths = paths.filter((path) => {
          // Don't allow dropping on self
          if (path === targetPath) {
            return false;
          }
          // Don't allow dropping a folder into itself
          if (targetPath.startsWith(path + '/')) {
            return false;
          }
          return true;
        });

        if (validPaths.length === 0) {
          console.warn('[handleDrop] No valid paths after filtering');
          setDraggedFiles([]);
          setDraggedFileObjects([]);
          setDragSourceId(null);
          return;
        }

        console.log('[handleDrop] Processing', validPaths.length, 'files');

        if (sourceId === selectedSource.id) {
          // Same storage: move or copy within storage
          // Only allow move operations on filesystem-capable storage
          if (isMove && !supportsFilesystemOperations(selectedSource)) {
            DialogService.error(
              'Move not supported',
              'Move is only available for file system storage (local, network, hybrid). Use Copy instead.',
            );
            setDraggedFiles([]);
            setDraggedFileObjects([]);
            setDragSourceId(null);
            return;
          }

          // For object storage, only allow single file operations (no folders)
          const isObjStorage = isObjectStorage(selectedSource);

          // Normalize target path: ensure it starts with / and doesn't end with /
          let normalizedTarget = targetPath.trim();
          if (!normalizedTarget || normalizedTarget === '') {
            normalizedTarget = '/';
          } else if (!normalizedTarget.startsWith('/')) {
            normalizedTarget = '/' + normalizedTarget;
          }

          // Prepare moves array for bulk operation
          const moves: Array<{ from_path: string; to_path: string }> = [];

          for (const path of validPaths) {
            // Check if it's a folder and object storage - disable folder operations
            const file = files.find((f) => f.path === path);
            if (
              isObjStorage &&
              file &&
              (file.isDirectory || file.mimeType === 'folder')
            ) {
              DialogService.error(
                'Folder operations not supported',
                'Object storage does not support moving or copying folders. Please select individual files.',
              );
              continue;
            }

            const fileName =
              path.split('/').pop() || path.split('\\').pop() || '';
            if (!fileName) {
              console.warn(
                '[handleDrop] Skipping path with no filename:',
                path,
              );
              continue;
            }

            // Build destination path
            const destPath =
              normalizedTarget === '/'
                ? `/${fileName}`
                : `${normalizedTarget}/${fileName}`;

            moves.push({ from_path: path, to_path: destPath });
          }

          if (moves.length === 0) {
            return;
          }

          try {
            if (isMove) {
              // Use vfs_move for single file, vfs_batch_move for multiple files
              if (moves.length === 1) {
                // Single file move - use vfs_move for better tracking
                const moveItem = moves[0];
                const operationId = await invoke<string>('vfs_move', {
                  sourceId: sourceId,
                  fromPath: moveItem.from_path,
                  toPath: moveItem.to_path,
                });

                // Trigger move-started event for OperationsPanel
                setTimeout(() => {
                  window.dispatchEvent(
                    new CustomEvent('move-started', {
                      detail: { operationId },
                    }),
                  );
                }, 100);

                console.log(
                  '[handleDrop] Successfully started move:',
                  moveItem.from_path,
                  '->',
                  moveItem.to_path,
                );
              } else {
                // Multiple files - use bulk move (single operation with progress tracking)
                const operationId = await invoke<string>('vfs_batch_move', {
                  sourceId: sourceId,
                  moves: moves,
                });

                // Trigger move-started event for OperationsPanel
                setTimeout(() => {
                  window.dispatchEvent(
                    new CustomEvent('move-started', {
                      detail: { operationId, moves },
                    }),
                  );
                }, 100);

                console.log(
                  '[handleDrop] Successfully started bulk move:',
                  moves.length,
                  'file(s)',
                );
              }
            } else {
              // Copy operations - handle individually for now (can be bulkified later)
              for (const moveItem of moves) {
                const fileName =
                  moveItem.from_path.split('/').pop() ||
                  moveItem.from_path.split('\\').pop() ||
                  '';
                try {
                  const operationId = await invoke<string>('vfs_copy', {
                    sourceId: sourceId,
                    fromPath: moveItem.from_path,
                    toPath: moveItem.to_path,
                  });
                  // Trigger copy-started event for OperationsPanel
                  if (operationId) {
                    setTimeout(() => {
                      window.dispatchEvent(
                        new CustomEvent('copy-started', {
                          detail: { operationId },
                        }),
                      );
                    }, 100);
                  }
                  console.log(
                    '[handleDrop] Successfully copied:',
                    moveItem.from_path,
                    '->',
                    moveItem.to_path,
                  );
                } catch (copyErr) {
                  console.error('[handleDrop] Failed to copy file:', copyErr);
                  const errorMsg =
                    copyErr instanceof Error
                      ? copyErr.message
                      : String(copyErr);
                  DialogService.error(
                    `Failed to copy "${fileName}": ${errorMsg}`,
                    'File Operation Error',
                  );
                }
              }
            }
          } catch (moveErr) {
            console.error('[handleDrop] Failed to move files:', moveErr);
            const errorMsg =
              moveErr instanceof Error ? moveErr.message : String(moveErr);
            DialogService.error(
              `Failed to ${isMove ? 'move' : 'copy'} files: ${errorMsg}`,
              'File Operation Error',
            );
          }
          await loadFilesList(selectedSource.id, currentPath);
        } else {
          // Cross-storage drag: Show StorageTierDialog for move operations
          // For object storage, only allow single file operations (no folders)
          const isObjStorage = isObjectStorage(selectedSource);
          const foldersInPaths = validPaths.filter((path) => {
            const file = files.find((f) => f.path === path);
            return file && (file.isDirectory || file.mimeType === 'folder');
          });

          if (isObjStorage && foldersInPaths.length > 0) {
            DialogService.error(
              'Folder operations not supported',
              'Object storage does not support moving or copying folders. Please select individual files.',
            );
            setDraggedFiles([]);
            setDraggedFileObjects([]);
            setDragSourceId(null);
            return;
          }

          if (isMove) {
            // Show StorageTierDialog for cross-storage moves
            setCrossStorageDrag({
              sourceId,
              destSourceId: selectedSource.id,
              paths: validPaths,
              isMove: true,
              destPath: targetPath,
            });
            setTierDialogPaths(validPaths);
            setShowTierDialog(true);
            // Don't clear drag state yet - will be cleared after dialog confirms
            return;
          } else {
            // Copy across storages - proceed directly
            // For object storage, only allow single file operations (no folders)
            const isObjStorage = isObjectStorage(selectedSource);

            for (const path of validPaths) {
              // Check if it's a folder and object storage - disable folder operations
              const file = files.find((f) => f.path === path);
              if (
                isObjStorage &&
                file &&
                (file.isDirectory || file.mimeType === 'folder')
              ) {
                DialogService.error(
                  'Folder operations not supported',
                  'Object storage does not support copying folders. Please select individual files.',
                );
                continue;
              }

              const fileName = path.split('/').pop() || '';
              const normalizedTarget = targetPath === '' ? '/' : targetPath;
              const destPath =
                normalizedTarget === '/'
                  ? `/${fileName}`
                  : `${normalizedTarget}/${fileName}`;

              await invoke('vfs_copy_to_source', {
                src_source_id: sourceId,
                from_path: path,
                dest_source_id: selectedSource.id,
                to_path: destPath,
              });
            }
            await loadFilesList(selectedSource.id, currentPath);
          }
        }
      } else if (e.dataTransfer.files.length > 0) {
        try {
          for (const file of Array.from(e.dataTransfer.files)) {
            const filePath = (file as unknown as { path?: string }).path;
            if (filePath) {
              const fileName = filePath.split('/').pop() || file.name;
              const normalizedTarget = targetPath === '' ? '/' : targetPath;
              const destPath =
                normalizedTarget === '/'
                  ? `/${fileName}`
                  : `${normalizedTarget}/${fileName}`;

              await invoke('vfs_copy_to_source', {
                src_source_id: 'native',
                from_path: filePath,
                dest_source_id: selectedSource.id,
                to_path: destPath,
              });
            }
          }

          await loadFilesList(selectedSource.id, currentPath);
        } catch (err) {
          DialogService.error(`Import failed: ${err}`, 'Import Error');
        }
      }
    } catch (err) {
      console.error('[handleDrop] Drop failed:', err);
      DialogService.error(`Drop failed: ${err}`, 'Drop Error');
    }

    setDraggedFiles([]);
    setDraggedFileObjects([]);
    setDragSourceId(null);
  };

  // Drag ends (cleanup)
  const handleDragEnd = () => {
    setDraggedFiles([]);
    setDraggedFileObjects([]);
    setDragSourceId(null);
    setDropTarget(null);
    setIsDraggingOver(false);
  };

  // Drop onto sidebar source (cross-storage transfer)
  const handleDropOnSource = async (
    e: React.DragEvent,
    targetSource: StorageSource,
  ) => {
    e.preventDefault();
    e.stopPropagation();

    const vfsData = e.dataTransfer.getData('application/x-vfs-files');
    if (!vfsData) return;

    try {
      const { sourceId, paths } = JSON.parse(vfsData) as {
        sourceId: string;
        paths: string[];
      };
      const isMove = e.shiftKey || (!e.ctrlKey && !e.metaKey); // Default to move if no modifier

      // Check if move is supported - both source and destination must support filesystem operations
      if (isMove) {
        // Find source storage
        const sourceStorage = sources.find((s) => s.id === sourceId);
        if (!sourceStorage || !supportsFilesystemOperations(sourceStorage)) {
          DialogService.error(
            'Move not supported',
            'Move is only available for file system storage (local, network, hybrid). Use Copy instead.',
          );
          return;
        }
        if (!supportsFilesystemOperations(targetSource)) {
          DialogService.error(
            'Move not supported',
            'Move destination must be file system storage (local, network, hybrid). Use Copy instead.',
          );
          return;
        }
      }

      // If moving across storages, show StorageTierDialog
      if (isMove && sourceId !== targetSource.id) {
        setCrossStorageDrag({
          sourceId,
          destSourceId: targetSource.id,
          paths,
          isMove: true,
          destPath: '/', // Root of target source
        });
        setTierDialogPaths(paths);
        setShowTierDialog(true);
        // Don't clear drag state yet - will be cleared after dialog confirms
        return;
      }

      // Copy operation or same source - proceed directly
      for (const path of paths) {
        const fileName = path.split('/').pop() || '';
        const destPath = `/${fileName}`;

        if (isMove) {
          await invoke('vfs_move_to_source', {
            src_source_id: sourceId,
            from_path: path,
            dest_source_id: targetSource.id,
            to_path: destPath,
          });
        } else {
          await invoke('vfs_copy_to_source', {
            src_source_id: sourceId,
            from_path: path,
            dest_source_id: targetSource.id,
            to_path: destPath,
          });
        }
      }

      // Optionally switch to target source
      // setSelectedSource(targetSource);
      // await loadFilesList(targetSource.id, '/');
    } catch (err) {
      console.error('Cross-storage drop failed:', err);
      DialogService.error(`Transfer failed: ${err}`, 'Transfer Error');
    }

    setDraggedFiles([]);
    setDraggedFileObjects([]);
    setDragSourceId(null);
    setDropTarget(null);
    setIsDraggingOver(false);
  };

  // Toggle favorite for a file
  const handleToggleFavorite = async (filePath: string) => {
    if (!selectedSource) return;

    const file = files.find((f) => f.path === filePath);
    if (!file) return;

    const favoriteId = `${selectedSource.id}:${filePath}`;
    const existingIndex = favorites.findIndex((f) => f.id === favoriteId);

    if (existingIndex >= 0) {
      // Remove from favorites
      removeFromGlobalFavorites(favoriteId);
    } else {
      // Add to favorites
      addToGlobalFavorites(file, selectedSource);
    }
  };

  // Check if a file is in favorites
  const isFileFavorite = (filePath: string): boolean => {
    if (!selectedSource) return false;
    const favoriteId = `${selectedSource.id}:${filePath}`;
    return favorites.some((f) => f.id === favoriteId);
  };

  // Navigate to a favorite
  const navigateToFavorite = useCallback(
    async (favorite: GlobalFavorite) => {
      // First, find and select the source - use stable reference from sources array
      const source = sources.find((s) => s.id === favorite.sourceId);
      if (source) {
        // Use source from sources array to ensure stable reference
        setSelectedSource(source);
      }

      // Get directory of the favorite
      const parts = favorite.path.split('/');
      if (!favorite.isDirectory) {
        parts.pop(); // Remove filename
      }
      const dirPath = parts.join('/') || '/';
      setCurrentPath(dirPath);

      // Select the file after navigation
      if (!favorite.isDirectory) {
        setTimeout(() => {
          setSelectedFiles(new Set([favorite.path]));
        }, 100);
      }
    },
    [sources],
  );

  // Handle adding a new storage source
  const handleAddStorage = async (sourceConfig: Partial<StorageSource>) => {
    try {
      if (editingSource) {
        // Update existing source
        // First remove the old source
        await invoke('vfs_remove_source', { sourceId: editingSource.id });

        // Then add the updated source
        const updatedSource = (await invoke('vfs_add_source', {
          source: sourceConfig,
        })) as StorageSource;

        // Update in sources list
        setSources((prev) =>
          prev.map((s) => (s.id === editingSource.id ? updatedSource : s)),
        );

        // Update selection if this was the selected source
        if (selectedSource?.id === editingSource.id) {
          setSelectedSource(updatedSource);
        }

        toast.showToast({
          type: 'success',
          message: `Updated ${updatedSource.name}`,
        });
      } else {
        // For S3 sources, use the register command to actually connect
        if (
          sourceConfig.providerId === 'aws-s3' ||
          sourceConfig.providerId === 's3'
        ) {
          const config = sourceConfig.config || {};
          const bucket = (config.bucket as string) || '';
          const region = (config.region as string) || '';
          const accessKeyId = (config.accessKeyId as string) || undefined;
          const secretAccessKey =
            (config.secretAccessKey as string) || undefined;
          const sessionToken = (config.sessionToken as string) || undefined;
          const endpoint = (config.endpoint as string) || undefined;

          if (!bucket || !region) {
            throw new Error('Bucket and region are required for S3 storage');
          }

          // Register S3 source with backend
          const newSource = (await invoke('vfs_register_s3_source', {
            name: sourceConfig.name || bucket,
            bucket,
            region,
            accessKey: accessKeyId,
            secretKey: secretAccessKey,
            sessionToken,
            endpoint,
          })) as StorageSource;

          // Add to sources list
          setSources((prev) => {
            const updated = [...prev, newSource];
            savePersistedSources(updated);
            return updated;
          });

          // Optionally select the new source
          setSelectedSource(newSource);
          setCurrentPath('/');

          toast.showToast({
            type: 'success',
            message: `Added ${newSource.name}`,
          });
        } else {
          // Add new source (non-S3)
          const newSource = (await invoke('vfs_add_source', {
            source: sourceConfig,
          })) as StorageSource;

          // Add to sources list
          setSources((prev) => {
            const updated = [...prev, newSource];
            savePersistedSources(updated);
            return updated;
          });

          // Optionally select the new source
          setSelectedSource(newSource);
          setCurrentPath('/');

          toast.showToast({
            type: 'success',
            message: `Added ${newSource.name}`,
          });
        }
      }

      setEditingSource(null);
    } catch (err) {
      console.error('Failed to add/update storage:', err);
      DialogService.error(
        `Failed to ${editingSource ? 'update' : 'add'} storage: ${err}`,
        'Storage Error',
      );
    }
  };

  // Handle editing a storage source
  const handleEditStorage = (source: StorageSource) => {
    setEditingSource(source);
    setShowAddStorage(true);
    setStorageContextMenu(null);
  };

  // Handle removing a storage source
  const handleRemoveStorage = async (sourceId: string) => {
    try {
      const confirmed = await DialogService.confirm({
        title: 'Remove Storage',
        message: `Are you sure you want to remove this storage source? This will not delete any files, only remove it from the list.`,
        okLabel: 'Remove',
        cancelLabel: 'Cancel',
        type: DialogType.Warning,
      });

      if (!confirmed) return;

      await invoke('vfs_remove_source', { sourceId });

      // Remove from sources list
      setSources((prev) => {
        const updated = prev.filter((s) => s.id !== sourceId);
        savePersistedSources(updated);
        return updated;
      });

      // If the removed source was selected, clear selection
      if (selectedSource?.id === sourceId) {
        setSelectedSource(null);
        setCurrentPath('');
        setFiles([]);
      }

      toast.showToast({
        type: 'success',
        message: 'Storage source removed',
      });
    } catch (err) {
      console.error('Failed to remove storage:', err);
      const errorMessage = err instanceof Error ? err.message : String(err);
      DialogService.error(
        `Failed to remove storage source: ${errorMessage}`,
        'Storage Error',
      );
    }
  };

  const checkObjectStorage = () => {
    if (!selectedSource) {
      DialogService.error(
        'Please select a storage source first',
        'Upload Error',
      );
      return false;
    }

    // Use the helper function to check if it's object storage
    if (!isObjectStorage(selectedSource)) {
      DialogService.error(
        'Upload is only available for object storage (S3, GCS, Azure Blob)',
        'Upload Error',
      );
      return false;
    }

    return true;
  };

  const handleUpload = async () => {
    if (!checkObjectStorage()) return;

    try {
      const s3BasePath =
        currentPath === '/' ? '' : currentPath.replace(/^\//, '');

      // Unified upload dialog: Allow selecting both files and folders
      // Note: Tauri's dialog doesn't support selecting both files and folders in one dialog
      // So we'll use the file dialog and check if selected items are directories
      // Users can select folders by navigating into them or selecting them directly
      // On macOS, folders can be selected in file dialogs
      // Use DialogService for consistency
      const fileResult = await DialogService.open({
        multiple: true,
        directory: false, // File dialog (folders can still be selected on macOS)
        title: `Select files and/or folders to upload to ${selectedSource?.name || 'storage'}`,
      });

      if (!fileResult) {
        return; // User canceled
      }

      const selectedPaths = Array.isArray(fileResult)
        ? fileResult
        : [fileResult];

      // Separate files and folders by checking each path
      const folders: string[] = [];
      const files: string[] = [];

      for (const path of selectedPaths) {
        try {
          const isDir = await invoke('vfs_is_directory', {
            path: path,
          });
          if (isDir) {
            folders.push(path);
          } else {
            files.push(path);
          }
        } catch {
          // If check fails, assume it's a file
          files.push(path);
        }
      }

      // Show feedback about what was selected
      if (folders.length === 0 && files.length === 0) {
        return; // Nothing selected
      }

      // Show initial feedback about selection
      if (folders.length > 0 && files.length > 0) {
        toast.showToast({
          type: 'info',
          message: `Processing ${folders.length} folder(s) and ${files.length} file(s)...`,
        });
      } else if (folders.length > 0) {
        toast.showToast({
          type: 'info',
          message: `Processing ${folders.length} folder(s)...`,
        });
      }

      // Group all items (files + folders) into a single batch operation
      const batchItems: Array<{ type: string; path: string }> = [];

      // Add folders
      folders.forEach((folderPath) => {
        batchItems.push({ type: 'folder', path: folderPath });
      });

      // Add files
      files.forEach((filePath) => {
        batchItems.push({ type: 'file', path: filePath });
      });

      if (batchItems.length === 0) {
        return; // Nothing to upload
      }

      try {
        // Use batch upload to create a single operation for all items
        const operationId = await invoke<string>('vfs_batch_upload', {
          sourceId: selectedSource?.id || '',
          items: batchItems,
          s3BasePath: s3BasePath,
          partSize: null,
        });

        // Don't auto-switch - OperationsPanel modal will show progress automatically
        // Give uploads a moment to start, then trigger a check
        // The OperationsPanel polls every 1000ms, but we want immediate visibility
        setTimeout(() => {
          // Trigger a custom event that OperationsPanel can listen to
          window.dispatchEvent(
            new CustomEvent('upload-started', { detail: { operationId } }),
          );
        }, 100);

        // Show success message
        const summary =
          folders.length > 0 && files.length > 0
            ? `Started uploading ${folders.length} folder(s) and ${files.length} file(s) as one operation`
            : folders.length > 0
              ? `Started uploading ${folders.length} folder(s) as one operation`
              : `Started uploading ${files.length} file(s) as one operation`;

        console.log(
          '[FinderPage] Batch upload operation created:',
          operationId,
        );
        toast.showToast({
          type: 'success',
          message: summary,
        });

        // Set up a listener to refresh file list when upload completes
        // Poll for completion status (OperationsPanel also polls, but we need to refresh file list)
        const checkUploadComplete = setInterval(async () => {
          try {
            const operations = await invoke<
              Array<{
                operation_id: string;
                status: string;
              }>
            >('vfs_list_operations', {
              operationTypes: ['Upload'],
              limit: 100,
            });
            const uploadOp = operations.find(
              (op) => op.operation_id === operationId,
            );
            if (uploadOp && uploadOp.status === 'Completed') {
              clearInterval(checkUploadComplete);
              // Dispatch event for other components
              window.dispatchEvent(
                new CustomEvent('upload-completed', {
                  detail: { operationId },
                }),
              );
              // Refresh file list after a short delay to ensure backend has updated
              setTimeout(() => {
                if (selectedSource) {
                  loadFilesList(selectedSource.id, currentPath).catch((err) => {
                    console.error(
                      '[FinderPage] Error refreshing files after upload:',
                      err,
                    );
                  });
                }
              }, 500);
            } else if (
              uploadOp &&
              (uploadOp.status === 'Failed' || uploadOp.status === 'Canceled')
            ) {
              clearInterval(checkUploadComplete);
            }
          } catch (err) {
            console.error('[FinderPage] Error checking upload status:', err);
          }
        }, 1000);

        // Clear interval after 5 minutes to prevent memory leaks
        setTimeout(() => {
          clearInterval(checkUploadComplete);
        }, 300000);
      } catch (err) {
        console.error('[FinderPage] Failed to batch upload:', err);
        const errorMessage = err instanceof Error ? err.message : String(err);
        DialogService.error(`Upload failed: ${errorMessage}`, 'Upload Error');
      }

      // Upload started successfully (no errors variable needed - batch upload handles errors internally)
    } catch (err) {
      console.error('[FinderPage] Failed to open upload dialog:', err);
      DialogService.error(
        `Failed to open upload dialog: ${err}`,
        'Upload Error',
      );
    }
  };

  // Context menu handlers
  const handleContextMenu = async (
    e: React.MouseEvent,
    file?: FileMetadata,
  ) => {
    e.preventDefault();
    e.stopPropagation(); // Prevent bubbling to parent containers

    console.log('[Context Menu] Right-click detected', {
      file: file?.name,
      path: file?.path,
      clientX: e.clientX,
      clientY: e.clientY,
    });

    // If right-clicking on a file that's not selected, select it FIRST
    // This ensures Copy/Cut buttons appear in the context menu
    if (file && !selectedFiles.has(file.path)) {
      setSelectedFiles(new Set([file.path]));
    }

    // Check clipboard (both VFS and native) - don't await, do it in parallel
    Promise.all([
      invoke('vfs_clipboard_has_files').catch(() => false),
      invoke('vfs_clipboard_read_native').catch(() => []),
    ])
      .then(([hasVfsFiles, nativeFiles]) => {
        setNativeClipboardCount((nativeFiles as string[]).length);
        setClipboardHasFiles(
          Boolean(hasVfsFiles || (nativeFiles as string[]).length > 0),
        );
      })
      .catch((err) => {
        console.error('Failed to check clipboard:', err);
        setClipboardHasFiles(false);
        setNativeClipboardCount(0);
      });

    // Set context menu position - ensure it's visible on screen
    const menuX = Math.min(e.clientX, window.innerWidth - 200); // Leave space for menu width
    const menuY = Math.min(e.clientY, window.innerHeight - 300); // Leave space for menu height

    // Set context menu immediately - use requestAnimationFrame to ensure DOM is ready
    requestAnimationFrame(() => {
      console.log('[Context Menu] Setting menu visible', {
        menuX,
        menuY,
        file: file?.name,
      });
      setContextMenu({
        visible: true,
        x: menuX,
        y: menuY,
        targetFile: file,
      });
    });
  };

  const closeContextMenu = () => {
    setContextMenu({ visible: false, x: 0, y: 0, targetFile: undefined });
  };

  // Handler for downloading files (object storage)
  const handleDownloadFile = async (file: FileMetadata) => {
    if (!selectedSource) return;

    try {
      const { save } = await import('@tauri-apps/plugin-dialog');

      const fileName = file.name;
      const savePath = await save({
        defaultPath: fileName,
        filters: [
          {
            name: 'All Files',
            extensions: ['*'],
          },
        ],
      });

      if (!savePath) {
        return; // User cancelled
      }

      // Start download - OperationsPanel will show progress
      const operationId = await invoke<string>('vfs_download_file', {
        sourceId: selectedSource.id,
        path: file.path,
        destPath: savePath,
      });

      // Trigger download-started event for OperationsPanel
      if (operationId) {
        setTimeout(() => {
          window.dispatchEvent(
            new CustomEvent('download-started', {
              detail: { operationId },
            }),
          );
        }, 100);
      }

      // Don't show toast - OperationsPanel handles UI feedback
    } catch (err) {
      console.error('Download failed:', err);
      DialogService.error(`Download failed: ${err}`, 'Download Error');
    }
  };

  // Handler for syncing files to storage tier
  const handleSyncToTier = async (tier: string, targetSourceId?: string) => {
    if (!selectedSource) return;

    try {
      // If this is a cross-storage drag operation, handle it differently
      if (crossStorageDrag) {
        const { sourceId, destSourceId, paths, isMove, destPath } =
          crossStorageDrag;

        // Check if move is supported - both source and destination must support filesystem operations
        if (isMove) {
          const sourceStorage = sources.find((s) => s.id === sourceId);
          const destStorage = sources.find((s) => s.id === destSourceId);
          if (!sourceStorage || !supportsFilesystemOperations(sourceStorage)) {
            DialogService.error(
              'Move not supported',
              'Move source must be file system storage (local, network, hybrid). Use Copy instead.',
            );
            setCrossStorageDrag(null);
            return;
          }
          if (!destStorage || !supportsFilesystemOperations(destStorage)) {
            DialogService.error(
              'Move not supported',
              'Move destination must be file system storage (local, network, hybrid). Use Copy instead.',
            );
            setCrossStorageDrag(null);
            return;
          }
        }

        // Perform cross-storage move/copy
        // Construct destination paths for each file
        const normalizedTarget =
          destPath === '' || destPath === '/' || !destPath ? '/' : destPath;

        for (const path of paths) {
          const fileName = path.split('/').pop() || '';
          const finalDestPath =
            normalizedTarget === '/'
              ? `/${fileName}`
              : `${normalizedTarget}/${fileName}`;

          if (isMove) {
            await invoke('vfs_move_to_source', {
              src_source_id: sourceId,
              from_path: path,
              dest_source_id: destSourceId,
              to_path: finalDestPath,
            });
          } else {
            await invoke('vfs_copy_to_source', {
              src_source_id: sourceId,
              from_path: path,
              dest_source_id: destSourceId,
              to_path: finalDestPath,
            });
          }
        }

        // Clear cross-storage drag state
        setCrossStorageDrag(null);
        setDraggedFiles([]);
        setDraggedFileObjects([]);
        setDragSourceId(null);
        setDropTarget(null);
        setIsDraggingOver(false);

        // Refresh file lists
        if (selectedSource) {
          if (sourceId === selectedSource.id) {
            await loadFilesList(selectedSource.id, currentPath);
          }
          if (destSourceId === selectedSource.id) {
            await loadFilesList(selectedSource.id, normalizedTarget || '/');
          }
        }

        toast.showToast({
          type: 'success',
          message: `Successfully ${isMove ? 'moved' : 'copied'} ${paths.length} file${paths.length !== 1 ? 's' : ''} to ${destSourceId}`,
          duration: 3000,
        });
      } else {
        // Regular tier sync operation
        if (!selectedSource) {
          throw new Error('No storage source selected');
        }
        const result = (await invoke('vfs_sync_to_tier', {
          sourceId: selectedSource.id,
          paths: tierDialogPaths,
          targetTier: tier,
          targetSourceId: targetSourceId || null,
        })) as {
          files_synced: number;
          files_failed: number;
          errors: string[];
          operation_ids?: string[]; // Operation IDs for tracking
        };

        // Dispatch events for OperationsPanel and TransferPanel to show tier sync operations
        // Operations are already tracked in backend, but we need to trigger panel refresh
        if (result.operation_ids && result.operation_ids.length > 0) {
          result.operation_ids.forEach((operationId) => {
            setTimeout(() => {
              // Dispatch both copy-started (for OperationsPanel) and tier-change-started (for TransferPanel)
              window.dispatchEvent(
                new CustomEvent('copy-started', {
                  detail: { operationId },
                }),
              );
              window.dispatchEvent(
                new CustomEvent('tier-change-started', {
                  detail: { operationId },
                }),
              );
            }, 100);
          });
        } else {
          // Fallback: trigger a general refresh
          setTimeout(() => {
            window.dispatchEvent(
              new CustomEvent('upload-started', { detail: {} }),
            );
          }, 100);
        }

        if (result.files_failed === 0) {
          toast.showToast({
            type: 'success',
            message: `Successfully moved ${result.files_synced} file${result.files_synced !== 1 ? 's' : ''} to ${tier} tier`,
            duration: 3000,
          });
        } else {
          DialogService.error(
            `Moved ${result.files_synced} file(s), but ${result.files_failed} failed. ${result.errors.join(', ')}`,
            'Tier Sync Warning',
          );
        }

        // Refresh the file list
        await loadFilesList(selectedSource.id, currentPath);
      }

      setShowTierDialog(false);
    } catch (err) {
      console.error('Tier sync failed:', err);
      DialogService.error(`Failed to sync to tier: ${err}`, 'Tier Sync Error');
      setShowTierDialog(false);
      setCrossStorageDrag(null);
    }
  };

  // Close context menus on click anywhere (but not on right-click)
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      // Don't close context menu on right-click (button 2) or middle-click (button 1)
      // Only handle left-click (button 0) or no button info
      if (e.button === 2 || e.button === 1) {
        return;
      }

      // Longer delay to allow context menu button clicks to register and menu to render
      setTimeout(() => {
        // Check if click is inside context menu - don't close if clicking menu items
        const contextMenuElement = document.querySelector('.context-menu');
        if (
          contextMenuElement &&
          contextMenuElement.contains(e.target as Node)
        ) {
          console.log('[Context Menu] Click inside menu, keeping open');
          return;
        }

        // Close context menus on left-click outside
        console.log('[Context Menu] Click outside menu, closing');
        closeContextMenu();
        setStorageContextMenu(null);
      }, 100); // Increased delay to ensure menu is rendered
    };

    // Also handle contextmenu events to prevent default browser menu
    const handleContextMenuEvent = (e: MouseEvent) => {
      // Only prevent default if we're handling it (on file items or empty area)
      const target = e.target as HTMLElement;
      const isFileItem = target.closest('.file-item, .list-row');
      const isContentArea = target.closest(
        '.finder-content, .icon-view, .list-body',
      );

      if (isFileItem || isContentArea) {
        // We're handling this, prevent default browser menu
        e.preventDefault();
      }
    };

    window.addEventListener('click', handleClick, true); // Use capture phase
    window.addEventListener('contextmenu', handleContextMenuEvent, true);
    return () => {
      window.removeEventListener('click', handleClick, true);
      window.removeEventListener('contextmenu', handleContextMenuEvent, true);
    };
  }, []);

  /**
   * Build breadcrumbs that work across different storage types:
   * - Local: /Users/tony/Documents -> [Home, Documents]
   * - S3: bucket/prefix/key -> [bucket, prefix, key]
   * - Network (SMB/NFS): //server/share/folder or /Volumes/Share/folder
   */
  const getBreadcrumbs = (): BreadcrumbItem[] => {
    // SVG icon components using theme colors
    const LocalIcon = () => (
      <svg
        width="14"
        height="14"
        viewBox="0 0 16 16"
        fill="currentColor"
        className="breadcrumb-icon location"
      >
        <path d="M4.5 5a.5.5 0 1 0 0-1 .5.5 0 0 0 0 1zM3 4.5a.5.5 0 1 1-1 0 .5.5 0 0 1 1 0z" />
        <path d="M0 4a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v1a2 2 0 0 1-2 2H8.5v3a1.5 1.5 0 0 1 1.5 1.5h5.5a.5.5 0 0 1 0 1H10A1.5 1.5 0 0 1 8.5 14h-1A1.5 1.5 0 0 1 6 12.5H.5a.5.5 0 0 1 0-1H6A1.5 1.5 0 0 1 7.5 10V7H2a2 2 0 0 1-2-2V4zm1 0v1a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V4a1 1 0 0 0-1-1H2a1 1 0 0 0-1 1zm6 7.5v1a.5.5 0 0 0 .5.5h1a.5.5 0 0 0 .5-.5v-1a.5.5 0 0 0-.5-.5h-1a.5.5 0 0 0-.5.5z" />
      </svg>
    );
    const CloudIcon = () => (
      <svg
        width="14"
        height="14"
        viewBox="0 0 16 16"
        fill="currentColor"
        className="breadcrumb-icon location"
      >
        <path d="M4.406 3.342A5.53 5.53 0 0 1 8 2c2.69 0 4.923 2 5.166 4.579C14.758 6.804 16 8.137 16 9.773 16 11.569 14.502 13 12.687 13H3.781C1.708 13 0 11.366 0 9.318c0-1.763 1.266-3.223 2.942-3.593.143-.863.698-1.723 1.464-2.383z" />
      </svg>
    );
    const NetworkIcon = () => (
      <svg
        width="14"
        height="14"
        viewBox="0 0 16 16"
        fill="currentColor"
        className="breadcrumb-icon location"
      >
        <path d="M6.5 9a.5.5 0 0 0-.5.5v2a.5.5 0 0 0 .5.5h3a.5.5 0 0 0 .5-.5v-2a.5.5 0 0 0-.5-.5h-3zM5 8.5A1.5 1.5 0 0 1 6.5 7h3A1.5 1.5 0 0 1 11 8.5v2A1.5 1.5 0 0 1 9.5 12h-3A1.5 1.5 0 0 1 5 10.5v-2z" />
        <path d="M1.5 1a.5.5 0 0 0-.5.5v3a.5.5 0 0 1-1 0v-3A1.5 1.5 0 0 1 1.5 0h3a.5.5 0 0 1 0 1h-3zm11 0a.5.5 0 0 0 0-1h3A1.5 1.5 0 0 1 16 1.5v3a.5.5 0 0 1-1 0v-3a.5.5 0 0 0-.5-.5h-3zM.5 11a.5.5 0 0 1 .5.5v3a.5.5 0 0 0 .5.5h3a.5.5 0 0 1 0 1h-3A1.5 1.5 0 0 1 0 14.5v-3a.5.5 0 0 1 .5-.5zm15 0a.5.5 0 0 1 .5.5v3a1.5 1.5 0 0 1-1.5 1.5h-3a.5.5 0 0 1 0-1h3a.5.5 0 0 0 .5-.5v-3a.5.5 0 0 1 .5-.5z" />
        <path d="M3 6.5a.5.5 0 0 1 .5-.5h9a.5.5 0 0 1 0 1h-9a.5.5 0 0 1-.5-.5z" />
      </svg>
    );
    const HybridIcon = () => (
      <svg
        width="14"
        height="14"
        viewBox="0 0 16 16"
        fill="currentColor"
        className="breadcrumb-icon location"
      >
        <path d="M5 0a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V1a1 1 0 0 0-1-1H5zm.5 14a.5.5 0 1 1 0-1 .5.5 0 0 1 0 1zm2 0a.5.5 0 1 1 0-1 .5.5 0 0 1 0 1zM5 1.5a.5.5 0 0 1 .5-.5h5a.5.5 0 0 1 0 1h-5a.5.5 0 0 1-.5-.5zM5.5 3h5a.5.5 0 0 1 0 1h-5a.5.5 0 0 1 0-1zm0 2h5a.5.5 0 0 1 0 1h-5a.5.5 0 0 1 0-1zm0 2h5a.5.5 0 0 1 0 1h-5a.5.5 0 0 1 0-1zm0 2h5a.5.5 0 0 1 0 1h-5a.5.5 0 0 1 0-1z" />
      </svg>
    );
    const FolderIcon = () => (
      <svg
        width="14"
        height="14"
        viewBox="0 0 16 16"
        fill="currentColor"
        className="breadcrumb-icon folder"
      >
        <path d="M.54 3.87.5 3a2 2 0 0 1 2-2h3.672a2 2 0 0 1 1.414.586l.828.828A2 2 0 0 0 9.828 3H13.5a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H2.5a2 2 0 0 1-2-2V3.87z" />
      </svg>
    );

    if (!selectedSource)
      return [{ name: 'Root', path: '', icon: <LocalIcon /> }];

    const storageType =
      selectedSource.providerId || selectedSource.type || 'local';
    const sourceName = selectedSource.name || 'Storage';

    // Root breadcrumb with storage-specific icon
    const getStorageIcon = (): React.ReactNode => {
      switch (storageType) {
        case 'aws-s3':
        case 's3':
        case 's3-compatible':
        case 'gcs':
        case 'azure-blob':
          return <CloudIcon />;
        case 'smb':
        case 'nfs':
        case 'nas':
          return <NetworkIcon />;
        case 'fsx-ontap':
          return <HybridIcon />;
        case 'sftp':
        case 'webdav':
          return <NetworkIcon />;
        default:
          return <LocalIcon />;
      }
    };

    const crumbs: BreadcrumbItem[] = [
      {
        name: sourceName,
        path: '',
        icon: getStorageIcon(),
      },
    ];

    if (!currentPath || currentPath === '/' || currentPath === '') {
      return crumbs;
    }

    // Parse path based on storage type
    let pathParts: string[] = [];

    if (
      storageType === 'aws-s3' ||
      storageType === 's3' ||
      storageType === 'gcs' ||
      storageType === 'azure-blob'
    ) {
      // Object storage: bucket/prefix/key format (no leading slash)
      pathParts = currentPath.replace(/^\/+/, '').split('/').filter(Boolean);
    } else if (storageType === 'smb' || storageType === 'nfs') {
      // Network paths: handle //server/share or UNC paths
      const cleanPath = currentPath.replace(/^\/\//, '').replace(/^\\\\/, '');
      pathParts = cleanPath.split(/[/\\]/).filter(Boolean);
    } else {
      // Local/default: standard Unix path
      pathParts = currentPath.split('/').filter(Boolean);
    }

    // Build accumulated paths for navigation
    let accumulated = '';
    const pathSeparator =
      storageType === 'smb' && currentPath.startsWith('\\\\') ? '\\' : '/';

    for (const part of pathParts) {
      accumulated = accumulated
        ? `${accumulated}${pathSeparator}${part}`
        : `/${part}`;
      crumbs.push({
        name: part,
        path: accumulated,
        icon: <FolderIcon />,
      });
    }

    return crumbs;
  };

  // Check if current storage is mounted/local (files directly accessible)
  // Transcode and Download features only make sense for remote/cloud storage
  const isMountedStorage = (): boolean => {
    if (!selectedSource) return true; // Default to true if no source
    const category = selectedSource.category;
    // Local, network, hybrid, and block storage are considered mounted
    return (
      category === 'local' ||
      category === 'network' ||
      category === 'block' ||
      category === 'hybrid'
    );
  };

  // Check if storage is true object storage (S3, GCS, Azure Blob) that is NOT mounted
  const isObjectStorage = (source?: StorageSource | null): boolean => {
    const checkSource = source || selectedSource;
    if (!checkSource) return false;
    const category = checkSource.category;
    const providerId = checkSource.providerId || checkSource.type || '';

    // Only cloud category with specific object storage providers are object storage
    if (category !== 'cloud') return false;

    // Check for specific object storage providers
    const objectStorageProviders = [
      's3',
      'aws-s3',
      's3-compatible',
      'gcs',
      'azure-blob',
      'backblaze-b2',
      'digitalocean-spaces',
      'cloudflare-r2',
      'linode-object',
      'wasabi',
      'minio',
    ];

    return objectStorageProviders.includes(providerId.toLowerCase());
  };

  // Parse search query for DAM/MAM search operators
  // Supports: tag:, type:, tier:, ext:, is:, size:, modified:
  const parseSearchQuery = (
    query: string,
  ): {
    textSearch: string;
    tagFilters?: string[];
    typeFilter?: string;
    tierFilter?: string;
    extFilter?: string;
    isFilter?: string;
    sizeFilter?: string;
    modifiedFilter?: string;
  } => {
    let textSearch = query;
    const tagFilters: string[] = [];
    let typeFilter: string | undefined;
    let tierFilter: string | undefined;
    let extFilter: string | undefined;
    let isFilter: string | undefined;
    let sizeFilter: string | undefined;
    let modifiedFilter: string | undefined;

    // Extract all tag: operators (support multiple tags)
    const tagMatches = query.matchAll(/tag:(\S+)/gi);
    for (const match of tagMatches) {
      const tagValue = match[1].toLowerCase();
      if (tagValue && !tagFilters.includes(tagValue)) {
        tagFilters.push(tagValue);
      }
      textSearch = textSearch.replace(match[0], '').trim();
    }

    // Extract type: operator
    const typeMatch = query.match(/type:(\S+)/i);
    if (typeMatch) {
      typeFilter = typeMatch[1].toLowerCase();
      textSearch = textSearch.replace(typeMatch[0], '').trim();
    }

    // Extract tier: operator
    const tierMatch = query.match(/tier:(\S+)/i);
    if (tierMatch) {
      tierFilter = tierMatch[1].toLowerCase();
      textSearch = textSearch.replace(tierMatch[0], '').trim();
    }

    // Extract ext: operator
    const extMatch = query.match(/ext:(\S+)/i);
    if (extMatch) {
      extFilter = extMatch[1].toLowerCase().replace(/^\./, ''); // Remove leading dot
      textSearch = textSearch.replace(extMatch[0], '').trim();
    }

    // Extract is: operator
    const isMatch = query.match(/is:(\S+)/i);
    if (isMatch) {
      isFilter = isMatch[1].toLowerCase();
      textSearch = textSearch.replace(isMatch[0], '').trim();
    }

    // Extract size: operator
    const sizeMatch = query.match(/size:(\S+)/i);
    if (sizeMatch) {
      sizeFilter = sizeMatch[1].toLowerCase();
      textSearch = textSearch.replace(sizeMatch[0], '').trim();
    }

    // Extract modified: operator
    const modifiedMatch = query.match(/modified:(\S+)/i);
    if (modifiedMatch) {
      modifiedFilter = modifiedMatch[1].toLowerCase();
      textSearch = textSearch.replace(modifiedMatch[0], '').trim();
    }

    return {
      textSearch,
      tagFilters: tagFilters.length > 0 ? tagFilters : undefined,
      typeFilter,
      tierFilter,
      extFilter,
      isFilter,
      sizeFilter,
      modifiedFilter,
    };
  };

  // Helper to parse size filter (e.g., ">10mb", "<1gb")
  const matchesSizeFilter = (size: number, filter: string): boolean => {
    const match = filter.match(/^([<>]=?)(\d+(?:\.\d+)?)(kb|mb|gb|tb)?$/i);
    if (!match) return true;

    const [, op, numStr, unit = 'b'] = match;
    const num = parseFloat(numStr);
    const multipliers: Record<string, number> = {
      b: 1,
      kb: 1024,
      mb: 1024 * 1024,
      gb: 1024 * 1024 * 1024,
      tb: 1024 * 1024 * 1024 * 1024,
    };
    const threshold = num * (multipliers[unit.toLowerCase()] || 1);

    switch (op) {
      case '>':
        return size > threshold;
      case '>=':
        return size >= threshold;
      case '<':
        return size < threshold;
      case '<=':
        return size <= threshold;
      default:
        return size > threshold;
    }
  };

  // Helper to check modified date filter
  const matchesModifiedFilter = (
    modifiedDate: string | undefined,
    filter: string,
  ): boolean => {
    if (!modifiedDate) return false;

    const fileDate = new Date(modifiedDate);
    const now = new Date();
    const startOfToday = new Date(
      now.getFullYear(),
      now.getMonth(),
      now.getDate(),
    );
    const startOfYesterday = new Date(
      startOfToday.getTime() - 24 * 60 * 60 * 1000,
    );
    const startOfWeek = new Date(
      startOfToday.getTime() - 7 * 24 * 60 * 60 * 1000,
    );
    const startOfMonth = new Date(
      startOfToday.getTime() - 30 * 24 * 60 * 60 * 1000,
    );
    const startOfYear = new Date(now.getFullYear(), 0, 1);

    switch (filter) {
      case 'today':
        return fileDate >= startOfToday;
      case 'yesterday':
        return fileDate >= startOfYesterday && fileDate < startOfToday;
      case 'week':
        return fileDate >= startOfWeek;
      case 'month':
        return fileDate >= startOfMonth;
      case 'year':
        return fileDate >= startOfYear;
      default:
        return true;
    }
  };

  // Filter files based on search query, tags, column filters, and hidden files toggle
  const filteredFiles = files
    .filter((f) => {
      const {
        textSearch,
        tagFilters: searchTagFilters,
        typeFilter,
        tierFilter,
        extFilter,
        isFilter,
        sizeFilter,
        modifiedFilter,
      } = parseSearchQuery(searchQuery);

      // Filter by column: Name
      if (columnFilters.name) {
        if (!f.name.toLowerCase().includes(columnFilters.name.toLowerCase())) {
          return false;
        }
      }

      // Filter by column: Date Modified
      if (columnFilters.date) {
        if (!matchesModifiedFilter(f.lastModified, columnFilters.date)) {
          return false;
        }
      }

      // Filter by column: Size
      if (columnFilters.size) {
        if (!matchesSizeFilter(f.size || 0, columnFilters.size)) {
          return false;
        }
      }

      // Filter by column: Tier
      if (columnFilters.tier) {
        const fileTier = f.tierStatus?.toLowerCase() || 'hot';
        if (fileTier !== columnFilters.tier.toLowerCase()) {
          return false;
        }
      }

      // Filter by text search (name) - only if column filter is not set
      if (
        textSearch &&
        !columnFilters.name &&
        !f.name.toLowerCase().includes(textSearch.toLowerCase())
      ) {
        return false;
      }

      // Filter by tag: operators in search (AND logic - file must have ALL specified tags)
      if (searchTagFilters && searchTagFilters.length > 0) {
        const fileTags = (f.tags || []).map((t) => {
          const tagName = typeof t === 'string' ? t : t.name;
          return tagName.toLowerCase();
        });
        // Check if file has ALL specified tags
        const hasAllTags = searchTagFilters.every((filterTag) =>
          fileTags.some((fileTag) => fileTag.includes(filterTag)),
        );
        if (!hasAllTags) {
          return false;
        }
      }

      // Filter by sidebar tag filter
      if (filterByTag) {
        const fileTagNames = (f.tags || []).map((t) =>
          typeof t === 'string' ? t : t.name,
        );
        if (!fileTagNames.includes(filterByTag)) {
          return false;
        }
      }

      // Filter by type: operator (video, image, audio, document, folder, archive)
      if (typeFilter) {
        const mimeType = f.mimeType?.toLowerCase() || '';
        const isMatch =
          (typeFilter === 'video' && mimeType.startsWith('video/')) ||
          (typeFilter === 'image' && mimeType.startsWith('image/')) ||
          (typeFilter === 'audio' && mimeType.startsWith('audio/')) ||
          (typeFilter === 'document' &&
            (mimeType.includes('pdf') ||
              mimeType.includes('document') ||
              mimeType.includes('text/'))) ||
          (typeFilter === 'folder' &&
            (mimeType === 'folder' || f.isDirectory)) ||
          (typeFilter === 'archive' &&
            (mimeType.includes('zip') ||
              mimeType.includes('tar') ||
              mimeType.includes('rar') ||
              mimeType.includes('7z') ||
              f.name.match(/\.(zip|tar|gz|rar|7z|bz2)$/i)));
        if (!isMatch) return false;
      }

      // Filter by tier: operator
      if (tierFilter && f.tierStatus?.toLowerCase() !== tierFilter) {
        return false;
      }

      // Filter by ext: operator
      if (extFilter) {
        if (!f.name.includes('.')) return false; // Files without extensions don't match

        const parts = f.name.split('.');
        const simpleExt = parts[parts.length - 1]?.toLowerCase();

        // Check simple extension match (e.g., .mp4)
        if (simpleExt === extFilter.toLowerCase()) {
          // Match found
        } else if (parts.length > 2) {
          // Check compound extension (e.g., .tar.gz)
          const compoundExt =
            `${parts[parts.length - 2]}.${parts[parts.length - 1]}`.toLowerCase();
          if (compoundExt !== extFilter.toLowerCase()) {
            return false;
          }
        } else {
          return false;
        }
      }

      // Filter by is: operator (folder, file, hidden, cached, tagged)
      if (isFilter) {
        switch (isFilter) {
          case 'folder':
            if (!f.isDirectory) return false;
            break;
          case 'file':
            if (f.isDirectory) return false;
            break;
          case 'hidden':
            if (!(f.isHidden ?? f.name.startsWith('.'))) return false;
            break;
          case 'cached':
            if (!f.isCached) return false;
            break;
          case 'tagged':
            if (!(f.tags && f.tags.length > 0)) return false;
            break;
        }
      }

      // Filter by size: operator
      if (sizeFilter && !matchesSizeFilter(f.size || 0, sizeFilter)) {
        return false;
      }

      // Filter by modified: operator
      if (
        modifiedFilter &&
        !matchesModifiedFilter(f.lastModified, modifiedFilter)
      ) {
        return false;
      }

      // Filter hidden files unless showHiddenFiles is enabled
      const isHidden = f.isHidden ?? f.name.startsWith('.');
      if (!showHiddenFiles && isHidden) {
        return false;
      }

      return true;
    })
    .sort((a, b) => {
      // Always sort folders first
      const aIsFolder = a.isDirectory || a.mimeType === 'folder';
      const bIsFolder = b.isDirectory || b.mimeType === 'folder';
      if (aIsFolder && !bIsFolder) return -1;
      if (!aIsFolder && bIsFolder) return 1;

      // Sort by selected column
      let comparison = 0;
      switch (sortColumn) {
        case 'name':
          comparison = a.name.localeCompare(b.name, undefined, {
            sensitivity: 'base',
          });
          break;
        case 'modified': {
          const aDate = a.lastModified ? new Date(a.lastModified).getTime() : 0;
          const bDate = b.lastModified ? new Date(b.lastModified).getTime() : 0;
          comparison = aDate - bDate;
          break;
        }
        case 'size':
          comparison = (a.size || 0) - (b.size || 0);
          break;
        case 'storage-class': {
          const aTier = a.tierStatus || '';
          const bTier = b.tierStatus || '';
          comparison = aTier.localeCompare(bTier);
          break;
        }
      }

      return sortDirection === 'asc' ? comparison : -comparison;
    });

  const selectedFile =
    selectedFiles.size === 1
      ? files.find((f) => f.path === Array.from(selectedFiles)[0]) || null
      : null;

  // Helper to render a storage item in the sidebar
  // Sort locations in specific order: Home, Downloads, Pictures, Movies, Documents, Music
  const sortLocations = (a: StorageSource, b: StorageSource): number => {
    const order = [
      'Home',
      'Downloads',
      'Pictures',
      'Movies',
      'Documents',
      'Music',
    ];
    const aIndex = order.findIndex((name) => a.name === name);
    const bIndex = order.findIndex((name) => b.name === name);

    // If both are in the order list, sort by their position
    if (aIndex !== -1 && bIndex !== -1) {
      return aIndex - bIndex;
    }
    // If only a is in the order list, it comes first
    if (aIndex !== -1) {
      return -1;
    }
    // If only b is in the order list, it comes first
    if (bIndex !== -1) {
      return 1;
    }
    // If neither is in the order list, sort alphabetically
    return a.name.localeCompare(b.name);
  };

  const renderStorageItem = (source: StorageSource) => {
    const StorageIcon = getStorageIcon(source);
    const isDropTarget = dropTarget === `source:${source.id}`;
    const isSelected = selectedSource?.id === source.id;

    // Determine tier class:
    // - Local storage shows 'local'
    // - S3/Cloud storage: Show 'nearline' (N) for STANDARD, 'cold' (C) for GLACIER_IR/etc
    // - Others show 'hot'
    let tierClass: string;
    if (source.category === 'local') {
      tierClass = 'local';
    } else if (source.category === 'cloud') {
      // For S3/GCS/Azure, check if it's cold tier or default to nearline
      // tierStatus 'cold' means GLACIER_IR/etc, otherwise it's nearline (STANDARD)
      tierClass = source.tierStatus === 'cold' ? 'cold' : 'nearline';
    } else {
      tierClass = source.tierStatus || 'hot';
    }

    // Determine storage type label
    const getTypeLabel = (cat: string) => {
      switch (cat) {
        case 'local':
          return 'Local';
        case 'network':
          return 'Network';
        case 'cloud':
          return 'Object';
        case 'hybrid':
          return 'Hybrid';
        case 'block':
          return 'Block';
        default:
          return cat;
      }
    };

    // Callback ref to scroll into view when selected
    const itemRefCallback = (element: HTMLButtonElement | null) => {
      if (element && isSelected) {
        // Use setTimeout to ensure DOM is updated
        setTimeout(() => {
          element.scrollIntoView({
            behavior: 'smooth',
            block: 'nearest',
            inline: 'nearest',
          });
        }, 100);
      }
    };

    return (
      <button
        ref={itemRefCallback}
        key={source.id}
        data-source-id={source.id}
        className={`sidebar-item storage-item ${isSelected ? 'active' : ''} ${isDropTarget ? 'drop-target' : ''}`}
        onClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          selectSource(source);
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          setStorageContextMenu({
            source,
            x: e.clientX,
            y: e.clientY,
          });
        }}
        onDragOver={(e) => {
          e.preventDefault();
          if (dragSourceId !== source.id) {
            setDropTarget(`source:${source.id}`);
          }
        }}
        onDragLeave={() => setDropTarget(null)}
        onDrop={(e) => handleDropOnSource(e, source)}
      >
        <span className="item-icon">
          <StorageIcon size={16} />
        </span>
        <span className="item-name" title={source.name}>
          <span>{source.name}</span>
          {/* Only show bucket name for object storage, not for mounted network storage */}
          {source.category === 'cloud' &&
            isObjectStorage(source) &&
            source.config &&
            'bucket' in source.config &&
            typeof source.config.bucket === 'string' && (
              <span className="item-subtitle">({source.config.bucket})</span>
            )}
        </span>
        <span className="storage-badges">
          {/* Storage Class badge - use category, not tier status */}
          {(() => {
            const badge = getStorageClassBadge(
              source.category,
              source.tierStatus,
            );
            if (!badge.letter) return null;
            return (
              <span
                className={`storage-tier-badge ${badge.tierClass}`}
                title={getCategoryName(source.category)}
              >
                {badge.letter}
              </span>
            );
          })()}
        </span>
        {source.status !== 'connected' && (
          <span className="offline-dot" title="Disconnected" />
        )}
      </button>
    );
  };

  return (
    <div className="finder">
      {/* Main Tab Navigation */}
      <div className="finder-tabs">
        <button
          className={`finder-tab ${activeTab === 'files' ? 'active' : ''}`}
          onClick={() => setActiveTab('files')}
        >
          <IconFolder size={16} />
          <span>Files</span>
        </button>
        <button
          className={`finder-tab ${activeTab === 'transfers' ? 'active' : ''}`}
          onClick={() => setActiveTab('transfers')}
        >
          <svg viewBox="0 0 16 16" fill="currentColor" width="16" height="16">
            <path d="M.5 9.9a.5.5 0 0 1 .5.5v2.5a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-2.5a.5.5 0 0 1 1 0v2.5a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2v-2.5a.5.5 0 0 1 .5-.5z" />
            <path d="M7.646 11.854a.5.5 0 0 0 .708 0l3-3a.5.5 0 0 0-.708-.708L8.5 10.293V1.5a.5.5 0 0 0-1 0v8.793L5.354 8.146a.5.5 0 1 0-.708.708l3 3z" />
          </svg>
          <span>Operations</span>
        </button>
      </div>

      {/* Files Tab Content */}
      {activeTab === 'files' && (
        <>
          {/* Toolbar */}
          <FinderToolbar
            canGoBack={canGoBack}
            canGoForward={canGoForward}
            canGoUp={canGoUp}
            viewMode={viewMode}
            searchQuery={searchQuery}
            showHiddenFiles={showHiddenFiles}
            showInfoPanel={showInfoPanel}
            files={files}
            selectedSource={selectedSource}
            breadcrumbs={getBreadcrumbs()}
            onGoBack={goBack}
            onGoForward={goForward}
            onGoUp={goUp}
            onNavigateTo={navigateTo}
            onSetViewMode={setViewMode}
            onSetSearchQuery={setSearchQuery}
            onSetShowHiddenFiles={setShowHiddenFiles}
            onSetShowInfoPanel={setShowInfoPanel}
            onHandleUpload={handleUpload}
          />

          <div className="finder-body">
            {/* Sidebar - Only render when sources are loaded */}
            {sourcesLoaded && (
              <aside
                className="finder-sidebar"
                style={{ width: `${sidebarWidth}px` }}
              >
                <div className="finder-sidebar-scrollable">
                  <div
                    className={`sidebar-section favorites-section ${dropTarget === 'favorites' ? 'drop-target' : ''}`}
                    onDragOver={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      setDropTarget('favorites');
                      e.dataTransfer.dropEffect = 'link';
                      console.log(
                        '[Favorites] Drag over, dropEffect:',
                        e.dataTransfer.dropEffect,
                      );
                    }}
                    onDragLeave={(e) => {
                      e.preventDefault();
                      if (!e.currentTarget.contains(e.relatedTarget as Node)) {
                        setDropTarget(null);
                      }
                    }}
                    onDrop={async (e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      setDropTarget(null);

                      console.log('[Favorites] Drop event triggered');
                      console.log('[Favorites] Drag state:', {
                        draggedFiles,
                        draggedFileObjects: draggedFileObjects.length,
                        dragSourceId,
                      });

                      let dropSourceId: string | null = null;
                      let pathsToAdd: string[] = [];
                      let fileObjectsToAdd: FileMetadata[] = [];

                      // 1. Check for native file drops (local folders, network drives)
                      // In Tauri, native file drops can be handled via dataTransfer.files
                      // but we need to get the actual file paths
                      const nativeFiles = e.dataTransfer.files;

                      // Prioritize Tauri file-drop event paths if available
                      let filePaths: string[] = [];
                      if (nativeFileDropPaths.length > 0) {
                        filePaths = [...nativeFileDropPaths];
                        // Clear the paths after use
                        setNativeFileDropPaths([]);
                      }

                      if (
                        nativeFiles &&
                        nativeFiles.length > 0 &&
                        filePaths.length === 0
                      ) {
                        // Handle native file system drops
                        const fileArray = Array.from(nativeFiles);

                        // Try to get file paths from dataTransfer items
                        const dataTransferItems = e.dataTransfer.items;

                        if (dataTransferItems) {
                          for (let i = 0; i < dataTransferItems.length; i++) {
                            const item = dataTransferItems[i];
                            if (item.kind === 'file') {
                              // Try to get the file entry
                              const entry = item.webkitGetAsEntry();
                              if (entry) {
                                // For Tauri, the path might be in the entry
                                const entryWithPath =
                                  entry as unknown as Record<string, unknown>;
                                const fullPath =
                                  (entryWithPath.fullPath as
                                    | string
                                    | undefined) ||
                                  (entryWithPath.path as string | undefined) ||
                                  entry.name;
                                filePaths.push(fullPath);
                              } else {
                                // Fallback: try to get path from file object
                                const file = fileArray[i];
                                const fileWithPath = file as unknown as Record<
                                  string,
                                  unknown
                                >;
                                const path =
                                  (fileWithPath.path as string | undefined) ||
                                  file.name;
                                filePaths.push(path);
                              }
                            }
                          }
                        } else {
                          // Fallback: use file names (paths may not be available)
                          filePaths.push(
                            ...fileArray.map((f) => {
                              const fileWithPath = f as unknown as Record<
                                string,
                                unknown
                              >;
                              return (
                                (fileWithPath.path as string | undefined) ||
                                f.name
                              );
                            }),
                          );
                        }
                      }

                      if (filePaths.length > 0) {
                        // Find appropriate source (local, network, or current source)
                        let targetSource = sources.find(
                          (s) => s.category === 'local',
                        );
                        if (!targetSource) {
                          targetSource = sources.find(
                            (s) => s.category === 'network',
                          );
                        }
                        // If still no source, use selected source if it's local/network
                        if (
                          !targetSource &&
                          selectedSource &&
                          (selectedSource.category === 'local' ||
                            selectedSource.category === 'network')
                        ) {
                          targetSource = selectedSource;
                        }

                        if (targetSource) {
                          dropSourceId = targetSource.id;

                          // Process each dropped file/folder
                          const fileArray = nativeFiles
                            ? Array.from(nativeFiles)
                            : [];
                          for (let i = 0; i < filePaths.length; i++) {
                            const filePath = filePaths[i];
                            const nativeFile = fileArray[i];
                            const fileName = nativeFile
                              ? nativeFile.name
                              : filePath.split('/').pop() || filePath;

                            // Normalize path - ensure it's absolute
                            let normalizedPath = filePath;
                            if (!normalizedPath.startsWith('/')) {
                              normalizedPath = `/${normalizedPath}`;
                            }

                            // Check if it's a directory
                            let isDir = false;
                            try {
                              if (targetSource.category === 'local') {
                                // For local files, use vfs_is_directory
                                isDir = await invoke<boolean>(
                                  'vfs_is_directory',
                                  {
                                    path: normalizedPath,
                                  },
                                );
                              } else if (targetSource.category === 'network') {
                                // For network drives, try to list the path
                                try {
                                  await invoke<FileMetadata[]>(
                                    'vfs_list_files',
                                    {
                                      sourceId: targetSource.id,
                                      path: normalizedPath,
                                    },
                                  );
                                  isDir = true;
                                } catch {
                                  // Listing failed, check if path suggests it's a directory
                                  isDir =
                                    normalizedPath.endsWith('/') ||
                                    (nativeFile && nativeFile.type === '') ||
                                    !normalizedPath.includes('.');
                                }
                              } else {
                                // Fallback: infer from path
                                isDir =
                                  normalizedPath.endsWith('/') ||
                                  (nativeFile && nativeFile.type === '') ||
                                  !normalizedPath.includes('.');
                              }
                            } catch (err) {
                              // Fallback: infer from path and file properties
                              isDir =
                                normalizedPath.endsWith('/') ||
                                (nativeFile && nativeFile.type === '') ||
                                !normalizedPath.includes('.');
                            }

                            // Ensure directory paths end with /
                            if (isDir && !normalizedPath.endsWith('/')) {
                              normalizedPath = `${normalizedPath}/`;
                            }

                            // Create file metadata object
                            const fileMetadata: FileMetadata = {
                              id: normalizedPath,
                              name: fileName,
                              path: normalizedPath,
                              size: nativeFile ? nativeFile.size : 0,
                              mimeType: isDir
                                ? 'folder'
                                : nativeFile
                                  ? nativeFile.type
                                  : 'application/octet-stream',
                              isDirectory: isDir,
                              lastModified: nativeFile
                                ? new Date(
                                    nativeFile.lastModified,
                                  ).toISOString()
                                : new Date().toISOString(),
                              tierStatus:
                                targetSource.category === 'cloud'
                                  ? 'nearline'
                                  : 'hot',
                              canWarm: targetSource.category === 'cloud',
                              canTranscode: false,
                            };

                            fileObjectsToAdd.push(fileMetadata);
                            pathsToAdd.push(normalizedPath);
                          }
                        }
                      }

                      // 2. Check for VFS drag data (files/folders dragged from within the app)
                      if (fileObjectsToAdd.length === 0) {
                        const vfsData = e.dataTransfer.getData(
                          'application/x-vfs-files',
                        );

                        if (vfsData) {
                          try {
                            const parsed = JSON.parse(vfsData) as {
                              sourceId: string;
                              paths: string[];
                            };
                            dropSourceId = parsed.sourceId;
                            pathsToAdd = parsed.paths;
                          } catch (err) {
                            console.error(
                              'Failed to parse VFS drag data:',
                              err,
                            );
                          }
                        }

                        // 3. Fallback to text/plain data (file paths)
                        if (pathsToAdd.length === 0) {
                          const textData = e.dataTransfer.getData('text/plain');
                          if (textData) {
                            pathsToAdd = textData.split('\n').filter(Boolean);
                          }
                        }

                        // 4. Fallback to state if drag data not available
                        if (!dropSourceId) {
                          dropSourceId =
                            dragSourceId || selectedSource?.id || null;
                        }
                        if (pathsToAdd.length === 0) {
                          pathsToAdd =
                            draggedFiles.length > 0 ? draggedFiles : [];
                        }

                        // 5. Use draggedFileObjects if available (more reliable)
                        if (draggedFileObjects.length > 0) {
                          fileObjectsToAdd = draggedFileObjects;
                        }
                      }

                      // Find the source
                      const dropSource =
                        (dropSourceId
                          ? sources.find((s) => s.id === dropSourceId)
                          : null) || selectedSource;

                      if (!dropSource) {
                        console.error('No source found for favorites drop');
                        setDraggedFiles([]);
                        setDraggedFileObjects([]);
                        setDragSourceId(null);
                        return;
                      }

                      // Add files/folders to favorites
                      if (fileObjectsToAdd.length > 0) {
                        // Use file objects directly (most reliable)
                        for (const file of fileObjectsToAdd) {
                          const favoriteId = `${dropSource.id}:${file.path}`;
                          if (!favorites.some((f) => f.id === favoriteId)) {
                            addToGlobalFavorites(file, dropSource);
                          }
                        }
                      } else if (pathsToAdd.length > 0) {
                        // Fallback: create file objects from paths
                        for (let filePath of pathsToAdd) {
                          // Try to find file in current files list first
                          let file = files.find((f) => f.path === filePath);

                          // If not found, check if it's a directory and create metadata
                          if (!file) {
                            let isDir = filePath.endsWith('/');

                            try {
                              if (!isDir) {
                                if (dropSource.category === 'local') {
                                  // For local files, use vfs_is_directory
                                  isDir = await invoke<boolean>(
                                    'vfs_is_directory',
                                    {
                                      path: filePath,
                                    },
                                  );
                                } else if (dropSource.category === 'network') {
                                  // For network drives, try to list the path
                                  try {
                                    await invoke<FileMetadata[]>(
                                      'vfs_list_files',
                                      {
                                        sourceId: dropSource.id,
                                        path: filePath,
                                      },
                                    );
                                    isDir = true;
                                  } catch {
                                    // Listing failed, assume it's a file
                                    isDir = false;
                                  }
                                } else if (dropSource.category === 'cloud') {
                                  // For object storage (S3, etc.), try to list the path
                                  try {
                                    await invoke<FileMetadata[]>(
                                      'vfs_list_files',
                                      {
                                        sourceId: dropSource.id,
                                        path: filePath,
                                      },
                                    );
                                    isDir = true;
                                  } catch {
                                    // Listing failed, check if path suggests directory
                                    // In S3, folders often end with / or have no extension
                                    isDir =
                                      filePath.endsWith('/') ||
                                      !filePath.includes('.');
                                  }
                                } else {
                                  // Fallback: infer from path
                                  isDir =
                                    filePath.endsWith('/') ||
                                    !filePath.includes('.');
                                }
                              }

                              // Ensure directory paths end with /
                              if (isDir && !filePath.endsWith('/')) {
                                filePath = `${filePath}/`;
                              }

                              const fileName =
                                filePath.split('/').filter(Boolean).pop() ||
                                filePath;
                              file = {
                                id: filePath,
                                name: fileName,
                                path: filePath,
                                size: 0,
                                mimeType: isDir
                                  ? 'folder'
                                  : 'application/octet-stream',
                                isDirectory: isDir,
                                lastModified: new Date().toISOString(),
                                tierStatus:
                                  dropSource.category === 'cloud'
                                    ? 'nearline'
                                    : 'hot',
                                canWarm: dropSource.category === 'cloud',
                                canTranscode: false,
                              };
                            } catch (err) {
                              // If check fails, infer from path
                              console.warn(
                                'Failed to check if path is directory:',
                                err,
                              );
                              const fileName =
                                filePath.split('/').filter(Boolean).pop() ||
                                filePath;
                              isDir =
                                filePath.endsWith('/') ||
                                !filePath.includes('.');
                              if (isDir && !filePath.endsWith('/')) {
                                filePath = `${filePath}/`;
                              }
                              file = {
                                id: filePath,
                                name: fileName,
                                path: filePath,
                                size: 0,
                                mimeType: isDir
                                  ? 'folder'
                                  : 'application/octet-stream',
                                isDirectory: isDir,
                                lastModified: new Date().toISOString(),
                                tierStatus:
                                  dropSource.category === 'cloud'
                                    ? 'nearline'
                                    : 'hot',
                                canWarm: dropSource.category === 'cloud',
                                canTranscode: false,
                              };
                            }
                          }

                          // Only add if not already in favorites
                          if (file) {
                            const favoriteId = `${dropSource.id}:${file.path}`;
                            if (!favorites.some((f) => f.id === favoriteId)) {
                              addToGlobalFavorites(file, dropSource);
                            }
                          }
                        }
                      }

                      setDraggedFiles([]);
                      setDraggedFileObjects([]);
                      setDragSourceId(null);
                    }}
                  >
                    <div className="section-header">
                      <IconStar size={14} glow={false} />
                      <span>Favorites</span>
                      {favorites.length > 0 && (
                        <span className="section-count">
                          ({favorites.length})
                        </span>
                      )}
                    </div>
                    {favorites.length === 0 ? (
                      <div className="sidebar-empty">
                        <span className="empty-text">Drop files here</span>
                        <span className="empty-hint">
                          Drag to add favorites
                        </span>
                      </div>
                    ) : (
                      favorites.slice(0, 10).map((fav) => (
                        <button
                          key={fav.id}
                          className="sidebar-item"
                          onClick={() => navigateToFavorite(fav)}
                          onContextMenu={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            removeFromGlobalFavorites(fav.id);
                          }}
                          title={`${fav.sourceName}: ${fav.path}\nRight-click to remove`}
                        >
                          <span className="item-icon">
                            {fav.isDirectory ? (
                              <IconFolder size={16} />
                            ) : (
                              <IconStar size={16} />
                            )}
                          </span>
                          <span className="item-name">{fav.name}</span>
                        </button>
                      ))
                    )}
                    {favorites.length > 10 && (
                      <div className="sidebar-item show-more">
                        <span className="item-icon">+</span>
                        <span>{favorites.length - 10} more</span>
                      </div>
                    )}
                  </div>

                  {/* Storage Section - Grouped by type with collapsible submenus */}
                  <div className="sidebar-section storage-section">
                    <div className="section-header">
                      <IconDatabase size={14} glow={false} />
                      <span>Storage</span>
                      <span className="section-count">({sources.length})</span>
                    </div>

                    {/* Track rendered source IDs to prevent duplicates - removed unused variable */}

                    {/* Empty state when no sources */}
                    {sources.length === 0 && (
                      <div className="sidebar-empty">
                        <span className="empty-text">No storage connected</span>
                        <button
                          className="add-storage-btn"
                          onClick={() => setShowAddStorage(true)}
                          style={{
                            marginTop: '8px',
                            padding: '6px 12px',
                            fontSize: '12px',
                            background: 'var(--primary, #0a84ff)',
                            color: 'white',
                            border: 'none',
                            borderRadius: '6px',
                            cursor: 'pointer',
                          }}
                        >
                          + Add Storage
                        </button>
                      </div>
                    )}

                    {/* Local Storage - Top level with Volumes and Locations as sub-items */}
                    {localSources.length > 0 && (
                      <div
                        className={`storage-group ${collapsedGroups.has('local') ? 'collapsed' : ''}`}
                      >
                        <button
                          className="storage-group-header"
                          onClick={() => toggleGroup('local')}
                        >
                          <span className="group-chevron">
                            <svg viewBox="0 0 16 16" fill="currentColor">
                              <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
                            </svg>
                          </span>
                          <span className="group-icon local">
                            <svg viewBox="0 0 16 16" fill="currentColor">
                              <path d="M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14zm0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16z" />
                              <path d="M8 4a.5.5 0 0 1 .5.5v3h3a.5.5 0 0 1 0 1h-3v3a.5.5 0 0 1-1 0v-3h-3a.5.5 0 0 1 0-1h3v-3A.5.5 0 0 1 8 4z" />
                            </svg>
                          </span>
                          <span className="group-label">Local</span>
                          <span className="group-count">
                            {localSources.length}
                          </span>
                        </button>
                        <div className="storage-group-items">
                          {/* System Locations (non-ejectable) - ALWAYS DISPLAY FIRST */}
                          {locations.length > 0 && (
                            <div
                              className={`storage-subgroup ${collapsedGroups.has('locations') ? 'collapsed' : ''}`}
                            >
                              <button
                                className="storage-group-header subgroup"
                                onClick={() => toggleGroup('locations')}
                              >
                                <span className="group-chevron">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
                                  </svg>
                                </span>
                                <span className="group-icon locations">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h2.764c.958 0 1.76.56 2.311 1.184C7.985 3.648 8.48 4 9 4h4.5A1.5 1.5 0 0 1 15 5.5v7a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z" />
                                  </svg>
                                </span>
                                <span className="group-label">Locations</span>
                                <span className="group-count">
                                  {locations.length}
                                </span>
                              </button>
                              <div className="storage-group-items">
                                {(() => {
                                  const renderedIds = new Set<string>();
                                  return locations
                                    .filter((source) => {
                                      if (renderedIds.has(source.id)) {
                                        console.warn(
                                          '[FinderPage] Skipping duplicate Local source render:',
                                          source.id,
                                          source.name,
                                        );
                                        return false;
                                      }
                                      renderedIds.add(source.id);
                                      return true;
                                    })
                                    .sort(sortLocations) // Sort locations in specified order
                                    .map((source) => renderStorageItem(source));
                                })()}
                              </div>
                            </div>
                          )}

                          {/* Mounted Volumes (ejectable) */}
                          {volumes.length > 0 && (
                            <div
                              className={`storage-subgroup ${collapsedGroups.has('volumes') ? 'collapsed' : ''}`}
                            >
                              <button
                                className="storage-group-header subgroup"
                                onClick={() => toggleGroup('volumes')}
                              >
                                <span className="group-chevron">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
                                  </svg>
                                </span>
                                <span className="group-icon volumes">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M0 1.5A1.5 1.5 0 0 1 1.5 0h13A1.5 1.5 0 0 1 16 1.5v2A1.5 1.5 0 0 1 14.5 5h-13A1.5 1.5 0 0 1 0 3.5v-2zM1.5 1a.5.5 0 0 0-.5.5v2a.5.5 0 0 0 .5.5h13a.5.5 0 0 0 .5-.5v-2a.5.5 0 0 0-.5-.5h-13z" />
                                    <path d="M2 2.5a.5.5 0 0 1 .5-.5h2a.5.5 0 0 1 0 1h-2a.5.5 0 0 1-.5-.5zm10 0a.5.5 0 1 1 1 0 .5.5 0 0 1-1 0z" />
                                  </svg>
                                </span>
                                <span className="group-label">Volumes</span>
                                <span className="group-count">
                                  {volumes.length}
                                </span>
                              </button>
                              <div className="storage-group-items">
                                {(() => {
                                  const renderedIds = new Set<string>();
                                  return volumes
                                    .filter((source) => {
                                      if (renderedIds.has(source.id)) {
                                        console.warn(
                                          '[FinderPage] Skipping duplicate Volume source render:',
                                          source.id,
                                          source.name,
                                        );
                                        return false;
                                      }
                                      renderedIds.add(source.id);
                                      return true;
                                    })
                                    .map((source) => renderStorageItem(source));
                                })()}
                              </div>
                            </div>
                          )}
                        </div>
                      </div>
                    )}

                    {/* Network Storage (NFS, SMB, NAS) */}
                    {sources.filter(
                      (s) =>
                        s.category === 'network' || s.category === 'hybrid',
                    ).length > 0 && (
                      <div
                        className={`storage-group ${collapsedGroups.has('network') ? 'collapsed' : ''}`}
                      >
                        <button
                          className="storage-group-header"
                          onClick={() => toggleGroup('network')}
                        >
                          <span className="group-chevron">
                            <svg viewBox="0 0 16 16" fill="currentColor">
                              <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
                            </svg>
                          </span>
                          <span className="group-icon network">
                            <svg viewBox="0 0 16 16" fill="currentColor">
                              <path d="M0 8a4 4 0 0 1 4-4h8a4 4 0 0 1 0 8H4a4 4 0 0 1-4-4zm4-3a3 3 0 0 0 0 6h8a3 3 0 0 0 0-6H4z" />
                              <path d="M8 8a1 1 0 1 0 0-2 1 1 0 0 0 0 2z" />
                            </svg>
                          </span>
                          <span className="group-label">Network</span>
                          <span className="group-count">
                            {networkSources.length}
                          </span>
                        </button>
                        <div className="storage-group-items">
                          {/* Network Shares (NFS, SMB, SFTP, WebDAV, iSCSI) */}
                          {networkSources.length > 0 && (
                            <div
                              className={`storage-subgroup ${collapsedGroups.has('network-shares') ? 'collapsed' : ''}`}
                            >
                              <button
                                className="storage-group-header subgroup"
                                onClick={() => toggleGroup('network-shares')}
                              >
                                <span className="group-chevron">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
                                  </svg>
                                </span>
                                <span className="group-icon network-shares">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M3 4.5a.5.5 0 0 1 .5-.5h6a.5.5 0 1 1 0 1h-6a.5.5 0 0 1-.5-.5zm0 2a.5.5 0 0 1 .5-.5h6a.5.5 0 0 1 0 1h-6a.5.5 0 0 1-.5-.5zm0 2a.5.5 0 0 1 .5-.5h6a.5.5 0 0 1 0 1h-6a.5.5 0 0 1-.5-.5zm0 2a.5.5 0 0 1 .5-.5h6a.5.5 0 0 1 0 1h-6a.5.5 0 0 1-.5-.5zm0 2a.5.5 0 0 1 .5-.5h6a.5.5 0 0 1 0 1h-6a.5.5 0 0 1-.5-.5zM11.5 4a.5.5 0 0 0 0 1h1a.5.5 0 0 0 0-1h-1zm0 2a.5.5 0 0 0 0 1h1a.5.5 0 0 0 0-1h-1zm0 2a.5.5 0 0 0 0 1h1a.5.5 0 0 0 0-1h-1zm0 2a.5.5 0 0 0 0 1h1a.5.5 0 0 0 0-1h-1zm0 2a.5.5 0 0 0 0 1h1a.5.5 0 0 0 0-1h-1z" />
                                    <path d="M2.354.646a.5.5 0 0 0-.801.13l-.5 1A.5.5 0 0 0 1 2v13H.5a.5.5 0 0 0 0 1h15a.5.5 0 0 0 0-1H15V2a.5.5 0 0 0-.053-.224l-.5-1a.5.5 0 0 0-.8-.13L13 1.293l-.646-.647a.5.5 0 0 0-.708 0L11 1.293l-.646-.647a.5.5 0 0 0-.708 0L9 1.293 8.354.646a.5.5 0 0 0-.708 0L7 1.293 6.354.646a.5.5 0 0 0-.708 0L5 1.293 4.354.646a.5.5 0 0 0-.708 0L3 1.293 2.354.646zM14 15H2V2.5l.5-.5.5.5.5-.5.5.5.5-.5.5.5.5-.5.5.5.5-.5.5.5.5-.5.5.5.5-.5.5.5V15z" />
                                  </svg>
                                </span>
                                <span className="group-label">
                                  Network Shares
                                </span>
                                <span className="group-count">
                                  {networkSources.length}
                                </span>
                              </button>
                              <div className="storage-group-items">
                                {(() => {
                                  const renderedIds = new Set<string>();
                                  return networkSources
                                    .filter((source) => {
                                      if (renderedIds.has(source.id)) {
                                        console.warn(
                                          '[FinderPage] Skipping duplicate Network source render:',
                                          source.id,
                                          source.name,
                                        );
                                        return false;
                                      }
                                      renderedIds.add(source.id);
                                      return true;
                                    })
                                    .map((source) => renderStorageItem(source));
                                })()}
                              </div>
                            </div>
                          )}
                        </div>
                      </div>
                    )}

                    {/* Cloud Storage - Top level with CSPs (AWS, Azure, GCP) as sub-items */}
                    {cloudSources.length > 0 && (
                      <div
                        className={`storage-group ${collapsedGroups.has('cloud') ? 'collapsed' : ''}`}
                      >
                        <button
                          className="storage-group-header"
                          onClick={() => toggleGroup('cloud')}
                        >
                          <span className="group-chevron">
                            <svg viewBox="0 0 16 16" fill="currentColor">
                              <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
                            </svg>
                          </span>
                          <span className="group-icon cloud">
                            <svg viewBox="0 0 16 16" fill="currentColor">
                              <path d="M4.406 3.342A5.53 5.53 0 0 1 8 2c2.69 0 4.923 2 5.166 4.579C14.758 6.804 16 8.137 16 9.773 16 11.569 14.502 13 12.687 13H3.781C1.708 13 0 11.366 0 9.318c0-1.763 1.266-3.223 2.942-3.593.143-.863.698-1.723 1.464-2.383z" />
                            </svg>
                          </span>
                          <span className="group-label">Cloud</span>
                          <span className="group-count">
                            {cloudSources.length}
                          </span>
                        </button>
                        <div className="storage-group-items">
                          {/* AWS (S3, S3-Compatible) */}
                          {awsSources.length > 0 && (
                            <div
                              className={`storage-subgroup ${collapsedGroups.has('aws') ? 'collapsed' : ''}`}
                            >
                              <button
                                className="storage-group-header subgroup"
                                onClick={() => toggleGroup('aws')}
                              >
                                <span className="group-chevron">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
                                  </svg>
                                </span>
                                <span className="group-icon aws">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M8 0a8 8 0 1 0 0 16A8 8 0 0 0 8 0zM7 11.5a.5.5 0 0 1-1 0V7.707L5.354 8.854a.5.5 0 1 1-.708-.708l2-2a.5.5 0 0 1 .708 0l2 2a.5.5 0 0 1-.708.708L7 7.707V11.5z" />
                                  </svg>
                                </span>
                                <span className="group-label">AWS</span>
                                <span className="group-count">
                                  {awsSources.length}
                                </span>
                              </button>
                              <div className="storage-group-items">
                                {(() => {
                                  const renderedIds = new Set<string>();
                                  return awsSources
                                    .filter((source) => {
                                      if (renderedIds.has(source.id)) {
                                        console.warn(
                                          '[FinderPage] Skipping duplicate AWS source render:',
                                          source.id,
                                          source.name,
                                        );
                                        return false;
                                      }
                                      renderedIds.add(source.id);
                                      return true;
                                    })
                                    .map((source) => renderStorageItem(source));
                                })()}
                              </div>
                            </div>
                          )}

                          {/* Azure Blob Storage */}
                          {azureSources.length > 0 && (
                            <div
                              className={`storage-subgroup ${collapsedGroups.has('azure') ? 'collapsed' : ''}`}
                            >
                              <button
                                className="storage-group-header subgroup"
                                onClick={() => toggleGroup('azure')}
                              >
                                <span className="group-chevron">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
                                  </svg>
                                </span>
                                <span className="group-icon azure">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M8 0a8 8 0 1 0 0 16A8 8 0 0 0 8 0zM7 11.5a.5.5 0 0 1-1 0V7.707L5.354 8.854a.5.5 0 1 1-.708-.708l2-2a.5.5 0 0 1 .708 0l2 2a.5.5 0 0 1-.708.708L7 7.707V11.5z" />
                                  </svg>
                                </span>
                                <span className="group-label">Azure</span>
                                <span className="group-count">
                                  {azureSources.length}
                                </span>
                              </button>
                              <div className="storage-group-items">
                                {(() => {
                                  const renderedIds = new Set<string>();
                                  return azureSources
                                    .filter((source) => {
                                      if (renderedIds.has(source.id)) {
                                        console.warn(
                                          '[FinderPage] Skipping duplicate Azure source render:',
                                          source.id,
                                          source.name,
                                        );
                                        return false;
                                      }
                                      renderedIds.add(source.id);
                                      return true;
                                    })
                                    .map((source) => renderStorageItem(source));
                                })()}
                              </div>
                            </div>
                          )}

                          {/* GCP (Google Cloud Storage) */}
                          {gcpSources.length > 0 && (
                            <div
                              className={`storage-subgroup ${collapsedGroups.has('gcp') ? 'collapsed' : ''}`}
                            >
                              <button
                                className="storage-group-header subgroup"
                                onClick={() => toggleGroup('gcp')}
                              >
                                <span className="group-chevron">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
                                  </svg>
                                </span>
                                <span className="group-icon gcp">
                                  <svg viewBox="0 0 16 16" fill="currentColor">
                                    <path d="M8 0a8 8 0 1 0 0 16A8 8 0 0 0 8 0zM7 11.5a.5.5 0 0 1-1 0V7.707L5.354 8.854a.5.5 0 1 1-.708-.708l2-2a.5.5 0 0 1 .708 0l2 2a.5.5 0 0 1-.708.708L7 7.707V11.5z" />
                                  </svg>
                                </span>
                                <span className="group-label">GCP</span>
                                <span className="group-count">
                                  {gcpSources.length}
                                </span>
                              </button>
                              <div className="storage-group-items">
                                {(() => {
                                  const renderedIds = new Set<string>();
                                  return gcpSources
                                    .filter((source) => {
                                      if (renderedIds.has(source.id)) {
                                        console.warn(
                                          '[FinderPage] Skipping duplicate GCP source render:',
                                          source.id,
                                          source.name,
                                        );
                                        return false;
                                      }
                                      renderedIds.add(source.id);
                                      return true;
                                    })
                                    .map((source) => renderStorageItem(source));
                                })()}
                              </div>
                            </div>
                          )}
                        </div>
                      </div>
                    )}

                    {/* Block Storage */}
                    {blockSources.length > 0 && (
                      <div
                        className={`storage-group ${collapsedGroups.has('block') ? 'collapsed' : ''}`}
                      >
                        <button
                          className="storage-group-header"
                          onClick={() => toggleGroup('block')}
                        >
                          <span className="group-chevron">
                            <svg viewBox="0 0 16 16" fill="currentColor">
                              <path d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z" />
                            </svg>
                          </span>
                          <span className="group-icon block">
                            <svg viewBox="0 0 16 16" fill="currentColor">
                              <path d="M0 1.5A1.5 1.5 0 0 1 1.5 0h13A1.5 1.5 0 0 1 16 1.5v2A1.5 1.5 0 0 1 14.5 5h-13A1.5 1.5 0 0 1 0 3.5v-2zM1.5 1a.5.5 0 0 0-.5.5v2a.5.5 0 0 0 .5.5h13a.5.5 0 0 0 .5-.5v-2a.5.5 0 0 0-.5-.5h-13z" />
                              <path d="M0 6.5A1.5 1.5 0 0 1 1.5 5h13A1.5 1.5 0 0 1 16 6.5v2A1.5 1.5 0 0 1 14.5 10h-13A1.5 1.5 0 0 1 0 8.5v-2z" />
                            </svg>
                          </span>
                          <span className="group-label">Block</span>
                          <span className="group-count">
                            {blockSources.length}
                          </span>
                        </button>
                        <div className="storage-group-items">
                          {(() => {
                            const renderedIds = new Set<string>();
                            return blockSources
                              .filter((source) => {
                                if (renderedIds.has(source.id)) {
                                  console.warn(
                                    '[FinderPage] Skipping duplicate Block source render:',
                                    source.id,
                                    source.name,
                                  );
                                  return false;
                                }
                                renderedIds.add(source.id);
                                return true;
                              })
                              .map((source) => renderStorageItem(source));
                          })()}
                        </div>
                      </div>
                    )}

                    {sources.length === 0 && (
                      <div className="sidebar-empty">
                        <span className="empty-text">No storage connected</span>
                      </div>
                    )}
                  </div>

                  {/* Add Storage Button */}
                  <div className="sidebar-section">
                    <button
                      className="add-storage-btn"
                      onClick={() => {
                        console.log('[FinderPage] Add Storage button clicked');
                        setShowAddStorage(true);
                      }}
                      title="Add Storage"
                    >
                      <span className="add-icon">+</span>
                      <span>Add Storage</span>
                    </button>
                  </div>

                  {/* Tags Section - Using same list design as Storage */}
                  <div className="sidebar-section storage-section">
                    <div className="section-header">
                      <IconTag size={14} glow={false} />
                      <span>Tags</span>
                      {allTags.length > 0 && (
                        <span className="section-count">
                          ({allTags.length})
                        </span>
                      )}
                    </div>
                    {filterByTag && (
                      <div className="storage-group-items">
                        <button
                          className="sidebar-item storage-item active filter-active"
                          onClick={() => setFilterByTag(null)}
                        >
                          <span className="item-icon">
                            <span
                              className="tag-dot"
                              style={{
                                background:
                                  allTags.find((t) => t.name === filterByTag)
                                    ?.color || 'var(--vfs-primary)',
                              }}
                            />
                          </span>
                          <span className="item-name">{filterByTag}</span>
                          <span className="clear-filter">✕</span>
                        </button>
                      </div>
                    )}
                    {allTags.length === 0 ? (
                      <div className="sidebar-empty">
                        <span className="empty-text">No tags yet</span>
                      </div>
                    ) : (
                      <div className="storage-group-items">
                        {allTags
                          .filter((t) => t.name !== filterByTag)
                          .slice(0, 8)
                          .map((tag) => (
                            <button
                              key={tag.name}
                              className={`sidebar-item storage-item ${filterByTag === tag.name ? 'active' : ''}`}
                              onClick={() => setFilterByTag(tag.name)}
                            >
                              <span className="item-icon">
                                <span
                                  className="tag-dot"
                                  style={{
                                    background:
                                      tag.color || 'var(--vfs-primary)',
                                  }}
                                />
                              </span>
                              <span className="item-name">{tag.name}</span>
                            </button>
                          ))}
                      </div>
                    )}
                  </div>
                </div>

                {/* Metrics Preview */}
                {onOpenMetrics && (
                  <MetricsPreview onOpenMetrics={onOpenMetrics} />
                )}
              </aside>
            )}

            {/* Resizer Handle */}
            <div
              className={`sidebar-resizer ${isResizing ? 'resizing' : ''}`}
              onMouseDown={handleResizeStart}
              title="Drag to resize sidebar"
            >
              <div className="resizer-handle" />
            </div>

            {/* Main Content */}
            <main
              className="finder-content file-browser"
              onContextMenu={(e) => handleContextMenu(e)}
              onDragOver={(e) => handleDragOver(e)}
              onDragLeave={handleDragLeave}
              onDrop={(e) => handleDrop(e, currentPath)}
            >
              <FileBrowser
                loading={loading}
                files={files}
                filteredFiles={filteredFiles}
                viewMode={viewMode}
                selectedFiles={selectedFiles}
                dropTarget={dropTarget}
                draggedFiles={draggedFiles}
                cutFiles={cutFiles}
                isDraggingOver={isDraggingOver}
                currentPath={currentPath}
                columnWidths={columnWidths}
                columnFilters={columnFilters}
                sortColumn={sortColumn}
                sortDirection={sortDirection}
                resizingColumn={resizingColumn}
                renamingFile={renamingFile}
                renameValue={renameValue}
                allTags={allTags}
                sourceCategory={selectedSource?.category}
                iconViewRef={iconViewRef}
                listViewRef={listViewRef}
                renameInputRef={renameInputRef}
                hasMore={paginationState.hasMore}
                isLoadingMore={paginationState.isLoadingMore}
                onLoadMore={handleLoadMore}
                onFileClick={handleFileClick}
                onFileDoubleClick={handleFileDoubleClick}
                onContextMenu={handleContextMenu}
                onDragOver={(e) => handleDragOver(e)}
                onDragLeave={() => handleDragLeave({} as React.DragEvent)}
                onDrop={(e) => handleDrop(e, currentPath)}
                onDragStart={handleDragStart}
                onDragEnd={handleDragEnd}
                onDragOverFile={handleDragOver}
                onSetColumnFilters={setColumnFilters}
                onSetColumnWidths={setColumnWidths}
                onHandleColumnResizeStart={handleColumnResizeStart}
                onSortChange={handleSortChange}
                onSetSelectedFiles={setSelectedFiles}
                onSetRenameValue={setRenameValue}
                onHandleRenameKeyDown={handleRenameKeyDown}
                onCommitRename={commitRename}
                onSetCommentModal={setCommentModal}
              />
            </main>

            {/* Info Panel */}
            {showInfoPanel && (
              <FinderInfoPanel
                selectedFile={selectedFile}
                files={files}
                isMountedStorage={isMountedStorage}
                onWarm={handleWarm}
                onTranscode={handleTranscode}
                onTranscribe={handleTranscribe}
              />
            )}
          </div>

          {/* Status Bar */}
          <div className="finder-statusbar">
            <span>
              {filteredFiles.length} items
              {selectedFiles.size > 0 && ` · ${selectedFiles.size} selected`}
              {showHiddenFiles && ` · Hidden files visible`}
            </span>
            {selectedSource && (
              <span className="statusbar-source">{selectedSource.name}</span>
            )}
          </div>

          {/* Context Menu */}
          <FinderContextMenu
            visible={contextMenu.visible}
            x={contextMenu.x}
            y={contextMenu.y}
            targetFile={contextMenu.targetFile}
            selectedSource={selectedSource}
            selectedFiles={selectedFiles}
            clipboardHasItems={clipboardHasFiles}
            showOpenWith={showOpenWith}
            appsLoading={appsLoading}
            availableApps={availableApps}
            isMountedStorage={isMountedStorage}
            onClose={closeContextMenu}
            onSetShowOpenWith={setShowOpenWith}
            onSetTierDialogPaths={setTierDialogPaths}
            onSetShowTierDialog={setShowTierDialog}
            onSetInfoModal={setInfoModal}
            onNavigateTo={navigateTo}
            onHandleOpenFile={handleOpenFile}
            onHandleOpenFileWith={handleOpenFileWith}
            onHandleDownloadFile={handleDownloadFile}
            onHandleDelete={handleDelete}
            onHandleCopy={handleCopy}
            onHandleCut={handleCut}
            onHandlePaste={handlePaste}
            onHandleRename={handleRename}
            onHandleNewFolder={handleNewFolder}
            onHandleTranscribe={handleTranscribe}
            onHandleAutoTag={handleAutoTag}
            onHandleUpload={handleUpload}
            onLoadAppsForFile={loadAppsForFile}
            onLoadFilesList={loadFilesList}
            currentPath={currentPath}
            onOpenSettings={onOpenSettings}
            aiModelsAvailable={aiModelsAvailable}
          />

          {/* Storage Context Menu - macOS Get Info style */}
          {storageContextMenu &&
            (() => {
              // Calculate position to ensure popover stays within viewport
              const popoverWidth = 320; // max-width from CSS
              const popoverHeight = 400; // estimated max height
              const padding = 16;

              let left = storageContextMenu.x;
              let top = storageContextMenu.y;

              // Adjust horizontal position if popover would go off-screen
              if (left + popoverWidth + padding > window.innerWidth) {
                left = window.innerWidth - popoverWidth - padding;
              }
              if (left < padding) {
                left = padding;
              }

              // Adjust vertical position if popover would go off-screen
              if (top + popoverHeight + padding > window.innerHeight) {
                top = window.innerHeight - popoverHeight - padding;
              }
              if (top < padding) {
                top = padding;
              }

              return (
                <div
                  className="storage-info-popover"
                  style={{
                    position: 'fixed',
                    left: `${left}px`,
                    top: `${top}px`,
                    maxHeight: `calc(100vh - ${top + padding}px)`,
                    overflowY: 'auto',
                  }}
                  onClick={(e) => e.stopPropagation()}
                >
                  {/* Header with icon and name */}
                  <div className="storage-info-hero">
                    <div
                      className={`storage-info-icon ${storageContextMenu.source.category}`}
                    >
                      {storageContextMenu.source.category === 'local' && (
                        <svg
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                        >
                          <rect x="4" y="2" width="16" height="20" rx="2" />
                          <line x1="8" y1="6" x2="16" y2="6" />
                          <line x1="8" y1="10" x2="16" y2="10" />
                          <circle cx="12" cy="17" r="2" />
                        </svg>
                      )}
                      {storageContextMenu.source.category === 'network' && (
                        <svg
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                        >
                          <circle cx="12" cy="12" r="10" />
                          <path d="M2 12h20" />
                          <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
                        </svg>
                      )}
                      {storageContextMenu.source.category === 'cloud' && (
                        <svg
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                        >
                          <path d="M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z" />
                        </svg>
                      )}
                      {(storageContextMenu.source.category === 'block' ||
                        storageContextMenu.source.category === 'hybrid') && (
                        <svg
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                        >
                          <rect x="2" y="4" width="20" height="6" rx="1" />
                          <rect x="2" y="14" width="20" height="6" rx="1" />
                          <circle cx="6" cy="7" r="1" fill="currentColor" />
                          <circle cx="6" cy="17" r="1" fill="currentColor" />
                        </svg>
                      )}
                    </div>
                    <div className="storage-info-title-area">
                      <h3 className="storage-info-name">
                        {storageContextMenu.source.name}
                      </h3>
                      <span
                        className={`storage-info-status ${storageContextMenu.source.status}`}
                      >
                        <span className="status-indicator" />
                        {storageContextMenu.source.status === 'connected'
                          ? 'Connected'
                          : 'Offline'}
                      </span>
                    </div>
                  </div>

                  {/* Info Grid */}
                  <div className="storage-info-grid">
                    <div className="info-row">
                      <span className="info-icon">
                        <svg viewBox="0 0 16 16" fill="currentColor">
                          <path d="M14.5 3a.5.5 0 0 1 .5.5v9a.5.5 0 0 1-.5.5h-13a.5.5 0 0 1-.5-.5v-9a.5.5 0 0 1 .5-.5h13zm-13-1A1.5 1.5 0 0 0 0 3.5v9A1.5 1.5 0 0 0 1.5 14h13a1.5 1.5 0 0 0 1.5-1.5v-9A1.5 1.5 0 0 0 14.5 2h-13z" />
                          <path d="M3 5.5a.5.5 0 0 1 .5-.5h9a.5.5 0 0 1 0 1h-9a.5.5 0 0 1-.5-.5zM3 8a.5.5 0 0 1 .5-.5h9a.5.5 0 0 1 0 1h-9A.5.5 0 0 1 3 8zm0 2.5a.5.5 0 0 1 .5-.5h6a.5.5 0 0 1 0 1h-6a.5.5 0 0 1-.5-.5z" />
                        </svg>
                      </span>
                      <span className="info-label">Kind</span>
                      <span className="info-value">
                        {storageContextMenu.source.category === 'local'
                          ? 'Local Volume'
                          : storageContextMenu.source.category === 'cloud'
                            ? 'Cloud Storage'
                            : storageContextMenu.source.category === 'network'
                              ? 'Network Volume'
                              : storageContextMenu.source.category === 'block'
                                ? 'Block Storage'
                                : 'Hybrid Volume'}
                      </span>
                    </div>

                    <div className="info-row">
                      <span className="info-icon">
                        <svg viewBox="0 0 16 16" fill="currentColor">
                          <path d="M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14zm0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16z" />
                          <path d="M8 4a.5.5 0 0 1 .5.5v3h3a.5.5 0 0 1 0 1h-3v3a.5.5 0 0 1-1 0v-3h-3a.5.5 0 0 1 0-1h3v-3A.5.5 0 0 1 8 4z" />
                        </svg>
                      </span>
                      <span className="info-label">Tier</span>
                      <span
                        className={`info-value tier-pill ${storageContextMenu.source.tierStatus || (storageContextMenu.source.category === 'cloud' ? 'cold' : 'hot')}`}
                      >
                        {(
                          storageContextMenu.source.tierStatus ||
                          (storageContextMenu.source.category === 'cloud'
                            ? 'cold'
                            : 'hot')
                        )
                          .charAt(0)
                          .toUpperCase() +
                          (
                            storageContextMenu.source.tierStatus ||
                            (storageContextMenu.source.category === 'cloud'
                              ? 'cold'
                              : 'hot')
                          ).slice(1)}
                      </span>
                    </div>

                    {storageContextMenu.source.path && (
                      <div className="info-row path-row">
                        <span className="info-icon">
                          <svg viewBox="0 0 16 16" fill="currentColor">
                            <path d="M3.5 0a.5.5 0 0 1 .5.5V1h8V.5a.5.5 0 0 1 1 0V1h1a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2V3a2 2 0 0 1 2-2h1V.5a.5.5 0 0 1 .5-.5zM2 2a1 1 0 0 0-1 1v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V3a1 1 0 0 0-1-1H2z" />
                            <path d="M2.5 4a.5.5 0 0 1 .5-.5h10a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5H3a.5.5 0 0 1-.5-.5V4z" />
                          </svg>
                        </span>
                        <span className="info-label">Path</span>
                        <span className="info-value path-value">
                          {storageContextMenu.source.path}
                        </span>
                      </div>
                    )}

                    {storageContextMenu.source.providerId && (
                      <div className="info-row">
                        <span className="info-icon">
                          <svg viewBox="0 0 16 16" fill="currentColor">
                            <path d="M1 0a1 1 0 0 0-1 1v14a1 1 0 0 0 1 1h5v-1H1V1h5V0H1zm9 0v1h5v14h-5v1h5a1 1 0 0 0 1-1V1a1 1 0 0 0-1-1h-5zM8 7a.5.5 0 0 0 0 1h3.793l-1.147 1.146a.5.5 0 0 0 .708.708l2-2a.5.5 0 0 0 0-.708l-2-2a.5.5 0 1 0-.708.708L11.793 7H8z" />
                          </svg>
                        </span>
                        <span className="info-label">Provider</span>
                        <span className="info-value">
                          {storageContextMenu.source.providerId.toUpperCase()}
                        </span>
                      </div>
                    )}
                  </div>

                  {/* Actions */}
                  <div className="storage-info-actions">
                    <button
                      className="storage-action-btn primary"
                      onClick={() => {
                        selectSource(storageContextMenu.source);
                        setStorageContextMenu(null);
                      }}
                    >
                      <svg viewBox="0 0 16 16" fill="currentColor">
                        <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h2.764c.958 0 1.76.56 2.311 1.184C7.985 3.648 8.48 4 9 4h4.5A1.5 1.5 0 0 1 15 5.5v7a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z" />
                      </svg>
                      Open
                    </button>
                    {/* Don't show edit/remove for system locations */}
                    {!storageContextMenu.source.isSystemLocation && (
                      <button
                        className="storage-action-btn"
                        onClick={() => {
                          handleEditStorage(storageContextMenu.source);
                        }}
                      >
                        <svg viewBox="0 0 16 16" fill="currentColor">
                          <path d="M12.146.146a.5.5 0 0 1 .708 0l3 3a.5.5 0 0 1 0 .708l-10 10a.5.5 0 0 1-.168.11l-5 2a.5.5 0 0 1-.65-.65l2-5a.5.5 0 0 1 .11-.168l10-10zM11.207 2.5 13.5 4.793 14.793 3.5 12.5 1.207 11.207 2.5zm1.586 3L10.5 3.207 4 9.707V10h.5a.5.5 0 0 1 .5.5v.5h.5a.5.5 0 0 1 .5.5v.5h.293l6.5-6.5zm-9.761 5.175-.106.106-1.528 3.821 3.821-1.528.106-.106A.5.5 0 0 1 5 12.5V12h-.5a.5.5 0 0 1-.5-.5V11h-.5a.5.5 0 0 1-.468-.325z" />
                        </svg>
                        Edit
                      </button>
                    )}
                    <button
                      className="storage-action-btn"
                      onClick={() => {
                        if (storageContextMenu.source.path) {
                          navigator.clipboard.writeText(
                            storageContextMenu.source.path,
                          );
                          toast.showToast({
                            type: 'success',
                            message: 'Path copied to clipboard',
                          });
                        }
                        setStorageContextMenu(null);
                      }}
                    >
                      <svg viewBox="0 0 16 16" fill="currentColor">
                        <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z" />
                        <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z" />
                      </svg>
                      Copy Path
                    </button>
                    {/* Don't show remove for system locations */}
                    {!storageContextMenu.source.isSystemLocation && (
                      <button
                        className="storage-action-btn danger"
                        onClick={async () => {
                          const source = storageContextMenu.source;
                          setStorageContextMenu(null);
                          await handleRemoveStorage(source.id);
                        }}
                      >
                        <svg viewBox="0 0 16 16" fill="currentColor">
                          <path d="M5.5 5.5A.5.5 0 0 1 6 6v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm2.5 0a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm3 .5a.5.5 0 0 0-1 0v6a.5.5 0 0 0 1 0V6z" />
                          <path
                            fillRule="evenodd"
                            d="M14.5 3a1 1 0 0 1-1 1H13v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4h-.5a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1H6a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1h3.5a1 1 0 0 1 1 1v1zM4.118 4 4 4.059V13a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V4.059L11.882 4H4.118zM2.5 3V2h11v1h-11z"
                          />
                        </svg>
                        Remove Storage
                      </button>
                    )}
                    {storageContextMenu.source.isEjectable && (
                      <button
                        className="storage-action-btn eject"
                        onClick={async () => {
                          const source = storageContextMenu.source;
                          setStorageContextMenu(null);

                          try {
                            await invoke('vfs_eject', { source_id: source.id });
                            toast.showToast({
                              type: 'success',
                              message: `Ejected ${source.name}`,
                            });
                            // Refresh sources list
                            await loadSourcesList();
                            // If the ejected source was selected, clear selection
                            if (selectedSource?.id === source.id) {
                              setSelectedSource(null);
                              setFiles([]);
                              setCurrentPath('');
                            }
                          } catch (err) {
                            toast.showToast({
                              type: 'error',
                              message: `Failed to eject: ${err}`,
                            });
                          }
                        }}
                      >
                        <svg viewBox="0 0 16 16" fill="currentColor">
                          <path d="M7.27 1.047a1 1 0 0 1 1.46 0l6.345 6.77c.6.638.146 1.683-.73 1.683H11.5v3a1 1 0 0 1-1 1h-5a1 1 0 0 1-1-1v-3H1.654C.78 9.5.326 8.455.924 7.816L7.27 1.047z" />
                        </svg>
                        Eject
                      </button>
                    )}
                  </div>
                </div>
              );
            })()}

          {/* Info Modal */}
          {infoModal.visible && infoModal.file && (
            <InfoModal
              file={infoModal.file}
              sourceId={selectedSource?.id}
              sourceCategory={selectedSource?.category}
              onClose={() => setInfoModal({ visible: false, file: null })}
              onToggleFavorite={(file) => handleToggleFavorite(file.path)}
              isFavorite={isFileFavorite(infoModal.file.path)}
              onAddTag={async (file, tagName) => {
                // Convert string tag name to tag object, preserving color from allTags if available
                const tagObj =
                  typeof tagName === 'string'
                    ? {
                        name: tagName,
                        color: allTags.find((t) => t.name === tagName)?.color,
                      }
                    : tagName;

                // Persist to backend
                if (selectedSource) {
                  try {
                    await invoke('vfs_add_tag', {
                      sourceId: selectedSource.id,
                      path: file.path,
                      tagName: tagObj.name,
                      tagColor: tagObj.color,
                    });
                  } catch (error) {
                    console.error('Failed to add tag:', error);
                  }
                }

                // Update file tags
                setFiles((prev) =>
                  prev.map((f) =>
                    f.id === file.id
                      ? {
                          ...f,
                          tags: [
                            ...(f.tags || []).filter((t) => {
                              const tName = typeof t === 'string' ? t : t.name;
                              return tName !== tagObj.name;
                            }),
                            tagObj,
                          ],
                        }
                      : f,
                  ),
                );
                // Update modal file
                setInfoModal((prev) =>
                  prev.file
                    ? {
                        ...prev,
                        file: {
                          ...prev.file,
                          tags: [
                            ...(prev.file.tags || []).filter((t) => {
                              const tName = typeof t === 'string' ? t : t.name;
                              return tName !== tagObj.name;
                            }),
                            tagObj,
                          ],
                        },
                      }
                    : prev,
                );
                // Add to global tags if new
                if (!allTags.some((t) => t.name === tagObj.name)) {
                  setAllTags((prev) => [
                    ...prev,
                    { name: tagObj.name, color: tagObj.color },
                  ]);
                }
              }}
              onRemoveTag={async (file, tagName) => {
                // Persist to backend
                if (selectedSource) {
                  try {
                    await invoke('vfs_remove_tag', {
                      source_id: selectedSource.id,
                      path: file.path,
                      tag: tagName,
                    });
                  } catch (error) {
                    console.error('Failed to remove tag:', error);
                  }
                }

                // Update file tags
                setFiles((prev) =>
                  prev.map((f) =>
                    f.id === file.id
                      ? {
                          ...f,
                          tags: (f.tags || []).filter((t) => {
                            const tName = typeof t === 'string' ? t : t.name;
                            return tName !== tagName;
                          }),
                        }
                      : f,
                  ),
                );
                // Update modal file
                setInfoModal((prev) =>
                  prev.file
                    ? {
                        ...prev,
                        file: {
                          ...prev.file,
                          tags: (prev.file.tags || []).filter((t) => {
                            const tName = typeof t === 'string' ? t : t.name;
                            return tName !== tagName;
                          }),
                        },
                      }
                    : prev,
                );
              }}
              onSetColorLabel={(file, color) => {
                // Update file color label
                setFiles((prev) =>
                  prev.map((f) =>
                    f.id === file.id
                      ? { ...f, colorLabel: color || undefined }
                      : f,
                  ),
                );
                // Update modal file
                setInfoModal((prev) =>
                  prev.file
                    ? {
                        ...prev,
                        file: { ...prev.file, colorLabel: color || undefined },
                      }
                    : prev,
                );
              }}
              onUpdateComments={(file, comments) => {
                // Update file comments
                setFiles((prev) =>
                  prev.map((f) => (f.id === file.id ? { ...f, comments } : f)),
                );
                // Update modal file
                setInfoModal((prev) =>
                  prev.file
                    ? { ...prev, file: { ...prev.file, comments } }
                    : prev,
                );
              }}
            />
          )}

          {/* Comment Modal */}
          {commentModal.visible && commentModal.file && (
            <CommentModal
              file={commentModal.file}
              sourceId={selectedSource?.id}
              onClose={() => setCommentModal({ visible: false, file: null })}
              onUpdateComments={(file, comments) => {
                // Update file comments in list
                setFiles((prev) =>
                  prev.map((f) => (f.id === file.id ? { ...f, comments } : f)),
                );
                // Update comment modal file
                setCommentModal((prev) =>
                  prev.file
                    ? { ...prev, file: { ...prev.file, comments } }
                    : prev,
                );
              }}
            />
          )}

          {/* Add Storage Modal */}
          {showAddStorage && (
            <AddStorageModal
              isOpen={showAddStorage}
              onClose={() => {
                console.log('[FinderPage] Closing Add Storage modal');
                setShowAddStorage(false);
                setEditingSource(null);
              }}
              onAdd={handleAddStorage}
              editingSource={editingSource}
            />
          )}

          {/* Spotlight Search */}
          <SpotlightSearch
            isOpen={spotlightOpen}
            onClose={handleCloseSpotlight}
            files={files}
            sources={sources}
            currentSourceId={selectedSource?.id}
            onNavigateToFile={(file) => {
              if (file.isDirectory) {
                navigateTo(file.path);
              } else {
                // Select the file
                setSelectedFiles(new Set([file.path]));
                // Open info modal
                setInfoModal({ visible: true, file });
              }
              handleCloseSpotlight();
            }}
            onNavigateToPath={(sourceId, path) => {
              const source = sources.find((s) => s.id === sourceId);
              if (source) {
                selectSource(source);
                navigateTo(path);
              }
              handleCloseSpotlight();
            }}
            onSearchSubmit={(query) => {
              setSearchQuery(query);
              handleCloseSpotlight();
            }}
          />

          {/* File Operation Progress */}
          {fileOperation && (
            <div
              className={`file-operation-toast ${!fileOperation.inProgress ? 'completed' : ''}`}
            >
              {fileOperation.inProgress ? (
                <div className="operation-spinner" />
              ) : (
                <svg
                  width="16"
                  height="16"
                  viewBox="0 0 16 16"
                  fill="var(--success, #34c759)"
                >
                  <path d="M13.854 3.646a.5.5 0 0 1 0 .708l-7 7a.5.5 0 0 1-.708 0l-3.5-3.5a.5.5 0 1 1 .708-.708L6.5 10.293l6.646-6.647a.5.5 0 0 1 .708 0z" />
                </svg>
              )}
              <span>
                {fileOperation.type}
                {fileOperation.inProgress ? '...' : ''}
              </span>
            </div>
          )}

          {/* Keyboard Shortcut Helper */}
          <KeyboardShortcutHelper
            isOpen={shortcutHelper.isOpen}
            onClose={shortcutHelper.close}
          />

          {/* Keyboard Shortcut Settings */}
          <ShortcutSettings
            isOpen={showShortcutSettings}
            onClose={() => setShowShortcutSettings(false)}
          />

          {/* Unified Operations Panel - Handles all operations including Copy, Cut (Move), Rename */}
          {/* Paste operations are tracked as Copy or Move depending on clipboard operation type */}
          <OperationsPanel
            operationTypes={[
              'Upload',
              'Download',
              'Delete',
              'Copy',
              'Move',
              'Rename',
            ]}
            onViewDetails={() => setActiveTab('transfers')}
          />
          <TranscriptionProgressPanel />
        </>
      )}

      {/* Operations Tab Content */}
      {activeTab === 'transfers' && (
        <div className="finder-transfers-view">
          <TransferPanel
            isVisible={true}
            filterSources={['network', 'cloud']}
            sources={sources}
          />
        </div>
      )}

      {/* Storage Tier Dialog */}
      {selectedSource && (
        <StorageTierDialog
          isOpen={showTierDialog}
          onClose={() => {
            setShowTierDialog(false);
            // Clear cross-storage drag state if dialog is cancelled
            if (crossStorageDrag) {
              setCrossStorageDrag(null);
              setDraggedFiles([]);
              setDraggedFileObjects([]);
              setDragSourceId(null);
              setDropTarget(null);
              setIsDraggingOver(false);
            }
          }}
          onConfirm={handleSyncToTier}
          sourceId={crossStorageDrag?.destSourceId || selectedSource.id}
          filePaths={tierDialogPaths}
          onAddProvider={() => {
            setShowTierDialog(false);
            setShowAddStorage(true);
            // Clear cross-storage drag state
            if (crossStorageDrag) {
              setCrossStorageDrag(null);
              setDraggedFiles([]);
              setDraggedFileObjects([]);
              setDragSourceId(null);
            }
          }}
        />
      )}
    </div>
  );
}

// Helper functions

// Get cyberpunk icon component based on folder name
function getLocationIcon(name: string) {
  const lowerName = name.toLowerCase();
  if (lowerName === 'home' || lowerName.includes('user')) return IconHome;
  if (lowerName === 'desktop') return IconDesktop;
  if (lowerName === 'documents' || lowerName === 'docs') return IconDocuments;
  if (lowerName === 'downloads') return IconDownloads;
  if (lowerName === 'pictures' || lowerName === 'photos') return IconPictures;
  if (lowerName === 'music' || lowerName === 'audio') return IconMusic;
  if (lowerName === 'volumes' || lowerName === 'drives') return IconVolumes;
  return IconFolder;
}

// Get storage icon based on category
function getStorageIcon(source: StorageSource) {
  switch (source.category) {
    case 'local':
      return getLocationIcon(source.name);
    case 'cloud':
      return IconCloud;
    case 'network':
      return IconNetwork;
    case 'hybrid':
      return IconDatabase;
    default:
      return IconFolder;
  }
}

/**
 * Get storage display label with naming conventions
 * Follows standard naming patterns:
 * - SMB/CIFS: \\server\share or //server/share
 * - NFS: server:/export
 * - S3: s3://bucket/prefix
 * - Cloud: provider://container
 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
function getStorageDisplayLabel(_source: StorageSource): string {
  const { providerId, name, config } = _source;

  // For named sources, just return the name
  if (name && !name.includes('/') && !name.includes('\\')) {
    return name;
  }

  // Format based on provider type
  switch (providerId) {
    case 'smb':
    case 'cifs': {
      const server = config?.server as string;
      const share = config?.share as string;
      if (server && share) {
        // Windows UNC format
        return `\\\\${server}\\${share}`;
      }
      return name;
    }
    case 'nfs': {
      const server = config?.server as string;
      const exportPath = config?.export as string;
      if (server && exportPath) {
        // NFS format: server:/export
        return `${server}:${exportPath}`;
      }
      return name;
    }
    case 'aws-s3':
    case 's3-compatible': {
      const bucket = config?.bucket as string;
      const prefix = config?.prefix as string;
      if (bucket) {
        // S3 URI format
        return prefix ? `s3://${bucket}/${prefix}` : `s3://${bucket}`;
      }
      return name;
    }
    case 'gcs': {
      const bucket = config?.bucket as string;
      if (bucket) {
        return `gs://${bucket}`;
      }
      return name;
    }
    case 'azure-blob': {
      const account = config?.accountName as string;
      const container = config?.container as string;
      if (account && container) {
        return `azure://${account}/${container}`;
      }
      return name;
    }
    case 'sftp': {
      const host = config?.host as string;
      const path = config?.remotePath as string;
      if (host) {
        return `sftp://${host}${path || '/'}`;
      }
      return name;
    }
    case 'webdav': {
      const url = config?.url as string;
      if (url) {
        return url.replace(/^https?:\/\//, 'dav://');
      }
      return name;
    }
    default:
      return name;
  }
}

// Unused helper functions - kept for potential future use
// function getFileIcon(file: FileMetadata, size = 48): React.ReactNode {
//   const isFolder =
//     file.isDirectory || file.mimeType === 'folder' || file.path.endsWith('/');
//
//   if (isFolder) {
//     // Use simple folder icon - cleaner at all sizes
//     return (
//       <IconFolder
//         size={size}
//         color="currentColor"
//         glow={false}
//         className="folder-icon"
//       />
//     );
//   }
//
//   // Use the helper function to get the appropriate icon component
//   // All file icons use currentColor to inherit from CSS variables
//   const IconComponent = getFileIconComponent(file.name, file.mimeType);
//   return <IconComponent size={size} color="currentColor" glow={false} />;
// }

// function formatDate(dateStr: string | undefined): string {
//   if (!dateStr || dateStr === '' || dateStr === '0') return '-';
//   try {
//     // Handle ISO 8601 format (YYYY-MM-DDTHH:MM:SS.sssZ) or legacy format (YYYY-MM-DD HH:MM:SS)
//     let date: Date;
//     if (dateStr.includes('T')) {
//       // ISO format
//       date = new Date(dateStr);
//     } else if (dateStr.match(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/)) {
//       // Legacy format: YYYY-MM-DD HH:MM:SS - convert to ISO
//       date = new Date(dateStr.replace(' ', 'T') + 'Z');
//     } else if (/^\d+$/.test(dateStr)) {
//       // Unix timestamp (seconds)
//       date = new Date(parseInt(dateStr, 10) * 1000);
//     } else {
//       // Try parsing as-is
//       date = new Date(dateStr);
//     }
//
//     if (isNaN(date.getTime())) return '-';
//
//     // Check if date is Unix epoch (1970-01-01) - treat as invalid
//     const epochTime = new Date('1970-01-01T00:00:00Z').getTime();
//     if (date.getTime() <= epochTime) return '-';
//
//     // Format as date and time
//     return date.toLocaleString(undefined, {
//       year: 'numeric',
//       month: 'short',
//       day: 'numeric',
//       hour: '2-digit',
//       minute: '2-digit',
//     });
//   } catch {
//     return '-';
//   }
// }

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
