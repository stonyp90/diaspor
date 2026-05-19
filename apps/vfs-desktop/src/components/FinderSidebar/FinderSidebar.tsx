/**
 * FinderSidebar Component
 *
 * Extracted sidebar panel from FinderPage for better maintainability.
 * Contains favorites, storage sources, and tags sections.
 */

import React, { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type {
  StorageSource,
  FileMetadata,
  GlobalFavorite,
} from '../../types/storage';
import { IconStar, IconFolder, IconTag, IconDatabase } from '../CyberpunkIcons';
import { MetricsPreview } from '../MetricsPreview';
import { getStorageIcon, isObjectStorage } from '../../pages/FinderPage/utils';
import './FinderSidebar.css';

export interface FinderSidebarProps {
  sidebarWidth: number;
  sources: StorageSource[];
  selectedSource: StorageSource | null;
  favorites: GlobalFavorite[];
  allTags: Array<{ name: string; color?: string }>;
  filterByTag: string | null;
  dropTarget: string | null;
  draggedFiles: string[];
  draggedFileObjects: FileMetadata[];
  dragSourceId: string | null;
  nativeFileDropPaths: string[];
  onSelectSource: (source: StorageSource, path?: string) => Promise<void>;
  onNavigateToFavorite: (favorite: GlobalFavorite) => void;
  onAddToFavorites: (file: FileMetadata, source: StorageSource) => void;
  onRemoveFromFavorites: (favoriteId: string) => void;
  onDropOnSource: (e: React.DragEvent, source: StorageSource) => Promise<void>;
  onSetDropTarget: (target: string | null) => void;
  onSetDraggedFiles: (files: string[]) => void;
  onSetDraggedFileObjects: (files: FileMetadata[]) => void;
  onSetDragSourceId: (id: string | null) => void;
  onSetNativeFileDropPaths: (paths: string[]) => void;
  onSetFilterByTag: (tag: string | null) => void;
  onSetStorageContextMenu: (
    menu: {
      source: StorageSource;
      x: number;
      y: number;
    } | null,
  ) => void;
  onSetShowAddStorage: (show: boolean) => void;
  onOpenMetrics?: () => void;
}

// Memoize the component to prevent re-renders when props haven't actually changed
const FinderSidebarMemo = React.memo(
  function FinderSidebar({
    sidebarWidth,
    sources,
    selectedSource,
    favorites,
    allTags,
    filterByTag,
    dropTarget,
    draggedFiles,
    draggedFileObjects,
    dragSourceId,
    nativeFileDropPaths,
    onSelectSource,
    onNavigateToFavorite,
    onAddToFavorites,
    onRemoveFromFavorites,
    onDropOnSource,
    onSetDropTarget,
    onSetDraggedFiles,
    onSetDraggedFileObjects,
    onSetDragSourceId,
    onSetNativeFileDropPaths,
    onSetFilterByTag,
    onSetStorageContextMenu,
    onSetShowAddStorage,
    onOpenMetrics,
  }: FinderSidebarProps) {
    // State for collapsed groups
    const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
      new Set(),
    );

    // Refs for storage items to prevent unnecessary scrolling
    const itemRefs = useRef<Map<string, HTMLButtonElement>>(new Map());
    const lastSelectedSourceId = useRef<string | null>(null);
    const sidebarRef = useRef<HTMLElement | null>(null);

    // Track navigation state to disable transitions during rapid navigation
    const isNavigatingRef = useRef(false);
    const navigationTimeoutRef = useRef<number | null>(null);

    const toggleGroup = (groupName: string) => {
      setCollapsedGroups((prev) => {
        const next = new Set(prev);
        if (next.has(groupName)) {
          next.delete(groupName);
        } else {
          next.add(groupName);
        }
        return next;
      });
    };

    // Scroll selected item into view only when selection actually changes
    // Disabled smooth scrolling to prevent visual flickering while browsing
    useEffect(() => {
      const selectedId = selectedSource?.id;
      // Only scroll if the selection actually changed (not on every render)
      if (selectedId && selectedId !== lastSelectedSourceId.current) {
        lastSelectedSourceId.current = selectedId;

        // Mark as navigating to disable transitions temporarily
        isNavigatingRef.current = true;
        // Use ref if available, otherwise fallback to querySelector
        const sidebar =
          sidebarRef.current || document.querySelector('.finder-sidebar');
        if (sidebar) {
          sidebar.classList.add('navigating');
        }

        // Clear any existing timeout
        if (navigationTimeoutRef.current !== null) {
          clearTimeout(navigationTimeoutRef.current);
        }

        // Re-enable transitions after navigation completes
        navigationTimeoutRef.current = window.setTimeout(() => {
          isNavigatingRef.current = false;
          const sidebarToUpdate =
            sidebarRef.current || document.querySelector('.finder-sidebar');
          if (sidebarToUpdate) {
            sidebarToUpdate.classList.remove('navigating');
          }
          navigationTimeoutRef.current = null;
        }, 200); // Longer timeout to ensure React has finished re-rendering

        const element = itemRefs.current.get(selectedId);
        if (element) {
          // Use instant scroll instead of smooth to prevent visual effects
          // Only scroll if element is not already visible
          const rect = element.getBoundingClientRect();
          if (sidebar) {
            const sidebarRect = sidebar.getBoundingClientRect();
            const isVisible =
              rect.top >= sidebarRect.top && rect.bottom <= sidebarRect.bottom;

            if (!isVisible) {
              // Use instant scroll (no smooth behavior) to prevent flickering
              element.scrollIntoView({
                behavior: 'auto',
                block: 'nearest',
                inline: 'nearest',
              });
            }
          }
        }
      }

      // Cleanup timeout on unmount
      return () => {
        if (navigationTimeoutRef.current !== null) {
          clearTimeout(navigationTimeoutRef.current);
          navigationTimeoutRef.current = null;
        }
      };
    }, [selectedSource?.id]);

    const renderStorageItem = (source: StorageSource) => {
      const StorageIcon = getStorageIcon(source);
      const isDropTarget = dropTarget === `source:${source.id}`;
      const isSelected = selectedSource?.id === source.id;

      // Determine tier class
      let tierClass: string;
      if (source.category === 'local') {
        tierClass = 'local';
      } else if (source.category === 'cloud') {
        tierClass = source.tierStatus === 'cold' ? 'cold' : 'nearline';
      } else {
        tierClass = source.tierStatus || 'hot';
      }

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

      // Ref callback to store element reference (doesn't trigger scrolling)
      const itemRefCallback = (element: HTMLButtonElement | null) => {
        if (element) {
          itemRefs.current.set(source.id, element);
        } else {
          itemRefs.current.delete(source.id);
        }
      };

      // Compute className efficiently - use stable computation to prevent unnecessary transitions
      // Build className from parts to ensure consistent string creation
      const classNameParts = ['sidebar-item', 'storage-item'];
      if (isSelected) classNameParts.push('active');
      if (isDropTarget) classNameParts.push('drop-target');
      const className = classNameParts.join(' ');

      return (
        <button
          ref={itemRefCallback}
          key={source.id}
          data-source-id={source.id}
          className={className}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            // Always select the source when clicked, even if already selected
            // This ensures the selection/focus remains on the clicked item
            onSelectSource(source);
          }}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onSetStorageContextMenu({
              source,
              x: e.clientX,
              y: e.clientY,
            });
          }}
          onDragOver={(e) => {
            e.preventDefault();
            if (dragSourceId !== source.id) {
              onSetDropTarget(`source:${source.id}`);
            }
          }}
          onDragLeave={() => onSetDropTarget(null)}
          onDrop={(e) => onDropOnSource(e, source)}
        >
          <span className="item-icon">
            <StorageIcon size={16} />
          </span>
          <span className="item-name" title={source.name}>
            <span>{source.name}</span>
            {source.category === 'cloud' &&
              isObjectStorage(source) &&
              source.config &&
              'bucket' in source.config &&
              typeof source.config.bucket === 'string' && (
                <span className="item-subtitle">({source.config.bucket})</span>
              )}
          </span>
          <span className="storage-badges">
            {/* Only show badge for cloud storage (N for Nearline, C for Cold) */}
            {source.category === 'cloud' && (
              <span
                className={`storage-tier-badge ${tierClass}`}
                title={`${tierClass === 'nearline' ? 'Nearline' : tierClass === 'cold' ? 'Cold' : 'Cloud'} Tier - ${getTypeLabel(source.category)}`}
              >
                {tierClass === 'nearline'
                  ? 'N'
                  : tierClass === 'cold'
                    ? 'C'
                    : 'N'}
              </span>
            )}
          </span>
          {source.status !== 'connected' && (
            <span className="offline-dot" title="Disconnected" />
          )}
        </button>
      );
    };

    const handleFavoritesDrop = async (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      onSetDropTarget(null);

      console.log('[Favorites] Drop event triggered');
      console.log('[Favorites] Drag state:', {
        draggedFiles,
        draggedFileObjects: draggedFileObjects.length,
        dragSourceId,
      });

      let dropSourceId: string | null = null;
      let pathsToAdd: string[] = [];
      let fileObjectsToAdd: FileMetadata[] = [];

      // 1. Check for native file drops
      const nativeFiles = e.dataTransfer.files;
      let filePaths: string[] = [];
      if (nativeFileDropPaths.length > 0) {
        filePaths = [...nativeFileDropPaths];
        onSetNativeFileDropPaths([]);
      }

      if (nativeFiles && nativeFiles.length > 0 && filePaths.length === 0) {
        const fileArray = Array.from(nativeFiles);
        const dataTransferItems = e.dataTransfer.items;

        if (dataTransferItems) {
          for (let i = 0; i < dataTransferItems.length; i++) {
            const item = dataTransferItems[i];
            if (item.kind === 'file') {
              const entry = item.webkitGetAsEntry();
              if (entry) {
                const entryWithPath = entry as unknown as Record<
                  string,
                  unknown
                >;
                const fullPath =
                  (entryWithPath.fullPath as string | undefined) ||
                  (entryWithPath.path as string | undefined) ||
                  entry.name;
                filePaths.push(fullPath);
              } else {
                const file = fileArray[i];
                const fileWithPath = file as unknown as Record<string, unknown>;
                const path =
                  (fileWithPath.path as string | undefined) || file.name;
                filePaths.push(path);
              }
            }
          }
        } else {
          filePaths.push(
            ...fileArray.map((f) => {
              const fileWithPath = f as unknown as Record<string, unknown>;
              return (fileWithPath.path as string | undefined) || f.name;
            }),
          );
        }
      }

      if (filePaths.length > 0) {
        let targetSource = sources.find((s) => s.category === 'local');
        if (!targetSource) {
          targetSource = sources.find((s) => s.category === 'network');
        }
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
          const fileArray = nativeFiles ? Array.from(nativeFiles) : [];
          for (let i = 0; i < filePaths.length; i++) {
            const filePath = filePaths[i];
            const nativeFile = fileArray[i];
            const fileName = nativeFile
              ? nativeFile.name
              : filePath.split('/').pop() || filePath;

            let normalizedPath = filePath;
            if (!normalizedPath.startsWith('/')) {
              normalizedPath = `/${normalizedPath}`;
            }

            let isDir = false;
            try {
              if (targetSource.category === 'local') {
                isDir = await invoke<boolean>('vfs_is_directory', {
                  path: normalizedPath,
                });
              } else if (targetSource.category === 'network') {
                try {
                  await invoke<FileMetadata[]>('vfs_list_files', {
                    sourceId: targetSource.id,
                    path: normalizedPath,
                  });
                  isDir = true;
                } catch {
                  isDir =
                    normalizedPath.endsWith('/') ||
                    (nativeFile && nativeFile.type === '') ||
                    !normalizedPath.includes('.');
                }
              } else {
                isDir =
                  normalizedPath.endsWith('/') ||
                  (nativeFile && nativeFile.type === '') ||
                  !normalizedPath.includes('.');
              }
            } catch (err) {
              isDir =
                normalizedPath.endsWith('/') ||
                (nativeFile && nativeFile.type === '') ||
                !normalizedPath.includes('.');
            }

            if (isDir && !normalizedPath.endsWith('/')) {
              normalizedPath = `${normalizedPath}/`;
            }

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
                ? new Date(nativeFile.lastModified).toISOString()
                : new Date().toISOString(),
              tierStatus:
                targetSource.category === 'cloud' ? 'nearline' : 'hot',
              canWarm: targetSource.category === 'cloud',
              canTranscode: false,
            };

            fileObjectsToAdd.push(fileMetadata);
            pathsToAdd.push(normalizedPath);
          }
        }
      }

      // 2. Check for VFS drag data
      if (fileObjectsToAdd.length === 0) {
        const vfsData = e.dataTransfer.getData('application/x-vfs-files');

        if (vfsData) {
          try {
            const parsed = JSON.parse(vfsData) as {
              sourceId: string;
              paths: string[];
            };
            dropSourceId = parsed.sourceId;
            pathsToAdd = parsed.paths;
          } catch (err) {
            console.error('Failed to parse VFS drag data:', err);
          }
        }

        // 3. Fallback to text/plain data
        if (pathsToAdd.length === 0) {
          const textData = e.dataTransfer.getData('text/plain');
          if (textData) {
            pathsToAdd = textData.split('\n').filter(Boolean);
          }
        }

        // 4. Fallback to state
        if (!dropSourceId) {
          dropSourceId = dragSourceId || selectedSource?.id || null;
        }
        if (pathsToAdd.length === 0) {
          pathsToAdd = draggedFiles.length > 0 ? draggedFiles : [];
        }

        // 5. Use draggedFileObjects if available
        if (draggedFileObjects.length > 0) {
          fileObjectsToAdd = draggedFileObjects;
        }
      }

      // Find the source
      const dropSource =
        (dropSourceId ? sources.find((s) => s.id === dropSourceId) : null) ||
        selectedSource;

      if (!dropSource) {
        console.error('No source found for favorites drop');
        onSetDraggedFiles([]);
        onSetDraggedFileObjects([]);
        onSetDragSourceId(null);
        return;
      }

      // Add files/folders to favorites
      if (fileObjectsToAdd.length > 0) {
        for (const file of fileObjectsToAdd) {
          const favoriteId = `${dropSource.id}:${file.path}`;
          if (!favorites.some((f) => f.id === favoriteId)) {
            onAddToFavorites(file, dropSource);
          }
        }
      } else if (pathsToAdd.length > 0) {
        // Fallback: create file objects from paths
        for (let filePath of pathsToAdd) {
          // Try to find file in current files list first
          // Note: This would need access to files list, but for now we'll create minimal metadata
          const fileName =
            filePath.split('/').filter(Boolean).pop() || filePath;
          const isDir = filePath.endsWith('/') || !filePath.includes('.');
          if (isDir && !filePath.endsWith('/')) {
            filePath = `${filePath}/`;
          }

          const file: FileMetadata = {
            id: filePath,
            name: fileName,
            path: filePath,
            size: 0,
            mimeType: isDir ? 'folder' : 'application/octet-stream',
            isDirectory: isDir,
            lastModified: new Date().toISOString(),
            tierStatus: dropSource.category === 'cloud' ? 'nearline' : 'hot',
            canWarm: dropSource.category === 'cloud',
            canTranscode: false,
          };

          const favoriteId = `${dropSource.id}:${file.path}`;
          if (!favorites.some((f) => f.id === favoriteId)) {
            onAddToFavorites(file, dropSource);
          }
        }
      }

      onSetDraggedFiles([]);
      onSetDraggedFileObjects([]);
      onSetDragSourceId(null);
    };

    return (
      <aside
        ref={(el) => {
          sidebarRef.current = el;
        }}
        className="finder-sidebar"
        style={{ width: `${sidebarWidth}px` }}
      >
        {/* Favorites Section */}
        <div
          className={`sidebar-section favorites-section ${dropTarget === 'favorites' ? 'drop-target' : ''}`}
          onDragOver={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onSetDropTarget('favorites');
            e.dataTransfer.dropEffect = 'link';
          }}
          onDragLeave={(e) => {
            e.preventDefault();
            if (!e.currentTarget.contains(e.relatedTarget as Node)) {
              onSetDropTarget(null);
            }
          }}
          onDrop={handleFavoritesDrop}
        >
          <div className="section-header">
            <IconStar size={14} glow={false} />
            <span>Favorites</span>
            {favorites.length > 0 && (
              <span className="section-count">({favorites.length})</span>
            )}
          </div>
          {favorites.length === 0 ? (
            <div className="sidebar-empty">
              <span className="empty-text">Drop files here</span>
              <span className="empty-hint">Drag to add favorites</span>
            </div>
          ) : (
            favorites.slice(0, 10).map((fav) => (
              <button
                key={fav.id}
                className="sidebar-item"
                onClick={() => onNavigateToFavorite(fav)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  onRemoveFromFavorites(fav.id);
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
              <span className="item-icon item-icon-text">+</span>
              <span>{favorites.length - 10} more</span>
            </div>
          )}
        </div>

        {/* Storage Section */}
        <div className="sidebar-section storage-section">
          <div className="section-header">
            <IconDatabase size={14} glow={false} />
            <span>Storage</span>
            <span className="section-count">({sources.length})</span>
          </div>

          {sources.length === 0 && (
            <div className="sidebar-empty">
              <span className="empty-text">No storage connected</span>
              <button
                className="add-storage-btn"
                onClick={() => onSetShowAddStorage(true)}
              >
                <span className="add-icon">+</span>
                <span>Add Storage</span>
              </button>
            </div>
          )}

          {/* Deduplicate sources by ID to prevent multiple occurrences */}
          {(() => {
            const uniqueSources = Array.from(
              new Map(sources.map((s) => [s.id, s])).values(),
            );

            // Separate local storage sources into locations, cloud drives, and volumes
            // This logic is OS-agnostic - relies on flags set by the backend
            const localSources = uniqueSources.filter(
              (s) => s.category === 'local',
            );

            // Locations: ONLY default system locations (Home, Desktop, Documents, Downloads, etc.)
            // Backend marks these as system locations by name matching:
            //   - macOS/Linux/Windows: "Home", "Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos"
            //   - These are standard user folders, consistent across all platforms
            //   - Windows drive letters (C:\, D:\, etc.) are NOT marked as system locations
            const locations = localSources.filter(
              (s) => s.isSystemLocation === true,
            );

            // Volumes: temporary mounted volumes and external drives
            // These are NOT system locations:
            //   - macOS: Mounted volumes from /Volumes (DMGs, external drives, etc.)
            //   - Linux: Mounted volumes from /media/{user} or /mnt (USB drives, etc.)
            //   - Windows: Drive letters (C:\, D:\, etc.) - all drives except system folders
            // Cloud drives (iCloud, Google Drive, etc.) are automatically accessible
            // via their local mount points as regular folders
            const volumes = localSources.filter(
              (s) => s.isSystemLocation !== true,
            );
            // Other storage types (cloud object storage, network, etc.)
            const otherSources = uniqueSources.filter(
              (s) => s.category !== 'local',
            );

            return (
              <>
                {/* Local Storage Group */}
                {localSources.length > 0 &&
                  (() => {
                    const isCollapsed = collapsedGroups.has('local');
                    const groupClassName = isCollapsed
                      ? 'storage-group collapsed'
                      : 'storage-group';

                    return (
                      <div className={groupClassName}>
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
                          {/* Locations Subgroup */}
                          {locations.length > 0 && (
                            <div
                              className={
                                collapsedGroups.has('locations')
                                  ? 'storage-subgroup collapsed'
                                  : 'storage-subgroup'
                              }
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
                                {locations.map((source) =>
                                  renderStorageItem(source),
                                )}
                              </div>
                            </div>
                          )}

                          {/* Volumes Subgroup */}
                          {volumes.length > 0 && (
                            <div
                              className={
                                collapsedGroups.has('volumes')
                                  ? 'storage-subgroup collapsed'
                                  : 'storage-subgroup'
                              }
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
                                {volumes.map((source) =>
                                  renderStorageItem(source),
                                )}
                              </div>
                            </div>
                          )}
                        </div>
                      </div>
                    );
                  })()}

                {/* Other Storage Types (Cloud, Network, etc.) */}
                {otherSources.length > 0 && (
                  <div className="storage-group-items">
                    {otherSources.map((source) => renderStorageItem(source))}
                  </div>
                )}
              </>
            );
          })()}
        </div>

        {/* Add Storage Button */}
        <div className="sidebar-section">
          <button
            className="add-storage-btn"
            onClick={() => {
              console.log('[FinderSidebar] Add Storage button clicked');
              onSetShowAddStorage(true);
            }}
            title="Add Storage"
          >
            <span className="add-icon">+</span>
            <span>Add Storage</span>
          </button>
        </div>

        {/* Tags Section */}
        <div className="sidebar-section storage-section">
          <div className="section-header">
            <IconTag size={14} glow={false} />
            <span>Tags</span>
            {allTags.length > 0 && (
              <span className="section-count">({allTags.length})</span>
            )}
          </div>
          {filterByTag && (
            <div className="storage-group-items">
              <button
                className="sidebar-item storage-item active filter-active"
                onClick={() => onSetFilterByTag(null)}
              >
                <span className="item-icon">
                  <span
                    className="tag-dot"
                    style={{
                      background:
                        allTags.find((t) => t.name === filterByTag)?.color ||
                        'var(--vfs-primary)',
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
                    onClick={() => onSetFilterByTag(tag.name)}
                  >
                    <span className="item-icon">
                      <span
                        className="tag-dot"
                        style={{
                          background: tag.color || 'var(--vfs-primary)',
                        }}
                      />
                    </span>
                    <span className="item-name">{tag.name}</span>
                  </button>
                ))}
            </div>
          )}
        </div>

        {/* Metrics Preview */}
        {onOpenMetrics && <MetricsPreview onOpenMetrics={onOpenMetrics} />}
      </aside>
    );
  },
  (prevProps, nextProps) => {
    // Custom comparison to prevent re-renders when sources array reference changes but content is the same
    // This prevents glitches during rapid file operations

    // Compare sources by ID and length (not reference)
    const prevSourceIds = prevProps.sources
      .map((s) => s.id)
      .sort()
      .join(',');
    const nextSourceIds = nextProps.sources
      .map((s) => s.id)
      .sort()
      .join(',');
    const sourcesChanged =
      prevSourceIds !== nextSourceIds ||
      prevProps.sources.length !== nextProps.sources.length;

    // Compare other props
    const selectedChanged =
      prevProps.selectedSource?.id !== nextProps.selectedSource?.id;
    const favoritesChanged =
      prevProps.favorites.length !== nextProps.favorites.length ||
      JSON.stringify(prevProps.favorites.map((f) => f.id).sort()) !==
        JSON.stringify(nextProps.favorites.map((f) => f.id).sort());
    const tagsChanged =
      prevProps.allTags.length !== nextProps.allTags.length ||
      JSON.stringify(prevProps.allTags.map((t) => t.name).sort()) !==
        JSON.stringify(nextProps.allTags.map((t) => t.name).sort());
    const filterChanged = prevProps.filterByTag !== nextProps.filterByTag;
    const dropTargetChanged = prevProps.dropTarget !== nextProps.dropTarget;
    const sidebarWidthChanged =
      prevProps.sidebarWidth !== nextProps.sidebarWidth;

    // Only re-render if something actually changed (ignore callback changes - they're stable)
    const shouldSkipRender =
      !sourcesChanged &&
      !selectedChanged &&
      !favoritesChanged &&
      !tagsChanged &&
      !filterChanged &&
      !dropTargetChanged &&
      !sidebarWidthChanged;
    return shouldSkipRender;
  },
);

export const FinderSidebar = FinderSidebarMemo;
