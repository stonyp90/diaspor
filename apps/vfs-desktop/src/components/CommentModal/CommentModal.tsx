/**
 * CommentModal - View and reply to file comments
 *
 * A modal for viewing and editing comments on files/folders.
 */

import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { IconComment, IconX } from '../CyberpunkIcons';
import type { FileMetadata } from '../../types/storage';
import './CommentModal.css';

interface CommentModalProps {
  file: FileMetadata;
  sourceId?: string;
  onClose: () => void;
  onUpdateComments?: (file: FileMetadata, comments: string) => void;
}

export const CommentModal: React.FC<CommentModalProps> = ({
  file,
  sourceId,
  onClose,
  onUpdateComments,
}) => {
  const [comment, setComment] = useState(file.comments || '');
  const [isEditing, setIsEditing] = useState(!file.comments);
  const [isSaving, setIsSaving] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Focus textarea when editing
  useEffect(() => {
    if (isEditing && textareaRef.current) {
      textareaRef.current.focus();
      // Move cursor to end
      textareaRef.current.setSelectionRange(
        textareaRef.current.value.length,
        textareaRef.current.value.length,
      );
    }
  }, [isEditing]);

  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (isEditing) {
          setIsEditing(false);
          setComment(file.comments || '');
        } else {
          onClose();
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown, true);
    return () => document.removeEventListener('keydown', handleKeyDown, true);
  }, [isEditing, file.comments, onClose]);

  const handleSave = async () => {
    setIsSaving(true);
    try {
      // Persist to backend
      if (sourceId) {
        await invoke('vfs_set_comment', {
          sourceId,
          path: file.path,
          comment: comment.trim() || null,
        });
      }

      // Update parent
      if (onUpdateComments) {
        onUpdateComments(file, comment.trim());
      }

      setIsEditing(false);
    } catch (error) {
      console.error('Failed to save comment:', error);
    } finally {
      setIsSaving(false);
    }
  };

  const handleDelete = async () => {
    setIsSaving(true);
    try {
      // Persist to backend
      if (sourceId) {
        await invoke('vfs_set_comment', {
          sourceId,
          path: file.path,
          comment: null,
        });
      }

      // Update parent
      if (onUpdateComments) {
        onUpdateComments(file, '');
      }

      setComment('');
      setIsEditing(true);
    } catch (error) {
      console.error('Failed to delete comment:', error);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="comment-modal-overlay" onClick={onClose}>
      <div
        className="comment-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="comment-modal-title"
      >
        <div className="comment-modal-header">
          <div className="comment-modal-title-row">
            <IconComment size={20} glow />
            <h3 id="comment-modal-title">Comments</h3>
          </div>
          <button
            className="comment-modal-close"
            onClick={onClose}
            title="Close"
          >
            <IconX size={16} />
          </button>
        </div>

        <div className="comment-modal-file-info">
          <span className="comment-modal-file-name">{file.name}</span>
          <span className="comment-modal-file-path">{file.path}</span>
        </div>

        <div className="comment-modal-content">
          {isEditing ? (
            <div className="comment-editor">
              <textarea
                ref={textareaRef}
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                placeholder="Add a comment about this file..."
                className="comment-textarea"
                rows={6}
              />
              <div className="comment-actions">
                <button
                  className="comment-btn cancel"
                  onClick={() => {
                    setIsEditing(false);
                    setComment(file.comments || '');
                  }}
                  disabled={isSaving}
                >
                  Cancel
                </button>
                <button
                  className="comment-btn save"
                  onClick={handleSave}
                  disabled={isSaving}
                >
                  {isSaving ? 'Saving...' : 'Save Comment'}
                </button>
              </div>
            </div>
          ) : (
            <div className="comment-display">
              <div className="comment-text">{comment}</div>
              <div className="comment-actions">
                <button
                  className="comment-btn edit"
                  onClick={() => setIsEditing(true)}
                >
                  Edit
                </button>
                <button
                  className="comment-btn delete"
                  onClick={handleDelete}
                  disabled={isSaving}
                >
                  {isSaving ? 'Deleting...' : 'Delete'}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default CommentModal;
