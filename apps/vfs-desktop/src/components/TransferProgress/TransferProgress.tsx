/**
 * Unified TransferProgress Component
 *
 * Works for uploads, downloads, and sync operations with:
 * - Unified progress bar interface
 * - Speed and ETA display
 * - Pause/Resume/Cancel controls
 * - Clean, modern UI using global theme variables
 * - Cross-platform support
 */
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './TransferProgress.css';

export type TransferType = 'upload' | 'download' | 'sync' | 'delete';

export interface TransferProgressData {
  id: string;
  fileName: string;
  bytesProcessed: number;
  totalSize: number;
  percentage: number;
  status:
    | 'Pending'
    | 'InProgress'
    | 'Completed'
    | 'Failed'
    | 'Paused'
    | 'Canceled';
  speedBytesPerSec?: number;
  estimatedTimeRemainingSec?: number;
  error?: string;
  type: TransferType;
  // Additional metadata
  currentPart?: number;
  totalParts?: number;
  sourcePath?: string;
  destinationPath?: string;
}

interface TransferProgressProps {
  transferId: string;
  transferType: TransferType;
  fileName: string;
  onComplete?: () => void;
  onCancel?: () => void;
  onClose?: () => void;
  onPause?: () => void;
  onResume?: () => void;
}

export const TransferProgress: React.FC<TransferProgressProps> = ({
  transferId,
  transferType,
  fileName,
  onComplete,
  onCancel,
  onClose,
  onPause,
  onResume,
}) => {
  const [progress, setProgress] = useState<TransferProgressData | null>(null);

  useEffect(() => {
    // Poll for progress updates
    const interval = setInterval(async () => {
      try {
        let progressData: TransferProgressData | null = null;

        // Get progress based on transfer type
        switch (transferType) {
          case 'upload': {
            const uploadData = await invoke<{
              upload_id: string;
              key: string;
              bytes_uploaded: number;
              total_size: number;
              percentage: number;
              current_part?: number;
              total_parts?: number;
              status: string;
              speed_bytes_per_sec?: number;
              estimated_time_remaining_sec?: number;
              error?: string;
            } | null>('vfs_get_upload_progress', { uploadId: transferId });

            if (uploadData) {
              progressData = {
                id: uploadData.upload_id,
                fileName: uploadData.key,
                bytesProcessed: uploadData.bytes_uploaded,
                totalSize: uploadData.total_size,
                percentage: uploadData.percentage,
                status: uploadData.status as TransferProgressData['status'],
                speedBytesPerSec: uploadData.speed_bytes_per_sec,
                estimatedTimeRemainingSec:
                  uploadData.estimated_time_remaining_sec,
                error: uploadData.error,
                type: 'upload',
                currentPart: uploadData.current_part,
                totalParts: uploadData.total_parts,
              };
            }
            break;
          }

          case 'download': {
            const downloadData = await invoke<{
              operation_id: string;
              source_path: string;
              destination_path?: string;
              bytes_processed: number;
              file_size?: number;
              percentage: number;
              status: string;
              speed_bytes_per_sec?: number;
              estimated_time_remaining_sec?: number;
              error?: string;
            } | null>('vfs_get_download_progress', { operationId: transferId });

            if (downloadData) {
              progressData = {
                id: downloadData.operation_id,
                fileName: downloadData.source_path.split('/').pop() || fileName,
                bytesProcessed: downloadData.bytes_processed,
                totalSize: downloadData.file_size || 0,
                percentage: downloadData.percentage,
                status: downloadData.status as TransferProgressData['status'],
                speedBytesPerSec: downloadData.speed_bytes_per_sec,
                estimatedTimeRemainingSec:
                  downloadData.estimated_time_remaining_sec,
                error: downloadData.error,
                type: 'download',
                sourcePath: downloadData.source_path,
                destinationPath: downloadData.destination_path,
              };
            }
            break;
          }

          case 'sync': {
            // Sync operations use operation history
            const syncData = await invoke<{
              operation_id: string;
              bytes_processed: number;
              total_size: number;
              percentage: number;
              status: string;
              speed_bytes_per_sec?: number;
              estimated_time_remaining_sec?: number;
              error?: string;
            } | null>('vfs_get_operation_status', { operationId: transferId });

            if (syncData) {
              progressData = {
                id: syncData.operation_id,
                fileName,
                bytesProcessed: syncData.bytes_processed,
                totalSize: syncData.total_size,
                percentage: syncData.percentage,
                status: syncData.status as TransferProgressData['status'],
                speedBytesPerSec: syncData.speed_bytes_per_sec,
                estimatedTimeRemainingSec:
                  syncData.estimated_time_remaining_sec,
                error: syncData.error,
                type: 'sync',
              };
            }
            break;
          }

          case 'delete': {
            // Delete operations use vfs_list_operations
            const operations = await invoke<Array<{
              operation_id: string;
              operation_type: string;
              bytes_processed: number;
              file_size?: number;
              status: string;
              file_count?: number;
              error?: string;
            }>>('vfs_list_operations');
            
            const deleteOp = operations.find(
              (op) => op.operation_id === transferId && op.operation_type === 'Delete'
            );
            
            if (deleteOp) {
              const calculatedPercentage = deleteOp.file_count && deleteOp.file_count > 0
                ? 100 // For deletes, we don't track individual file progress, so show 100% when in progress
                : deleteOp.file_size && deleteOp.file_size > 0
                  ? (deleteOp.bytes_processed / deleteOp.file_size) * 100
                  : deleteOp.status === 'Completed' ? 100 : 0;
              
              progressData = {
                id: deleteOp.operation_id,
                fileName,
                bytesProcessed: deleteOp.bytes_processed,
                totalSize: deleteOp.file_size || 0,
                percentage: calculatedPercentage,
                status: deleteOp.status as TransferProgressData['status'],
                error: deleteOp.error,
                type: 'delete',
              };
            }
            break;
          }
        }

        if (progressData) {
          // Calculate percentage if not provided or invalid
          const calculatedPercentage =
            progressData.totalSize > 0
              ? Math.max(
                  0,
                  Math.min(
                    100,
                    (progressData.bytesProcessed / progressData.totalSize) *
                      100,
                  ),
                )
              : 0;

          const displayPercentage =
            progressData.percentage >= 0 && progressData.percentage <= 100
              ? progressData.percentage
              : calculatedPercentage;

          const updatedProgress = {
            ...progressData,
            percentage: displayPercentage,
          };

          setProgress(updatedProgress);

          // Notify on completion
          if (progressData.status === 'Completed' && onComplete) {
            onComplete();
          }
        } else {
          // Operation not found - might be completed and removed
          setTimeout(() => {
            if (onClose) {
              onClose();
            }
          }, 2000);
        }
      } catch (err) {
        console.error(`Failed to get ${transferType} progress:`, err);
      }
    }, 500); // Poll every 500ms

    return () => clearInterval(interval);
  }, [transferId, transferType, onComplete, onClose]);

  if (!progress) {
    return (
      <div className="transfer-progress">
        <div className="transfer-progress-loading">
          Loading {transferType} progress...
        </div>
      </div>
    );
  }

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
  };

  const formatSpeed = (bytesPerSec: number): string => {
    return `${formatBytes(bytesPerSec)}/s`;
  };

  const formatTime = (seconds: number): string => {
    if (seconds < 60) return `${Math.round(seconds)}s`;
    const mins = Math.floor(seconds / 60);
    const secs = Math.round(seconds % 60);
    return `${mins}m ${secs}s`;
  };

  const getStatusColor = (): string => {
    switch (progress.status) {
      case 'Completed':
        return 'var(--color-success)';
      case 'Failed':
        return 'var(--color-error)';
      case 'Paused':
      case 'Canceled':
        return 'var(--color-warning)';
      default:
        return 'var(--color-primary)';
    }
  };

  const getStatusIcon = (): string => {
    switch (progress.status) {
      case 'Completed':
        return '✓';
      case 'Failed':
        return '✗';
      case 'Paused':
        return '⏸';
      case 'Canceled':
        return '⊘';
      default:
        return transferType === 'upload'
          ? '↑'
          : transferType === 'download'
            ? '↓'
            : transferType === 'delete'
              ? '🗑'
              : '⇄';
    }
  };

  return (
    <div className={`transfer-progress transfer-progress-${transferType}`}>
      <div className="transfer-progress-header">
        <div className="transfer-progress-info">
          <div className="transfer-progress-title-row">
            <span className="transfer-progress-icon">{getStatusIcon()}</span>
            <div
              className="transfer-progress-filename"
              title={progress.fileName}
            >
              {progress.fileName}
            </div>
          </div>
          <div className="transfer-progress-stats">
            <span>
              {formatBytes(progress.bytesProcessed)} /{' '}
              {formatBytes(progress.totalSize)}
            </span>
            <span className="transfer-progress-separator">•</span>
            <span>{progress.percentage.toFixed(1)}%</span>
            {progress.speedBytesPerSec && (
              <>
                <span className="transfer-progress-separator">•</span>
                <span>{formatSpeed(progress.speedBytesPerSec)}</span>
              </>
            )}
            {progress.estimatedTimeRemainingSec &&
              progress.estimatedTimeRemainingSec > 0 && (
                <>
                  <span className="transfer-progress-separator">•</span>
                  <span>
                    {formatTime(progress.estimatedTimeRemainingSec)} remaining
                  </span>
                </>
              )}
            {progress.currentPart && progress.totalParts && (
              <>
                <span className="transfer-progress-separator">•</span>
                <span>
                  Part {progress.currentPart}/{progress.totalParts}
                </span>
              </>
            )}
          </div>
        </div>
        <div className="transfer-progress-actions">
          {progress.status === 'InProgress' && (
            <>
              {onPause && (
                <button
                  className="transfer-progress-btn pause"
                  onClick={onPause}
                  title="Pause"
                  aria-label="Pause transfer"
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
                    <rect x="6" y="4" width="4" height="16"></rect>
                    <rect x="14" y="4" width="4" height="16"></rect>
                  </svg>
                </button>
              )}
              {onCancel && (
                <button
                  className="transfer-progress-btn cancel"
                  onClick={onCancel}
                  title="Cancel"
                  aria-label="Cancel transfer"
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
            </>
          )}
          {progress.status === 'Paused' && onResume && (
            <button
              className="transfer-progress-btn resume"
              onClick={onResume}
              title="Resume"
              aria-label="Resume transfer"
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
                <polygon points="5 3 19 12 5 21 5 3"></polygon>
              </svg>
            </button>
          )}
          {(progress.status === 'Completed' ||
            progress.status === 'Failed' ||
            progress.status === 'Canceled') &&
            onClose && (
              <button
                className="transfer-progress-btn close"
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

      <div className="transfer-progress-bar-container">
        <div
          className="transfer-progress-bar"
          style={{
            width: `${Math.max(0, Math.min(100, progress.percentage))}%`,
            backgroundColor: getStatusColor(),
          }}
        />
      </div>

      {/* Status messages */}
      {progress.status === 'Completed' && (
        <div className="transfer-progress-success">
          {transferType === 'upload' && 'Upload completed successfully'}
          {transferType === 'download' && 'Download completed successfully'}
          {transferType === 'delete' && 'Delete completed successfully'}
          {transferType === 'sync' && 'Sync completed successfully'}
        </div>
      )}

      {progress.status === 'Failed' && progress.error && (
        <div className="transfer-progress-error">{progress.error}</div>
      )}

      {progress.status === 'Canceled' && (
        <div className="transfer-progress-error">
          {transferType === 'upload' && 'Upload canceled'}
          {transferType === 'download' && 'Download canceled'}
          {transferType === 'delete' && 'Delete canceled'}
          {transferType === 'sync' && 'Sync canceled'}
        </div>
      )}
    </div>
  );
};

export default TransferProgress;
