/**
 * FileBrowser Component
 *
 * Main file browser panel that displays files in grid or list view.
 */

import React from 'react';
import { Select } from '../Select';
import { IconFolder } from '../CyberpunkIcons';
import type { FileMetadata, StorageCategory } from '../../types/storage';
import { getCategoryName } from '../../types/storage';
import type {
  ViewMode,
  ColumnWidths,
  ColumnFilters,
  SortColumn,
  SortDirection,
} from '../../pages/FinderPage/types';
import { FileGridItem } from '../FileGridItem';
import { FileListRow } from '../FileListRow';
import './FileBrowser.css';

interface FileBrowserProps {
  loading: boolean;
  files: FileMetadata[];
  filteredFiles: FileMetadata[];
  viewMode: ViewMode;
  hasMore?: boolean;
  isLoadingMore?: boolean;
  onLoadMore?: () => void;
  selectedFiles: Set<string>;
  dropTarget: string | null;
  draggedFiles: string[];
  cutFiles: Set<string>;
  isDraggingOver: boolean;
  currentPath: string;
  columnWidths: ColumnWidths | null;
  columnFilters: ColumnFilters;
  resizingColumn: string | null;
  renamingFile: string | null;
  renameValue: string;
  allTags: Array<{ name: string; color?: string }>;
  sortColumn: SortColumn;
  sortDirection: SortDirection;
  sourceCategory?:
    | 'local'
    | 'cloud'
    | 'block'
    | 'network'
    | 'hybrid'
    | 'custom';
  iconViewRef: React.RefObject<HTMLDivElement>;
  listViewRef: React.RefObject<HTMLDivElement>;
  renameInputRef: React.RefObject<HTMLInputElement>;
  onFileClick: (file: FileMetadata, e: React.MouseEvent) => void;
  onFileDoubleClick: (file: FileMetadata) => void;
  onContextMenu: (e: React.MouseEvent, file?: FileMetadata) => void;
  onDragOver: (e: React.DragEvent) => void;
  onDragLeave: () => void;
  onDrop: (e: React.DragEvent, path: string) => void;
  onDragStart: (e: React.DragEvent, file: FileMetadata) => void;
  onDragEnd: () => void;
  onDragOverFile: (e: React.DragEvent, path: string, isFolder: boolean) => void;
  onSetColumnFilters: (
    filters: ColumnFilters | ((prev: ColumnFilters) => ColumnFilters),
  ) => void;
  onSetColumnWidths: (
    widths:
      | ColumnWidths
      | null
      | ((prev: ColumnWidths | null) => ColumnWidths | null),
  ) => void;
  onHandleColumnResizeStart: (e: React.MouseEvent, column: string) => void;
  onSortChange: (column: SortColumn) => void;
  onSetSelectedFiles: (files: Set<string>) => void;
  onSetRenameValue: (value: string) => void;
  onHandleRenameKeyDown: (e: React.KeyboardEvent) => void;
  onCommitRename: () => void;
  onSetCommentModal: (modal: { visible: boolean; file: FileMetadata }) => void;
}

