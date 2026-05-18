/**
 * VFS File Card Component
 *
 * A modern file card with thumbnail preview, tier status indicators,
 * and contextual actions menu.
 */
import React, { useState, useRef, useEffect } from 'react';
import type { FileMetadata } from '../../types/storage';
import { FileActionsMenu, FileAction } from './FileActionsMenu';
import { formatSize } from '../../pages/FinderPage/utils';
import './VfsFileCard.css';

export interface VfsFileCardProps {
  file: FileMetadata;
  selected?: boolean;
  viewMode?: 'grid' | 'list';
  onSelect?: (file: FileMetadata, multiSelect: boolean) => void;
  onDoubleClick?: (file: FileMetadata) => void;
  onAction?: (action: FileAction, file: FileMetadata) => void;
  warmProgress?: number;
  transcodeProgress?: number;
  thumbnail?: string;
  /** If true, show only limited features for object storage (download, tier management, delete) */
  isObjectStorage?: boolean;
  /** Callback to navigate to AI settings when AI features are not available */
  onOpenAISettings?: () => void;
}

export function VfsFileCard({
  file,
  selected = false,
  viewMode = 'grid',
  onSelect,
  onDoubleClick,
  onAction,
  warmProgress,
  transcodeProgress,
  thumbnail,
  isObjectStorage = false,
  onOpenAISettings,
}: VfsFileCardProps) {
  const [showMenu, setShowMenu] = useState(false);
  const [menuPosition, setMenuPosition] = useState({ x: 0, y: 0 });
  const cardRef = useRef<HTMLDivElement>(null);

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (cardRef.current && !cardRef.current.contains(e.target as Node)) {
        setShowMenu(false);
      }
    };

    if (showMenu) {
      document.addEventListener('mousedown', handleClickOutside);
      return () =>
        document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [showMenu]);

  const handleClick = (e: React.MouseEvent) => {
    const multiSelect = e.metaKey || e.ctrlKey;
    onSelect?.(file, multiSelect);
  };

  const handleDoubleClick = () => {
    onDoubleClick?.(file);
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setMenuPosition({ x: e.clientX, y: e.clientY });
    setShowMenu(true);
  };

  const handleAction = (action: FileAction) => {
    setShowMenu(false);
    onAction?.(action, file);
  };

  const getFileIcon = () => {
    if (file.isDirectory) return '📁';

    const ext = file.name.split('.').pop()?.toLowerCase() || '';
    const iconMap: Record<string, string> = {
      mp4: '🎬',
      mov: '🎬',
      mkv: '🎬',
      avi: '🎬',
      webm: '🎬',
      mp3: '🎵',
      wav: '🎵',
      flac: '🎵',
      jpg: '🖼️',
      jpeg: '🖼️',
      png: '🖼️',
      gif: '🖼️',
      webp: '🖼️',
      pdf: '📄',
      doc: '📝',
      docx: '📝',
      txt: '📃',
      zip: '📦',
      tar: '📦',
      gz: '📦',
    };

    return iconMap[ext] || '📄';
  };

  const renderProgress = () => {
    const progress = warmProgress ?? transcodeProgress;
    if (progress === undefined || progress >= 100) return null;

    const isTranscode = transcodeProgress !== undefined;

    return (
      <div className="progress-overlay">
        <div className="progress-bar">
          <div
            className={`progress-fill ${isTranscode ? 'transcode' : 'warm'}`}
            style={{ width: `${progress}%` }}
          />
        </div>
        <span className="progress-text">
          {isTranscode ? 'Transcoding' : 'Warming'}: {progress.toFixed(0)}%
        </span>
      </div>
    );
  };

  if (viewMode === 'list') {
    return (
      <div
        ref={cardRef}
        className={`vfs-file-card list-view ${selected ? 'selected' : ''}`}
        onClick={handleClick}
        onDoubleClick={handleDoubleClick}
        onContextMenu={handleContextMenu}
      >
        <div className="file-icon">{getFileIcon()}</div>
        <div className="file-info">
          <div className="file-name-row">
            <span className="file-name">{file.name}</span>
            {file.tags && file.tags.length > 0 && (
              <div className="file-tags-inline">
                {file.tags.slice(0, 2).map((tag, idx) => (
                  <span
                    key={idx}
                    className="file-tag-inline"
                    style={{
                      backgroundColor: tag.color
                        ? `${tag.color}20`
                        : 'var(--vfs-card-border)',
                      color: tag.color || 'var(--vfs-text-secondary)',
                      borderColor: tag.color || 'transparent',
                    }}
                    title={tag.name}
                  >
                    {tag.name}
                  </span>
                ))}
                {file.tags.length > 2 && (
                  <span className="file-tag-more-inline">
                    +{file.tags.length - 2}
                  </span>
                )}
              </div>
            )}
          </div>
          <span className="file-meta">
            {file.size_human || formatSize(file.size)}
          </span>
        </div>
        <div className="file-date">{file.lastModified}</div>
        <div className="file-actions">
          {file.canWarm && !file.isCached && (
            <button
              className="action-btn warm"
              onClick={(e) => {
                e.stopPropagation();
                handleAction('warm');
              }}
              title="Hydrate file"
            >
              🔥
            </button>
          )}
          {file.canTranscode && (
            <button
              className="action-btn transcode"
              onClick={(e) => {
                e.stopPropagation();
                handleAction('transcode');
              }}
              title="Transcode to HLS"
            >
              🎥
            </button>
          )}
          <button
            className="action-btn menu"
            onClick={(e) => {
              e.stopPropagation();
              setMenuPosition({ x: e.clientX, y: e.clientY });
              setShowMenu(true);
            }}
            title="More actions"
          >
            ⋮
          </button>
        </div>
        {renderProgress()}
        {showMenu && (
          <FileActionsMenu
            file={file}
            position={menuPosition}
            onAction={handleAction}
            onClose={() => setShowMenu(false)}
            isObjectStorage={isObjectStorage}
            onOpenAISettings={onOpenAISettings}
          />
        )}
      </div>
    );
  }

  // Grid view
  return (
    <div
      ref={cardRef}
      className={`vfs-file-card grid-view ${selected ? 'selected' : ''}`}
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
      onContextMenu={handleContextMenu}
    >
      <div className="card-thumbnail">
        {thumbnail ? (
          <img src={thumbnail} alt={file.name} />
        ) : (
          <span className="file-icon-large">{getFileIcon()}</span>
        )}
        {renderProgress()}
      </div>
      <div className="card-content">
        <span className="file-name" title={file.name}>
          {file.name}
        </span>
        <span className="file-meta">
          {file.size_human || formatSize(file.size)}
        </span>
        {file.tags && file.tags.length > 0 && (
          <div className="file-tags">
            {file.tags.slice(0, 3).map((tag, idx) => (
              <span
                key={idx}
                className="file-tag"
                style={{
                  backgroundColor: tag.color
                    ? `${tag.color}20`
                    : 'var(--vfs-card-border)',
                  color: tag.color || 'var(--vfs-text-secondary)',
                  borderColor: tag.color || 'transparent',
                }}
                title={tag.name}
              >
                {tag.name}
              </span>
            ))}
            {file.tags.length > 3 && (
              <span className="file-tag-more">+{file.tags.length - 3}</span>
            )}
          </div>
        )}
      </div>
      <div className="card-actions">
        {file.canWarm && !file.isCached && (
          <button
            className="action-btn warm"
            onClick={(e) => {
              e.stopPropagation();
              handleAction('warm');
            }}
            title="Hydrate file"
          >
            🔥
          </button>
        )}
        {file.canTranscode && (
          <button
            className="action-btn transcode"
            onClick={(e) => {
              e.stopPropagation();
              handleAction('transcode');
            }}
            title="Transcode to HLS"
          >
            🎥
          </button>
        )}
      </div>
      {showMenu && (
        <FileActionsMenu
          file={file}
          position={menuPosition}
          onAction={handleAction}
          onClose={() => setShowMenu(false)}
          isObjectStorage={isObjectStorage}
          onOpenAISettings={onOpenAISettings}
        />
      )}
    </div>
  );
}

export default VfsFileCard;
