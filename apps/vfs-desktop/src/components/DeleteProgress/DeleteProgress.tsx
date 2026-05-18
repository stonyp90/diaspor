/**
 * DeleteProgress Component
 *
 * Displays progress for delete operations with:
 * - Progress bar
 * - File count and status
 * - Cancel controls
 * - Clean, modern UI (reuses upload UI styling)
 */
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import '../UploadProgress/UploadProgress.css';

export interface DeleteProgressData {
  operation_id: string;
  source_path: string;
  bytes_processed: number;
  file_size?: number;
  percentage: number;
  status: 'Pending' | 'InProgress' | 'Completed' | 'Failed' | 'Canceled';
  file_count?: number;
  error?: string;
}

interface DeleteProgressProps {
  operationId: string;
  fileName: string;
  onComplete?: () => void;
  onCancel?: () => void;
  onClose?: () => void;
}

export const DeleteProgress: React.FC<DeleteProgressProps> = ({
  operationId,
  fileName,
  onComplete,
  onCancel,
  onClose,
}) => {
  const [progress, setProgress] = useState<DeleteProgressData | null>(null);

  useEffect(() => {
    // Poll for progress updates
    const interval = setInterval(async () => {
      try {
        // Get operation details from vfs_list_operations
        const operations = await invoke<
          Array<{
            operation_id: string;
            operation_type: string;
            source_path: string;
            bytes_processed: number;
            file_size?: number;
            status: string;
            file_count?: number;
            error?: string;
          }>
        >('vfs_list_operations');

        const deleteOp = operations.find(
          (op) =>
            op.operation_id === operationId && op.operation_type === 'Delete',
        );

        if (deleteOp) {
          // Calculate percentage based on file_count or bytes_processed
          const calculatedPercentage =
            deleteOp.file_count && deleteOp.file_count > 0
              ? 100 // For deletes, we don't track individual file progress, so show 100% when in progress
              : deleteOp.file_size && deleteOp.file_size > 0
                ? (deleteOp.bytes_processed / deleteOp.file_size) * 100
                : deleteOp.status === 'Completed'
                  ? 100
                  : 0;

          const progressData: DeleteProgressData = {
            operation_id: deleteOp.operation_id,
            source_path: deleteOp.source_path,
            bytes_processed: deleteOp.bytes_processed,
            file_size: deleteOp.file_size,
            percentage: calculatedPercentage,
            status: deleteOp.status as DeleteProgressData['status'],
            file_count: deleteOp.file_count,
            error: deleteOp.error,
          };

          setProgress(progressData);

          // Don't auto-close on complete - let user close manually
          if (progressData.status === 'Completed' && onComplete) {
            onComplete();
          }
        } else {
          // Operation not found - might be completed and removed
          // Keep showing it for a bit, then auto-close
          setTimeout(() => {
            if (onClose) {
              onClose();
            }
          }, 2000);
        }
      } catch (err) {
        console.error('Failed to get delete progress:', err);
      }
    }, 500); // Poll every 500ms

    return () => clearInterval(interval);
  }, [operationId, onComplete, onClose]);

  if (!progress) {
    return (
      <div className="upload-progress">
        <div className="upload-progress-loading">
          Loading delete progress...
        </div>
      </div>
    );
  }

  const getStatusColor = (): string => {
    switch (progress.status) {
      case 'Completed':
        return 'var(--vfs-success, #30d158)';
      case 'Failed':
        return 'var(--vfs-error, #ff453a)';
      case 'Canceled':
        return 'var(--vfs-warning, #ff9f0a)';
      default:
        return 'var(--vfs-primary, #6366f1)';
    }
  };

  return (
    <div className="upload-progress">
      <div className="upload-progress-header">
        <div className="upload-progress-info">
          <div className="upload-progress-filename" title={fileName}>
            {fileName}
          </div>
          <div className="upload-progress-stats">
            {progress.file_count !== undefined && (
              <>
                <span>
                  {progress.status === 'Completed'
                    ? `${progress.file_count} file${progress.file_count !== 1 ? 's' : ''} deleted`
                    : progress.status === 'InProgress' ||
                        progress.status === 'Pending'
                      ? `Deleting ${progress.file_count} file${progress.file_count !== 1 ? 's' : ''}...`
                      : `${progress.file_count} file${progress.file_count !== 1 ? 's' : ''}`}
                </span>
                <span className="upload-progress-separator">•</span>
              </>
            )}
            <span>
              {(() => {
                const displayPercentage =
                  progress.percentage >= 0 && progress.percentage <= 100
                    ? progress.percentage
                    : progress.status === 'Completed'
                      ? 100
                      : 0;
                return `${displayPercentage.toFixed(1)}%`;
              })()}
            </span>
          </div>
        </div>
        <div className="upload-progress-actions">
          {(progress.status === 'InProgress' ||
            progress.status === 'Pending') &&
            onCancel && (
              <button
                className="upload-progress-btn cancel"
                onClick={onCancel}
                title="Cancel delete"
                aria-label="Cancel delete"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            )}
          {(progress.status === 'Completed' ||
            progress.status === 'Failed' ||
            progress.status === 'Canceled') &&
            onClose && (
              <button
                className="upload-progress-btn close"
                onClick={onClose}
                title="Close"
                aria-label="Close"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            )}
        </div>
      </div>

      <div className="upload-progress-bar-container">
        <div
          className="upload-progress-bar"
          style={{
            width: `${(() => {
              const displayPercentage =
                progress.percentage >= 0 && progress.percentage <= 100
                  ? progress.percentage
                  : progress.status === 'Completed'
                    ? 100
                    : 0;
              return Math.max(0, Math.min(100, displayPercentage));
            })()}%`,
            backgroundColor: getStatusColor(),
          }}
        />
      </div>

      {/* Show completion status */}
      {progress.status === 'Completed' && (
        <div className="upload-progress-success">
          Delete completed successfully
        </div>
      )}

      {progress.status === 'Failed' && progress.error && (
        <div className="upload-progress-error">{progress.error}</div>
      )}

      {progress.status === 'Canceled' && (
        <div className="upload-progress-error">Delete canceled</div>
      )}
    </div>
  );
};

export default DeleteProgress;