export function FileBrowser({
  loading,
  files,
  filteredFiles,
  viewMode,
  selectedFiles,
  dropTarget,
  draggedFiles,
  cutFiles,
  isDraggingOver,
  currentPath,
  columnWidths,
  columnFilters,
  resizingColumn,
  renamingFile,
  renameValue,
  allTags,
  sortColumn,
  sortDirection,
  sourceCategory,
  iconViewRef,
  listViewRef,
  renameInputRef,
  hasMore = false,
  isLoadingMore = false,
  onLoadMore,
  onFileClick,
  onFileDoubleClick,
  onContextMenu,
  onDragOver,
  onDragLeave,
  onDrop,
  onDragStart,
  onDragEnd,
  onDragOverFile,
  onSetColumnFilters,
  onHandleColumnResizeStart,
  onSortChange,
  onSetSelectedFiles,
  onSetRenameValue,
  onHandleRenameKeyDown,
  onCommitRename,
  onSetCommentModal,
}: FileBrowserProps) {
  // Track whether the active view's content actually overflows the viewport;
  // the "All items loaded" footer should only appear when the user has
  // something to scroll past. Without this, short lists (e.g. 10 files in a
  // tall window) render a redundant end-of-list indicator with no scroll.
  const [isOverflowing, setIsOverflowing] = React.useState(false);
  React.useLayoutEffect(() => {
    const container =
      viewMode === 'icon'
        ? iconViewRef.current
        : viewMode === 'list'
          ? listViewRef.current
          : null;
    if (!container) {
      setIsOverflowing(false);
      return;
    }
    const update = () =>
      setIsOverflowing(container.scrollHeight > container.clientHeight + 1);
    update();
    const ro = new ResizeObserver(update);
    ro.observe(container);
    if (container.firstElementChild) ro.observe(container.firstElementChild);
    return () => ro.disconnect();
  }, [viewMode, files.length, filteredFiles.length, iconViewRef, listViewRef]);

  // Handle scroll to load more items
  const handleScroll = React.useCallback(
    (e: React.UIEvent<HTMLDivElement>) => {
      const target = e.currentTarget;
      const scrollBottom =
        target.scrollHeight - target.scrollTop - target.clientHeight;

      // Load more when within 200px of bottom
      if (
        hasMore &&
        !isLoadingMore &&
        !loading &&
        scrollBottom < 200 &&
        onLoadMore
      ) {
        onLoadMore();
      }
    },
    [hasMore, isLoadingMore, loading, onLoadMore],
  );
  if (loading) {
    return (
      <div className="empty-state">
        <div className="spinner" />
        <span className="empty-state-text">Loading...</span>
      </div>
    );
  }

  if (files.length === 0) {
    return (
      <div className="empty-state">
        <IconFolder
          size={48}
          color="var(--finder-text-quaternary)"
          glow={false}
        />
        <span className="empty-state-text">No files</span>
        <span className="empty-state-hint">Right-click to create or paste</span>
      </div>
    );
  }

  return (
    <>
      {viewMode === 'icon' && (
        <div
          ref={iconViewRef}
          className={`icon-view ${isDraggingOver && dropTarget === null ? 'drop-zone-active' : ''}`}
          onContextMenu={onContextMenu}
          onDragOver={onDragOver}
          onDragLeave={onDragLeave}
          onDrop={(e) => onDrop(e, currentPath)}
          onScroll={handleScroll}
        >
          {filteredFiles.map((file, index) => {
            const isFolder =
              file.isDirectory ||
              file.mimeType === 'folder' ||
              file.path.endsWith('/');
            const isDropTarget = isFolder && dropTarget === file.path;
            const isDragging = draggedFiles.includes(file.path);
            const fileIsHidden = file.isHidden ?? file.name.startsWith('.');
            const isCut = cutFiles.has(file.path);

            return (
              <FileGridItem
                key={file.path}
                file={file}
                index={index}
                isSelected={selectedFiles.has(file.path)}
                isFolder={isFolder}
                isDropTarget={isDropTarget}
                isDragging={isDragging}
                isHidden={fileIsHidden}
                isCut={isCut}
                renamingFile={renamingFile}
                renameValue={renameValue}
                allTags={allTags}
                sourceCategory={sourceCategory}
                onFileClick={onFileClick}
                onFileDoubleClick={onFileDoubleClick}
                onContextMenu={onContextMenu}
                onDragStart={onDragStart}
                onDragEnd={onDragEnd}
                onDragOver={onDragOverFile}
                onDragLeave={onDragLeave}
                onDrop={onDrop}
                onKeyDown={(e, file, index) => {
                  if (e.key === 'Enter') onFileDoubleClick(file);
                  if (
                    (e.shiftKey && e.key === 'F10') ||
                    e.key === 'ContextMenu'
                  ) {
                    e.preventDefault();
                    e.stopPropagation();
                    const rect = e.currentTarget.getBoundingClientRect();
                    const syntheticEvent = {
                      clientX: rect.left + rect.width / 2,
                      clientY: rect.top + rect.height / 2,
                      preventDefault: () => {
                        // Prevent default behavior
                      },
                      stopPropagation: () => {
                        // Stop event propagation
                      },
                    } as React.MouseEvent;
                    onContextMenu(syntheticEvent, file);
                  }
                  if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
                    e.preventDefault();
                    const nextEl = e.currentTarget
                      .nextElementSibling as HTMLElement;
                    if (nextEl) {
                      nextEl.focus();
                      const next = filteredFiles[index + 1];
                      if (next) onSetSelectedFiles(new Set([next.path]));
                    }
                  }
                  if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
                    e.preventDefault();
                    const prevEl = e.currentTarget
                      .previousElementSibling as HTMLElement;
                    if (prevEl) {
                      prevEl.focus();
                      const prev = filteredFiles[index - 1];
                      if (prev) onSetSelectedFiles(new Set([prev.path]));
                    }
                  }
                }}
                onRenameChange={onSetRenameValue}
                onRenameKeyDown={onHandleRenameKeyDown}
                onRenameBlur={() => {
                  // Use setTimeout to allow click events to process first
                  // This prevents the blur from canceling the rename when clicking buttons
                  setTimeout(() => {
                    onCommitRename();
                  }, 100);
                }}
                onCommentClick={(file) =>
                  onSetCommentModal({ visible: true, file })
                }
                renameInputRef={renameInputRef}
              />
            );
          })}
          {/* Loading indicator for pagination */}
          {isLoadingMore && (
            <div
              style={{
                padding: '16px',
                textAlign: 'center',
                color: 'var(--finder-text-quaternary)',
                gridColumn: '1 / -1',
              }}
            >
              <div
                className="spinner"
                style={{
                  width: '20px',
                  height: '20px',
                  margin: '0 auto',
                  borderWidth: '2px',
                }}
              />
              <span
                style={{ fontSize: '12px', marginTop: '8px', display: 'block' }}
              >
                Loading more...
              </span>
            </div>
          )}
          {/* End of list indicator */}
          {!hasMore && files.length > 0 && (
            <div
              className="pagination-end"
              style={{
                padding: '16px',
                textAlign: 'center',
                color: 'var(--finder-text-quaternary)',
                fontSize: '12px',
                gridColumn: '1 / -1',
                flexShrink: 0,
              }}
            >
              All items loaded ({files.length}{' '}
              {files.length === 1 ? 'item' : 'items'})
            </div>
          )}
        </div>
      )}

      {viewMode === 'list' && (
        <div
          ref={listViewRef}
          className={`list-view ${isDraggingOver && dropTarget === null ? 'drop-zone-active' : ''}`}
          onContextMenu={onContextMenu}
          onDragOver={onDragOver}
          onDragLeave={onDragLeave}
          onDrop={(e) => onDrop(e, currentPath)}
          onScroll={handleScroll}
        >
          <div
            className="list-header"
            style={
              columnWidths
                ? {
                    gridTemplateColumns: `${columnWidths.name}px ${columnWidths.modified}px ${columnWidths.size}px 0px ${columnWidths['storage-class'] || 140}px`,
                  }
                : undefined
            }
          >
            <div className="col-name">
              <div className="col-header-content">
                <button
                  className={`col-sort-btn ${sortColumn === 'name' ? 'active' : ''}`}
                  onClick={() => onSortChange('name')}
                  title="Sort by name"
                >
                  <span>Name</span>
                  {sortColumn === 'name' && (
                    <span className="sort-indicator">
                      {sortDirection === 'asc' ? '↑' : '↓'}
                    </span>
                  )}
                </button>
                <input
                  type="text"
                  className="col-filter-input"
                  placeholder="Filter..."
                  value={columnFilters.name}
                  onChange={(e) =>
                    onSetColumnFilters((prev) => ({
                      ...prev,
                      name: e.target.value,
                    }))
                  }
                  onClick={(e) => e.stopPropagation()}
                  onKeyDown={(e) => e.stopPropagation()}
                />
              </div>
              <div
                className={`col-resizer ${resizingColumn === 'name' ? 'resizing' : ''}`}
                onMouseDown={(e) => onHandleColumnResizeStart(e, 'name')}
                title="Drag to resize column"
              />
            </div>
            <div className="col-date">
              <div className="col-header-content">
                <button
                  className={`col-sort-btn ${sortColumn === 'modified' ? 'active' : ''}`}
                  onClick={() => onSortChange('modified')}
                  title="Sort by date modified"
                >
                  <span>Date Modified</span>
                  {sortColumn === 'modified' && (
                    <span className="sort-indicator">
                      {sortDirection === 'asc' ? '↑' : '↓'}
                    </span>
                  )}
                </button>
                <Select
                  value={columnFilters.date}
                  onChange={(value) =>
                    onSetColumnFilters((prev) => ({
                      ...prev,
                      date: value,
                    }))
                  }
                  options={[
                    { value: '', label: 'All' },
                    { value: 'today', label: 'Today' },
                    { value: 'yesterday', label: 'Yesterday' },
                    { value: 'week', label: 'This Week' },
                    { value: 'month', label: 'This Month' },
                    { value: 'year', label: 'This Year' },
                  ]}
                  className="compact"
                  onClick={(e) => e.stopPropagation()}
                />
              </div>
              <div
                className={`col-resizer ${resizingColumn === 'modified' ? 'resizing' : ''}`}
                onMouseDown={(e) => onHandleColumnResizeStart(e, 'modified')}
                title="Drag to resize column"
              />
            </div>
            <div className="col-size">
              <div className="col-header-content">
                <button
                  className={`col-sort-btn ${sortColumn === 'size' ? 'active' : ''}`}
                  onClick={() => onSortChange('size')}
                  title="Sort by size"
                >
                  <span>Size</span>
                  {sortColumn === 'size' && (
                    <span className="sort-indicator">
                      {sortDirection === 'asc' ? '↑' : '↓'}
                    </span>
                  )}
                </button>
                <input
                  type="text"
                  className="col-filter-input"
                  placeholder="e.g., >10mb"
                  value={columnFilters.size}
                  onChange={(e) =>
                    onSetColumnFilters((prev) => ({
                      ...prev,
                      size: e.target.value,
                    }))
                  }
                  onClick={(e) => e.stopPropagation()}
                  onKeyDown={(e) => e.stopPropagation()}
                />
              </div>
              <div
                className={`col-resizer ${resizingColumn === 'size' ? 'resizing' : ''}`}
                onMouseDown={(e) => onHandleColumnResizeStart(e, 'size')}
                title="Drag to resize column"
              />
            </div>
            <div className="col-storage-class">
              <div className="col-header-content">
                <button
                  className={`col-sort-btn ${sortColumn === 'storage-class' ? 'active' : ''}`}
                  onClick={() => onSortChange('storage-class')}
                  title="Sort by storage class"
                >
                  <span>Storage Class</span>
                  {sortColumn === 'storage-class' && (
                    <span className="sort-indicator">
                      {sortDirection === 'asc' ? '↑' : '↓'}
                    </span>
                  )}
                </button>
              </div>
              <div
                className={`col-resizer ${resizingColumn === 'storage-class' ? 'resizing' : ''}`}
                onMouseDown={(e) =>
                  onHandleColumnResizeStart(e, 'storage-class')
                }
                title="Drag to resize column"
              />
            </div>
          </div>
          <div className="list-body">
            {filteredFiles.map((file, index) => {
              const isFolder =
                file.isDirectory ||
                file.mimeType === 'folder' ||
                file.path.endsWith('/');
              const isDropTarget = isFolder && dropTarget === file.path;
              const isDragging = draggedFiles.includes(file.path);
              const fileIsHidden = file.isHidden ?? file.name.startsWith('.');
              const isCut = cutFiles.has(file.path);

              return (
                <FileListRow
                  key={file.path}
                  file={file}
                  index={index}
                  isSelected={selectedFiles.has(file.path)}
                  isFolder={isFolder}
                  isDropTarget={isDropTarget}
                  isDragging={isDragging}
                  isHidden={fileIsHidden}
                  isCut={isCut}
                  renamingFile={renamingFile}
                  renameValue={renameValue}
                  allTags={allTags}
                  columnWidths={columnWidths}
                  sourceCategory={sourceCategory}
                  onFileClick={onFileClick}
                  onFileDoubleClick={onFileDoubleClick}
                  onContextMenu={onContextMenu}
                  onDragStart={onDragStart}
                  onDragEnd={onDragEnd}
                  onDragOver={onDragOverFile}
                  onDragLeave={onDragLeave}
                  onDrop={onDrop}
                  onRenameChange={onSetRenameValue}
                  onRenameKeyDown={onHandleRenameKeyDown}
                  onRenameBlur={() => {
                    // Use setTimeout to allow click events to process first
                    // This prevents the blur from canceling the rename when clicking buttons
                    setTimeout(() => {
                      onCommitRename();
                    }, 100);
                  }}
                  onCommentClick={(file) =>
                    onSetCommentModal({ visible: true, file })
                  }
                  onSetSelectedFiles={onSetSelectedFiles}
                  filteredFiles={filteredFiles}
                  renameInputRef={renameInputRef}
                />
              );
            })}
            {/* Loading indicator for pagination */}
            {isLoadingMore && (
              <div
                style={{
                  padding: '16px',
                  textAlign: 'center',
                  color: 'var(--finder-text-quaternary)',
                }}
              >
                <div
                  className="spinner"
                  style={{
                    width: '20px',
                    height: '20px',
                    margin: '0 auto',
                    borderWidth: '2px',
                  }}
                />
                <span
                  style={{
                    fontSize: '12px',
                    marginTop: '8px',
                    display: 'block',
                  }}
                >
                  Loading more...
                </span>
              </div>
            )}
            {/* End of list indicator */}
            {!hasMore && files.length > 0 && (
              <div
                className="pagination-end"
                style={{
                  padding: '16px',
                  textAlign: 'center',
                  color: 'var(--finder-text-quaternary)',
                  fontSize: '12px',
                  flexShrink: 0,
                }}
              >
                All items loaded ({files.length}{' '}
                {files.length === 1 ? 'item' : 'items'})
              </div>
            )}
          </div>
        </div>
      )}
    </>
  );
}
