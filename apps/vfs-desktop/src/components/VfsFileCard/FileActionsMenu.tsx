/**
 * File Actions Context Menu
 *
 * Displays a contextual menu with file operations following POSIX semantics.
 */
import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { FileMetadata } from '../../types/storage';
import { formatSize } from '../../pages/FinderPage/utils';
import './VfsFileCard.css';

export type FileAction =
  | 'open'
  | 'rename'
  | 'copy'
  | 'move'
  | 'delete'
  | 'warm'
  | 'transcode'
  | 'transcribe'
  | 'tag'
  | 'download'
  | 'info'
  | 'share'
  | 'archive'
  | 'retrieve'
  | 'mkdir'
  | 'preview'
  | 'play'
  | 'tier-hot'
  | 'tier-instant-retrieval'
  | 'tier-cold'
  | 'duplicate';

interface MenuAction {
  id: FileAction;
  label: string;
  icon: string;
  shortcut?: string;
  divider?: boolean;
  disabled?: boolean;
  danger?: boolean;
}

interface FileActionsMenuProps {
  file: FileMetadata;
  position: { x: number; y: number };
  onAction: (action: FileAction) => void;
  onClose: () => void;
  /** If true, show only limited features for object storage (download, tier management, delete) */
  isObjectStorage?: boolean;
  /** Callback to navigate to AI settings when AI features are not available */
  onOpenAISettings?: () => void;
}

