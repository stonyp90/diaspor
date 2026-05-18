/**
 * FileGridItem Component
 *
 * Individual file/folder item in grid/icon view.
 */

import React from 'react';
import type { FileMetadata, StorageCategory } from '../../types/storage';
import { getFileIcon } from '../../pages/FinderPage/utils';
import { IconComment } from '../CyberpunkIcons';
import './FileGridItem.css';

export interface FileGridItemProps {
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
  sourceCategory?: StorageCategory;
  onFileClick: (file: FileMetadata, e: React.MouseEvent) => void;
  onFileDoubleClick: (file: FileMetadata) => void;
  onContextMenu: (e: React.MouseEvent, file?: FileMetadata) => void;
  onDragStart: (e: React.DragEvent, file: FileMetadata) => void;
  onDragEnd: () => void;
  onDragOver: (e: React.DragEvent, path: string, isFolder: boolean) => void;
  onDragLeave: () => void;
  onDrop: (e: React.DragEvent, path: string) => void;
  onKeyDown: (
    e: React.KeyboardEvent,
    file: FileMetadata,
    index: number,
  ) => void;
  onRenameChange: (value: string) => void;
  onRenameKeyDown: (e: React.KeyboardEvent) => void;
  onRenameBlur: () => void;
  onCommentClick: (file: FileMetadata) => void;
  renameInputRef: React.RefObject<HTMLInputElement>;
}

export function FileGridItem({
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
  sourceCategory,
  onFileClick,
  onFileDoubleClick,
  onContextMenu,
  onDragStart,
  onDragEnd,
  onDragOver,
  onDragLeave,
  onDrop,
  onKeyDown,
  onRenameChange,
  onRenameKeyDown,
  onRenameBlur,
  onCommentClick,
  renameInputRef,
}: FileGridItemProps) {
  return (
    <div
      key={file.path}
      data-path={file.path}
      className={`file-item ${isSelected ? 'selected' : ''} ${isFolder ? 'folder' : ''} ${isDropTarget ? 'drop-target' : ''} ${isDragging ? 'dragging' : ''} ${isHidden ? 'is-hidden' : ''} ${isCut ? 'is-cut' : ''}`}
      onClick={(e) => onFileClick(file, e)}
      onDoubleClick={() => onFileDoubleClick(file)}
      onContextMenu={(e) => onContextMenu(e, file)}
      data-type={isFolder ? 'folder' : 'file'}
      tabIndex={0}
      draggable={true}
      onDragStart={(e) => {
        console.log('[FileItem] Drag start triggered for:', file.path);
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
          '[FileItem] Drop triggered on:',
          file.path,
          'isFolder:',
          isFolder,
        );
        if (isFolder) {
          onDrop(e, file.path);
        }
      }}
      onKeyDown={(e) => onKeyDown(e, file, index)}
    >
      <div className="file-icon">
        {file.thumbnail ? (
          <img src={file.thumbnail} alt="" className="file-thumbnail" />
        ) : (
          <span className="icon-placeholder">{getFileIcon(file, 48)}</span>
        )}
      </div>
      <div className="file-name" title={file.name}>
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
          <span className="file-name-text">{file.name}</span>
        )}
      </div>
      
      {/* Visual indicators for tags and comments */}
      <div className="grid-item-badges">
        {file.tags && file.tags.length > 0 && (
          <div className="grid-tag-indicator" title={`${file.tags.length} tag${file.tags.length > 1 ? 's' : ''}`}>
            <span className="grid-badge-count">{file.tags.length}</span>
          </div>
        )}
        {file.comments && (
          <div
            className="grid-comment-indicator"
            onClick={(e) => {
              e.stopPropagation();
              onCommentClick(file);
            }}
            title="Has comment"
          >
            <IconComment size={12} glow={false} />
          </div>
        )}
      </div>
    </div>
  );
}
