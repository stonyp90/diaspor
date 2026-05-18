/**
 * TranscriptionProgress Component
 *
 * Displays progress for transcription operations with:
 * - Progress bar
 * - Status information
 * - Cancel controls
 * - Clean, modern UI (reuses upload UI styling)
 */
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import '../UploadProgress/UploadProgress.css';

export interface TranscriptionProgressData {
  operation_id: string;
  source_id: string;
  source_path: string;
  file_size?: number;
  bytes_processed: number;
  percentage: number;
  status: 'Pending' | 'InProgress' | 'Completed' | 'Failed' | 'Canceled';
  error?: string;
}

interface TranscriptionProgressProps {
  operationId: string;
  fileName: string;
  onComplete?: () => void;
  onCancel?: () => void;
  onClose?: () => void;
}

export const TranscriptionProgress: React.FC<TranscriptionProgressProps> = ({
  operationId,
  fileName,
  onComplete,
  onCancel,
  onClose,
}) => {
  const [progress, setProgress] = useState<TranscriptionProgressData | null>(
    null,
  );

  useEffect(() => {
    // Poll for progress updates
    const interval = setInterval(async () => {
      try {
        const progressData = await invoke<TranscriptionProgressData | null>(
          'vfs_get_transcription_progress',
          { operationId },
        );
        if (progressData) {
          // Calculate percentage if not provided
          const calculatedPercentage =
            progressData.file_size && progressData.file_size > 0
              ? (progressData.bytes_processed / progressData.file_size) * 100
              : progressData.percentage || 0;

          // Use provided percentage if valid, otherwise use calculated
          const displayPercentage =
            progressData.percentage >= 0 && progressData.percentage <= 100
              ? progressData.percentage
              : calculatedPercentage;

          // Update progress with calculated percentage
          const updatedProgress = {
            ...progressData,
            percentage: Math.min(100, Math.max(0, displayPercentage)),
          };

          setProgress(updatedProgress);

          // Log progress for debugging
          if (
            progressData.status === 'InProgress' ||
            progressData.status === 'Pending'
          ) {
            console.log(
              `[TranscriptionProgress] ${operationId}: ${progressData.bytes_processed}/${progressData.file_size || 0} bytes (${displayPercentage.toFixed(1)}%) - Status: ${progressData.status}`,
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
        console.error('Failed to get transcription progress:', err);
      }
    }, 500); // Poll every 500ms

    return () => clearInterval(interval);
  }, [operationId, onComplete, onClose]);

  if (!progress) {
    return (
      <div className="upload-progress">
        <div className="upload-progress-loading">
          Loading transcription progress...
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

  const getStatusText = (): string => {
    switch (progress.status) {
      case 'Pending':
        return 'Waiting...';
      case 'InProgress':
        return 'Transcribing...';
      case 'Completed':
        return 'Completed';
      case 'Failed':
        return 'Failed';
      case 'Canceled':
        return 'Canceled';
      default:
        return 'Unknown';
    }
  };

  const handleCancel = async () => {
    try {
      await invoke('vfs_cancel_transcription', { operation_id: operationId });
      if (onCancel) {
        onCancel();
      }
    } catch (err) {
      console.error('Failed to cancel transcription:', err);
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
            <span>{getStatusText()}</span>
            <span className="upload-progress-separator">•</span>
            <span>
              {(() => {
                const displayPercentage =
                  progress.percentage >= 0 && progress.percentage <= 100
                    ? progress.percentage
                    : 0;
                return `${displayPercentage.toFixed(1)}%`;
              })()}
            </span>
            {progress.file_size && (
              <>
                <span className="upload-progress-separator">•</span>
                <span>
                  {formatBytes(progress.bytes_processed)} /{' '}
                  {formatBytes(progress.file_size)}
                </span>
              </>
            )}
          </div>
        </div>
        <div className="upload-progress-actions">
          {progress.status === 'InProgress' && (
            <button
              className="upload-progress-btn cancel"
              onClick={handleCancel}
              aria-label="Cancel transcription"
            >
              <svg viewBox="0 0 16 16" fill="currentColor">
                <path d="M16 8A8 8 0 1 1 0 8a8 8 0 0 1 16 0zM5.354 4.646a.5.5 0 1 0-.708.708L7.293 8l-2.647 2.646a.5.5 0 0 0 .708.708L8 8.707l2.646 2.647a.5.5 0 0 0 .708-.708L8.707 8l2.647-2.646a.5.5 0 0 0-.708-.708L8 7.293 5.354 4.646z" />
              </svg>
            </button>
          )}
          {(progress.status === 'Completed' ||
            progress.status === 'Failed' ||
            progress.status === 'Canceled') && (
            <button
              className="upload-progress-btn close"
              onClick={onClose}
              aria-label="Close"
              title="Close"
            >
              <svg viewBox="0 0 16 16" fill="currentColor">
                <path d="M2.146 2.854a.5.5 0 1 1 .708-.708L8 7.293l5.146-5.147a.5.5 0 0 1 .708.708L8.707 8l5.147 5.146a.5.5 0 0 1-.708.708L8 8.707l-5.146 5.147a.5.5 0 0 1-.708-.708L7.293 8 2.146 2.854Z" />
              </svg>
            </button>
          )}
        </div>
      </div>

      <div className="upload-progress-bar">
        <div
          className="upload-progress-fill"
          style={{
            width: `${Math.min(100, Math.max(0, progress.percentage))}%`,
            backgroundColor: getStatusColor(),
          }}
        />
      </div>

      {progress.error && (
        <div className="upload-progress-error">{progress.error}</div>
      )}
    </div>
  );
};