export function FileActionsMenu({
  file,
  position,
  onAction,
  onClose,
  isObjectStorage = false,
  onOpenAISettings,
}: FileActionsMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [isTaggingModelRunning, setIsTaggingModelRunning] = useState(false);

  // Check if tagging model is running
  useEffect(() => {
    const checkTaggingModel = async () => {
      try {
        const response = await invoke<{
          models?: Array<{ name: string; model?: string }>;
        }>('ollama_ps');
        const runningModels = response.models || [];
        const runningModelNames = runningModels.map((m) => {
          const modelName = (m.model || m.name || '').toLowerCase();
          return modelName;
        });
        const taggingRunning = runningModelNames.some(
          (n) =>
            n === 'llava' ||
            n === 'llava:latest' ||
            n.includes('llava:') ||
            n.includes('llava'),
        );
        setIsTaggingModelRunning(taggingRunning);
      } catch (error) {
        // Ollama not running or not available
        setIsTaggingModelRunning(false);
      }
    };

    checkTaggingModel();
  }, []);

  // Adjust position if menu would go off-screen
  useEffect(() => {
    if (menuRef.current) {
      const rect = menuRef.current.getBoundingClientRect();
      const windowWidth = window.innerWidth;
      const windowHeight = window.innerHeight;

      let x = position.x;
      let y = position.y;

      if (x + rect.width > windowWidth) {
        x = windowWidth - rect.width - 10;
      }
      if (y + rect.height > windowHeight) {
        y = windowHeight - rect.height - 10;
      }

      menuRef.current.style.left = `${x}px`;
      menuRef.current.style.top = `${y}px`;
    }
  }, [position]);

  // Close on escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  const getMenuActions = (): MenuAction[] => {
    const actions: MenuAction[] = [];

    // For object storage, show full feature set: open, copy, cut, paste, rename, duplicate, tier management, delete
    if (isObjectStorage) {
      // Open action for folders
      if (file.isDirectory) {
        actions.push({
          id: 'open',
          label: 'Open Folder',
          icon: '📂',
          shortcut: '⌘O',
        });
      }

      // Download for files
      if (!file.isDirectory) {
        actions.push({
          id: 'download',
          label: 'Download',
          icon: '⬇️',
          shortcut: '⌘D',
        });
      }

      // Play action for video files
      if (!file.isDirectory) {
        const isVideoFile = file.mimeType?.startsWith('video/') ||
          ['mp4', 'mov', 'avi', 'mkv', 'webm', 'm4v'].some(ext => 
            file.name.toLowerCase().endsWith(`.${ext}`)
          );
        
        if (isVideoFile) {
          actions.push({
            id: 'play',
            label: 'Play Video',
            icon: '▶️',
            divider: true,
          });
        }
      }

      // Asset Details / Get Info
      actions.push({
        id: 'info',
        label: 'Asset Details',
        icon: 'ℹ️',
        shortcut: '⌘I',
        divider: true,
      });

      // File operations: Copy, Cut, Rename, Duplicate
      actions.push({
        id: 'copy',
        label: 'Copy',
        icon: '📋',
        shortcut: '⌘C',
      });
      actions.push({
        id: 'move',
        label: 'Cut',
        icon: '✂️',
        shortcut: '⌘X',
      });
      actions.push({
        id: 'rename',
        label: 'Rename',
        icon: '✏️',
        shortcut: '⏎',
      });
      if (!file.isDirectory) {
        actions.push({
          id: 'duplicate',
          label: 'Duplicate',
          icon: '📄',
        });
      }

      // Tier management (only for files)
      if (!file.isDirectory) {
        actions.push({
          id: 'tier-hot',
          label: 'Move to Hot Tier',
          icon: '🔥',
          divider: true,
        });
        actions.push({
          id: 'tier-instant-retrieval',
          label: 'Move to Instant Retrieval',
          icon: '⚡',
        });
        actions.push({
          id: 'tier-cold',
          label: 'Move to Cold Tier',
          icon: '❄️',
        });
      }

      // AI Features: Transcription and Tagging for object storage files
      if (!file.isDirectory) {
        const isMediaFile =
          file.mimeType?.startsWith('video/') ||
          file.mimeType?.startsWith('audio/');
        const isImageFile = file.mimeType?.startsWith('image/');
        const isPdfFile = file.mimeType === 'application/pdf';

        // Transcription: available for video, audio, PDF, and images
        if (isMediaFile || isPdfFile || isImageFile) {
          actions.push({
            id: 'transcribe',
            label: isMediaFile
              ? 'Transcribe Audio'
              : isPdfFile
                ? 'Extract Text (OCR)'
                : 'Extract Text (OCR)',
            icon: '🎤',
            divider: true,
          });
        }

        // Tagging: only show if tagging model is running
        // If running, show "Generate Tags", otherwise show "AI Tag Suggestions" (which will prompt to start model)
        if (isTaggingModelRunning) {
          actions.push({
            id: 'tag',
            label: 'Generate Tags',
            icon: '🏷️',
            divider: true,
          });
        } else {
          // Only show for video/images when model is not running
          if (isMediaFile || isImageFile) {
            actions.push({
              id: 'tag',
              label: 'AI Tag Suggestions',
              icon: '🏷️',
              divider: true,
            });
          }
        }
      }

      // Delete
      actions.push({
        id: 'delete',
        label: 'Delete',
        icon: '🗑️',
        shortcut: '⌘⌫',
        danger: true,
        divider: true,
      });

      return actions;
    }

    // Full feature set for non-object storage (mount-based storage)

    // Open action
    actions.push({
      id: 'open',
      label: file.isDirectory ? 'Open Folder' : 'Open',
      icon: file.isDirectory ? '📂' : '📄',
      shortcut: '⌘O',
    });

    // Preview (for supported file types)
    if (
      !file.isDirectory &&
      (file.canTranscode || file.name.match(/\.(jpg|jpeg|png|gif|webp|pdf)$/i))
    ) {
      actions.push({
        id: 'preview',
        label: 'Quick Look',
        icon: '👁️',
        shortcut: 'Space',
      });
    }

    actions.push({
      id: 'info',
      label: 'Get Info',
      icon: 'ℹ️',
      shortcut: '⌘I',
      divider: true,
    });

    // File operations
    actions.push({ id: 'rename', label: 'Rename', icon: '✏️', shortcut: '⏎' });
    actions.push({ id: 'copy', label: 'Copy', icon: '📋', shortcut: '⌘C' });
    actions.push({
      id: 'move',
      label: 'Move To...',
      icon: '📦',
      shortcut: '⌘M',
    });

    if (!file.isDirectory) {
      actions.push({
        id: 'download',
        label: 'Download',
        icon: '⬇️',
        shortcut: '⌘D',
        divider: true,
      });
    } else {
      actions.push({
        id: 'mkdir',
        label: 'New Folder Inside',
        icon: '📁',
        divider: true,
      });
    }

    // Tier operations
    if (file.canWarm && !file.isCached) {
      actions.push({
        id: 'warm',
        label: 'Hydrate (Warm)',
        icon: '🔥',
        shortcut: '⌘H',
      });
    }

    if ((file.tierStatus as string) === 'archive') {
      actions.push({
        id: 'retrieve',
        label: 'Retrieve from Archive',
        icon: '📤',
      });
    } else if (!file.isDirectory) {
      actions.push({
        id: 'archive',
        label: 'Move to Archive',
        icon: '📥',
      });
    }

    // Transcode (video files only)
    if (file.canTranscode) {
      actions.push({
        id: 'transcode',
        label: 'Transcode to HLS',
        icon: '🎥',
        shortcut: '⌘T',
      });
    }

    // AI Features: Transcription and Tagging
    // Show for mounted storage files (PDF, images, video, audio)
    if (!isObjectStorage && !file.isDirectory) {
      const isMediaFile =
        file.mimeType?.startsWith('video/') ||
        file.mimeType?.startsWith('audio/');
      const isImageFile = file.mimeType?.startsWith('image/');
      const isPdfFile = file.mimeType === 'application/pdf';

      // Transcription: available for video, audio, PDF, and images
      if (isMediaFile || isPdfFile || isImageFile) {
        actions.push({
          id: 'transcribe',
          label: isMediaFile
            ? 'Transcribe Audio'
            : isPdfFile
              ? 'Extract Text (OCR)'
              : 'Extract Text (OCR)',
          icon: '🎤',
          shortcut: '⌘⇧T',
        });
      }

        // Tagging: only show if tagging model is running
        // If running, show "Generate Tags", otherwise show "AI Tag Suggestions" (which will prompt to start model)
        if (isTaggingModelRunning) {
          actions.push({
            id: 'tag',
            label: 'Generate Tags',
            icon: '🏷️',
            shortcut: '⌘⇧A',
            divider: true,
          });
        } else {
          // Only show for video/images when model is not running
          if (isMediaFile || isImageFile) {
            actions.push({
              id: 'tag',
              label: 'AI Tag Suggestions',
              icon: '🏷️',
              shortcut: '⌘⇧A',
              divider: true,
            });
          }
        }
    }

    // Share
    actions.push({
      id: 'share',
      label: 'Share...',
      icon: '🔗',
      shortcut: '⌘⇧S',
      divider: true,
    });

    // Delete
    actions.push({
      id: 'delete',
      label: 'Move to Trash',
      icon: '🗑️',
      shortcut: '⌘⌫',
      danger: true,
    });

    return actions;
  };

  const handleActionClick = async (action: MenuAction) => {
    if (action.disabled) return;

    // For AI actions (transcribe, tag), check availability and redirect to settings if needed
    if (action.id === 'transcribe' || action.id === 'tag') {
      try {
        const { invoke } = await import('@tauri-apps/api/core');

        if (action.id === 'transcribe') {
          const isAvailable = await invoke<boolean>(
            'vfs_is_transcription_available',
          );
          if (!isAvailable && onOpenAISettings) {
            onOpenAISettings();
            return;
          }
        } else if (action.id === 'tag') {
          const isOllamaRunning = await invoke<boolean>('check_ollama_running');
          if (!isOllamaRunning && onOpenAISettings) {
            onOpenAISettings();
            return;
          }
        }
      } catch (err) {
        console.error('Failed to check AI availability:', err);
        // On error, still redirect to settings to let user configure
        if (onOpenAISettings) {
          onOpenAISettings();
          return;
        }
      }
    }

    onAction(action.id);
  };

  const actions = getMenuActions();

  return (
    <div
      ref={menuRef}
      className="file-actions-menu"
      style={{
        left: position.x,
        top: position.y,
      }}
    >
      <div className="menu-header">
        <span className="menu-title">{file.name}</span>
        <span className="menu-subtitle">
          {file.size_human || formatSize(file.size)}
        </span>
      </div>
      <div className="menu-divider" />
      {actions.map((action, index) => (
        <React.Fragment key={action.id}>
          <button
            className={`menu-item ${action.disabled ? 'disabled' : ''} ${action.danger ? 'danger' : ''}`}
            onClick={() => handleActionClick(action)}
            disabled={action.disabled}
          >
            <span className="menu-icon">{action.icon}</span>
            <span className="menu-label">{action.label}</span>
            {action.shortcut && (
              <span className="menu-shortcut">{action.shortcut}</span>
            )}
          </button>
          {action.divider && index < actions.length - 1 && (
            <div className="menu-divider" />
          )}
        </React.Fragment>
      ))}
    </div>
  );
}

export default FileActionsMenu;
