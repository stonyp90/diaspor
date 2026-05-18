/**
 * FileListRow Component
 *
 * Individual file/folder row in list view.
 */

import React from 'react';
import type { FileMetadata, StorageCategory } from '../../types/storage';
import { getCategoryName } from '../../types/storage';
import { IconComment } from '../CyberpunkIcons';
import {
  getFileIcon,
  formatDate,
  formatSize,
  getStorageClassBadge,
} from '../../pages/FinderPage/utils';
import './FileListRow.css';

export interface FileListRowProps {
  file: FileMetadata;
  index: number;
  isSelected: boolean;
  isFolder: boolean;
  isDropTarget: boolean;
  isDragging: boolean;
  isHidden: boolean;
  isCut: boolean;
  renamingFile: string | null;
  renameValue: string;
  allTags: Array<{ name: string; color?: string }>;
  columnWidths: {
    name: number;
    modified: number;
    size: number;
    tier: number;
    'storage-class'?: number;
  } | null;
  sourceCategory?: StorageCategory;
  onFileClick: (file: FileMetadata, e: React.MouseEvent) => void;
  onFileDoubleClick: (file: FileMetadata) => void;
  onContextMenu: (e: React.MouseEvent, file?: FileMetadata) => void;
  onDragStart: (e: React.DragEvent, file: FileMetadata) => void;
  onDragEnd: () => void;
  onDragOver: (e: React.DragEvent, path: string, isFolder: boolean) => void;
  onDragLeave: () => void;
  onDrop: (e: React.DragEvent, path: string) => void;
  onRenameChange: (value: string) => void;
  onRenameKeyDown: (e: React.KeyboardEvent) => void;
  onRenameBlur: () => void;
  onCommentClick: (file: FileMetadata) => void;
  onSetSelectedFiles: (files: Set<string>) => void;
  filteredFiles: FileMetadata[];
  renameInputRef: React.RefObject<HTMLInputElement>;
}

export function FileListRow({
  file,
  index,
  isSelected,
  isFolder,
  isDropTarget,
  isDragging,
  isHidden,
  isCut,
  renamingFile,
  renameValue,
  allTags,
  columnWidths,
  sourceCategory,
  onFileClick,
  onFileDoubleClick,
  onContextMenu,
  onDragStart,
  onDragEnd,
  onDragOver,
  onDragLeave,
  onDrop,
  onRenameChange,
  onRenameKeyDown,
  onRenameBlur,
  onCommentClick,
  onSetSelectedFiles,
  filteredFiles,
  renameInputRef,
}: FileListRowProps) {
  return (
    <div
      key={file.path}
      data-path={file.path}
      className={`list-row ${isSelected ? 'selected' : ''} ${isFolder ? 'folder' : ''} ${isDropTarget ? 'drop-target' : ''} ${isDragging ? 'dragging' : ''} ${isHidden ? 'is-hidden' : ''} ${isCut ? 'is-cut' : ''}`}
      style={
        columnWidths
          ? {
              gridTemplateColumns: `${columnWidths.name}px ${columnWidths.modified}px ${columnWidths.size}px 0px ${columnWidths['storage-class'] || 140}px`,
            }
          : undefined
      }
      onClick={(e) => onFileClick(file, e)}
      onDoubleClick={() => onFileDoubleClick(file)}
      onContextMenu={(e) => onContextMenu(e, file)}
      data-type={isFolder ? 'folder' : 'file'}
      tabIndex={0}
      draggable={true}
      onDragStart={(e) => {
        console.log('[ListRow] Drag start triggered for:', file.path);
        onDragStart(e, file);
      }}
      onDragEnd={onDragEnd}
      onDragOver={(e) => {
        e.preventDefault();
        e.stopPropagation();
        onDragOver(e, file.path, isFolder);
      }}
      onDragLeave={onDragLeave}
      onDrop={(e) => {
        e.preventDefault();
        e.stopPropagation();
        console.log(
          '[ListRow] Drop triggered on:',
          file.path,
          'isFolder:',
          isFolder,
        );
        if (isFolder) {
          onDrop(e, file.path);
        }
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter') onFileDoubleClick(file);
        // Context menu keyboard shortcut (Shift+F10 or Menu key)
        if ((e.shiftKey && e.key === 'F10') || e.key === 'ContextMenu') {
          e.preventDefault();
          e.stopPropagation();
          // Get element position for context menu
          const rect = e.currentTarget.getBoundingClientRect();
          // Create a synthetic mouse event for context menu
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
          onContextMenu(syntheticEvent, file);
          return;
        }
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          const nextEl = e.currentTarget.nextElementSibling as HTMLElement;
          if (nextEl) {
            nextEl.focus();
            const next = filteredFiles[index + 1];
            if (next) onSetSelectedFiles(new Set([next.path]));
          }
        }
        if (e.key === 'ArrowUp') {
          e.preventDefault();
          const prevEl = e.currentTarget.previousElementSibling as HTMLElement;
          if (prevEl) {
            prevEl.focus();
            const prev = filteredFiles[index - 1];
            if (prev) onSetSelectedFiles(new Set([prev.path]));
          }
        }
      }}
    >
      <div className="col-name" title={file.name}>
        <span className="row-icon">{getFileIcon(file, 18)}</span>
        {renamingFile === file.path ? (
          <input
            ref={renameInputRef}
            type="text"
            className="rename-input"
            value={renameValue}
            onChange={(e) => onRenameChange(e.target.value)}
            onKeyDown={onRenameKeyDown}
            onBlur={onRenameBlur}
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <>
            <span className="file-name-text">{file.name}</span>
            {/* Tags in list view */}
            {file.tags && file.tags.length > 0 && (
              <span className="list-tags">
                {file.tags.slice(0, 3).map((tag, i) => {
                  const tagName = typeof tag === 'string' ? tag : tag.name;
                  const tagColor =
                    typeof tag === 'string'
                      ? allTags.find((t) => t.name === tagName)?.color ||
                        '#6b7280'
                      : tag.color ||
                        allTags.find((t) => t.name === tagName)?.color ||
                        '#6b7280';
                  return (
                    <span
                      key={i}
                      className="list-tag-badge"
                      style={{
                        backgroundColor: tagColor,
                        borderColor: tagColor,
                      }}
                      title={tagName}
                    >
                      {tagName.length > 8 ? tagName.slice(0, 8) + '…' : tagName}
                    </span>
                  );
                })}
                {file.tags.length > 3 && (
                  <span
                    className="list-tag-more"
                    title={`${file.tags.length} tags total`}
                  >
                    +{file.tags.length - 3}
                  </span>
                )}
              </span>
            )}
            {/* Comment indicator */}
            {file.comments && (
              <span
                className="list-comment-indicator"
                title={file.comments}
                onClick={(e) => {
                  e.stopPropagation();
                  onCommentClick(file);
                }}
              >
                <IconComment size={14} glow={false} />
              </span>
            )}
          </>
        )}
      </div>
      <div className="col-date">{formatDate(file.lastModified)}</div>
      <div className="col-size">{formatSize(file.size)}</div>
      <div className="col-tier">
        {/* Tier column - empty, storage class badge is in col-storage-class */}
      </div>
      <div className="col-storage-class">
        {sourceCategory &&
          (() => {
            const badge = getStorageClassBadge(sourceCategory, file.tierStatus);
            return badge.letter ? (
              <span
                className={`storage-tier-badge ${badge.tierClass}`}
                title={getCategoryName(sourceCategory)}
              >
                {badge.letter}
              </span>
            ) : null;
          })()}
      </div>
    </div>
  );
}
