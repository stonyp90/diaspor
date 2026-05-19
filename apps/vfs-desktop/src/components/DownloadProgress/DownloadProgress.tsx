/**
 * DownloadProgress Component
 *
 * Displays progress for downloads with:
 * - Progress bar
 * - Speed and ETA
 * - Cancel controls
 * - Clean, modern UI (reuses upload UI styling)
 */
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import '../UploadProgress/UploadProgress.css';

export interface DownloadProgressData {
  operation_id: string;
  source_path: string;
  destination_path?: string;
  bytes_processed: number;
  file_size?: number;
  percentage: number;
  status: 'Pending' | 'InProgress' | 'Completed' | 'Failed' | 'Canceled';
  speed_bytes_per_sec?: number;
  estimated_time_remaining_sec?: number;
  error?: string;
}

interface DownloadProgressProps {
  operationId: string;
  fileName: string;
  onComplete?: () => void;
  onCancel?: () => void;
  onClose?: () => void;
}

export const DownloadProgress: React.FC<DownloadProgressProps> = ({
  operationId,
  fileName,
  onComplete,
  onCancel,
  onClose,
}) => {
  const [progress, setProgress] = useState<DownloadProgressData | null>(null);

  useEffect(() => {
    // Poll for progress updates
    const interval = setInterval(async () => {
      try {
        const progressData = await invoke<DownloadProgressData | null>(
          'vfs_get_download_progress',
          { operationId },
        );
        if (progressData) {
          // Calculate percentage if not provided
          const calculatedPercentage =
            progressData.file_size && progressData.file_size > 0
              ? (progressData.bytes_processed / progressData.file_size) * 100
              : 0;

          // Use provided percentage if valid, otherwise use calculated
          const displayPercentage =
            progressData.percentage >= 0 && progressData.percentage <= 100
              ? progressData.percentage
              : calculatedPercentage;

          // Update progress with calculated percentage
          const updatedProgress = {
            ...progressData,
            percentage: displayPercentage,
          };

          setProgress(updatedProgress);

          // Log progress for debugging
          if (
            progressData.status === 'InProgress' ||
            progressData.status === 'Pending'
          ) {
            console.log(
              `[DownloadProgress] ${operationId}: ${progressData.bytes_processed}/${progressData.file_size || 0} bytes (${displayPercentage.toFixed(1)}%) - Status: ${progressData.status}`,
            );
          }

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
        console.error('Failed to get download progress:', err);
      }
    }, 500); // Poll every 500ms

    return () => clearInterval(interval);
  }, [operationId, onComplete, onClose]);

  if (!progress) {
    return (
      <div className="upload-progress">
        <div className="upload-progress-loading">
          Loading download progress...
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

  const getStatusToken = ():
    | 'running'
    | 'complete'
    | 'failed'
    | 'canceled'
    | 'pending' => {
    switch (progress.status) {
      case 'Completed':
        return 'complete';
      case 'Failed':
        return 'failed';
      case 'Canceled':
        return 'canceled';
      case 'InProgress':
        return 'running';
      default:
        return 'pending';
    }
  };

  return (
    <div className="upload-progress" data-status={getStatusToken()}>
      <div className="upload-progress-header">
        <div className="upload-progress-info">
          <div className="upload-progress-filename" title={fileName}>
            {fileName}
          </div>
          <div className="upload-progress-stats">
            <span>
              {formatBytes(progress.bytes_processed)} /{' '}
              {formatBytes(progress.file_size || 0)}
            </span>
            <span className="upload-progress-separator">•</span>
            <span>
              {(() => {
                // Always calculate percentage from bytes_processed and file_size
                const calculatedPercentage =
                  progress.file_size && progress.file_size > 0
                    ? Math.max(
                        0,
                        Math.min(
                          100,
                          (progress.bytes_processed / progress.file_size) * 100,
                        ),
                      )
                    : 0;
                // Use provided percentage if valid, otherwise use calculated
                const displayPercentage =
                  progress.percentage >= 0 && progress.percentage <= 100
                    ? progress.percentage
                    : calculatedPercentage;
                return `${displayPercentage.toFixed(1)}%`;
              })()}
            </span>
            {progress.speed_bytes_per_sec && (
              <>
                <span className="upload-progress-separator">•</span>
                <span>{formatSpeed(progress.speed_bytes_per_sec)}</span>
              </>
            )}
            {progress.estimated_time_remaining_sec &&
              progress.estimated_time_remaining_sec > 0 && (
                <>
                  <span className="upload-progress-separator">•</span>
                  <span>
                    {formatTime(progress.estimated_time_remaining_sec)}{' '}
                    remaining
                  </span>
                </>
              )}
          </div>
        </div>
        <div className="upload-progress-actions">
          {progress.status === 'InProgress' && onCancel && (
            <button
              className="upload-progress-btn cancel"
              onClick={onCancel}
              title="Cancel download"
              aria-label="Cancel download"
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
              // Always calculate percentage from bytes_processed and file_size
              const calculatedPercentage =
                progress.file_size && progress.file_size > 0
                  ? Math.max(
                      0,
                      Math.min(
                        100,
                        (progress.bytes_processed / progress.file_size) * 100,
                      ),
                    )
                  : 0;
              // Use provided percentage if valid, otherwise use calculated
              const displayPercentage =
                progress.percentage >= 0 && progress.percentage <= 100
                  ? progress.percentage
                  : calculatedPercentage;
              return Math.max(0, Math.min(100, displayPercentage));
            })()}%`,
          }}
        />
      </div>

      {/* Show completion status */}
      {progress.status === 'Completed' && (
        <div className="upload-progress-success">
          Download completed successfully
        </div>
      )}

      {progress.status === 'Failed' && progress.error && (
        <div className="upload-progress-error">{progress.error}</div>
      )}

      {progress.status === 'Canceled' && (
        <div className="upload-progress-error">Download canceled</div>
      )}
    </div>
  );
};

export default DownloadProgress;
