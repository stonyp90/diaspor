/**
 * ObjectStoragePanel Component
 *
 * Simplified UI for object storage (S3) operations:
 * - Upload files with progress tracking
 * - View current and past uploads
 * - Change storage tier (hot/nearline vs cold)
 * - Download files
 */

import React, { useEffect, useState, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { DialogService } from '../../services/dialog';
import './ObjectStoragePanel.css';

interface UploadState {
  upload_id: string;
  operation_id?: string;
  source_id: string;
  key: string;
  local_path: string;
  total_size: number;
  bytes_uploaded: number;
  current_part: number;
  total_parts: number;
  status:
    | 'Pending'
    | 'InProgress'
    | 'Paused'
    | 'Completed'
    | 'Failed'
    | 'Canceled';
  error?: string;
  speed_bytes_per_sec?: number;
  estimated_time_remaining_sec?: number;
  created_at?: string | number;
  completed_at?: string | number;
  last_updated_at?: string | number;
}

interface GroupedOperation {
  operation_id: string;
  uploads: UploadState[];
  total_size: number;
  file_count: number;
  status: UploadState['status'];
  completed_at?: string | number;
  last_updated_at?: string | number;
}

interface ObjectStoragePanelProps {
  sourceId: string;
  onRefresh?: () => void;
}

export const ObjectStoragePanel: React.FC<ObjectStoragePanelProps> = ({
  sourceId,
  onRefresh,
}) => {
  const [uploads, setUploads] = useState<UploadState[]>([]);
  const [isUploading, setIsUploading] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [expandedOperations, setExpandedOperations] = useState<Set<string>>(
    new Set(),
  );

  const loadUploads = useCallback(async () => {
    try {
      const uploadList = await invoke<UploadState[]>('vfs_list_uploads');
      // Filter uploads for this source with defensive checks
      const sourceUploads = (uploadList || []).filter(
        (u) => u && u.source_id === sourceId,
      );
      setUploads(sourceUploads);
    } catch (err) {
      console.error('Failed to load uploads:', err);
      // Set empty array on error to prevent rendering issues
      setUploads([]);
    }
  }, [sourceId]);

  useEffect(() => {
    loadUploads();
    const interval = setInterval(loadUploads, 1000); // Update every second
    return () => clearInterval(interval);
  }, [loadUploads]);

  const handleUpload = async () => {
    try {
      setIsUploading(true);

      const { invoke } = await import('@tauri-apps/api/core');
      const { open } = await import('@tauri-apps/plugin-dialog');

      // Unified upload dialog: Show file dialog that allows selecting both files and folders
      // On macOS/Windows, users can navigate to folders and select them, or use Cmd/Ctrl+Click
      // We'll detect what was selected and handle accordingly
      const fileResult = await open({
        multiple: true,
        directory: false, // File dialog, but we'll check for folders
        title: 'Select files and/or folders to upload',
      });

      if (!fileResult) {
        setIsUploading(false);
        return; // User canceled
      }

      const selectedPaths = Array.isArray(fileResult)
        ? fileResult
        : [fileResult];

      // Separate files and folders by checking each path
      const folders: string[] = [];
      const files: string[] = [];

      for (const path of selectedPaths) {
        try {
          const isDir = await invoke<boolean>('vfs_is_directory', {
            path: path,
          });
          if (isDir) {
            folders.push(path);
          } else {
            files.push(path);
          }
        } catch {
          // If check fails, assume it's a file
          files.push(path);
        }
      }

      // Show feedback if nothing was selected
      if (folders.length === 0 && files.length === 0) {
        setIsUploading(false);
        return;
      }

      // Log what was selected for debugging
      if (folders.length > 0 && files.length > 0) {
        console.log(
          `[ObjectStoragePanel] Processing ${folders.length} folder(s) and ${files.length} file(s)`,
        );
      } else if (folders.length > 0) {
        console.log(
          `[ObjectStoragePanel] Processing ${folders.length} folder(s)`,
        );
      } else {
        console.log(`[ObjectStoragePanel] Processing ${files.length} file(s)`);
      }

      // Process folders
      for (const folderPath of folders) {
        try {
          const uploadIds = await invoke<string[]>('vfs_upload_folder', {
            sourceId,
            localFolderPath: folderPath,
            s3BasePath: '',
            partSize: null,
          });
          console.log(
            `Folder upload started: ${folderPath} (${uploadIds.length} files)`,
          );
        } catch (err) {
          console.error(`Failed to upload folder ${folderPath}:`, err);
          const errorMessage = err instanceof Error ? err.message : String(err);
          DialogService.error(
            `Failed to upload folder ${folderPath}: ${errorMessage}`,
            'Upload Error',
          );
        }
      }

      // Process files
      for (const filePath of files) {
        try {
          // Upload as single file
          const fileName =
            filePath.split('/').pop() ||
            filePath.split('\\').pop() ||
            'unknown';
          await invoke('vfs_start_multipart_upload', {
            sourceId,
            localPath: filePath,
            s3Path: fileName,
            partSize: null,
          });
          console.log(`File upload started: ${fileName}`);
        } catch (err) {
          console.error(`Failed to upload ${filePath}:`, err);
          const errorMessage = err instanceof Error ? err.message : String(err);
          DialogService.error(
            `Failed to upload ${filePath}: ${errorMessage}`,
            'Upload Error',
          );
        }
      }

      setIsUploading(false);
      loadUploads();
      if (onRefresh) {
        onRefresh();
      }
    } catch (err) {
      console.error('Upload failed:', err);
      setIsUploading(false);
      const errorMessage = err instanceof Error ? err.message : String(err);
      DialogService.error(`Upload failed: ${errorMessage}`, 'Upload Error');
    }
  };

  const handleChangeTier = async (path: string, targetTier: 'hot' | 'cold') => {
    try {
      await invoke('vfs_change_tier', {
        sourceId,
        paths: [path],
        targetTier: targetTier,
      });
      if (onRefresh) {
        onRefresh();
      }
    } catch (err) {
      console.error('Failed to change tier:', err);
    }
  };

  const handleDownload = async (path: string) => {
    try {
      const fileName = path.split('/').pop() || 'download';

      // Open save dialog
      const savePath = await save({
        defaultPath: fileName,
        filters: [
          {
            name: 'All Files',
            extensions: ['*'],
          },
        ],
      });

      if (!savePath) {
        return; // User cancelled
      }

      // Download file using Rust command (handles file writing)
      const operationId = await invoke<string>('vfs_download_file', {
        sourceId,
        path,
        destPath: savePath,
      });

      // Trigger download-started event for OperationsPanel
      if (operationId) {
        setTimeout(() => {
          window.dispatchEvent(
            new CustomEvent('download-started', {
              detail: { operationId },
            }),
          );
        }, 100);
      }

      console.log('File downloaded successfully:', savePath);
    } catch (err) {
      console.error('Download failed:', err);
      const errorMessage = err instanceof Error ? err.message : String(err);
      DialogService.error(`Download failed: ${errorMessage}`, 'Download Error');
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  };

  const formatTime = (seconds?: number): string => {
    if (!seconds || seconds < 0) return '';
    if (seconds < 60) return `${seconds}s`;
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}m ${s}s`;
  };

  const getProgressPercentage = (upload: UploadState): number => {
    if (!upload || upload.total_size === 0) return 0;
    const uploaded = upload.bytes_uploaded || 0;
    const total = upload.total_size || 0;
    if (total === 0) return 0;
    return Math.min(100, Math.round((uploaded / total) * 100));
  };

  const getFileName = (upload: UploadState): string => {
    if (!upload || !upload.key) return 'Unknown';
    return upload.key.split('/').pop() || 'Unknown';
  };

  const toggleExpandOperation = (operationId: string) => {
    setExpandedOperations((prev) => {
      const next = new Set(prev);
      if (next.has(operationId)) {
        next.delete(operationId);
      } else {
        next.add(operationId);
      }
      return next;
    });
  };

  // Group uploads by operation_id
  const groupUploadsByOperation = (
    uploadsList: UploadState[],
  ): {
    grouped: GroupedOperation[];
    ungrouped: UploadState[];
  } => {
    const operationMap = new Map<string, UploadState[]>();
    const ungrouped: UploadState[] = [];

    for (const upload of uploadsList) {
      if (upload.operation_id && upload.operation_id.trim() !== '') {
        if (!operationMap.has(upload.operation_id)) {
          operationMap.set(upload.operation_id, []);
        }
        const operationList = operationMap.get(upload.operation_id);
        if (operationList) {
          operationList.push(upload);
        }
      } else {
        ungrouped.push(upload);
      }
    }

    const grouped: GroupedOperation[] = Array.from(operationMap.entries()).map(
      ([operation_id, uploads]) => {
        const total_size = uploads.reduce(
          (sum, u) => sum + (u.total_size || 0),
          0,
        );
        // Determine overall status (if any failed, mark as failed, otherwise use first status)
        const hasFailed = uploads.some((u) => u.status === 'Failed');
        const status = hasFailed
          ? 'Failed'
          : uploads.every((u) => u.status === 'Completed')
            ? 'Completed'
            : uploads[0]?.status || 'Pending';

        // Get the most recent completion time
        const completionTimes = uploads
          .map((u) => u.completed_at || u.last_updated_at || '')
          .filter((t) => t)
          .sort()
          .reverse();
        const completed_at = completionTimes[0] || undefined;
        const last_updated_at = completionTimes[0] || undefined;

        return {
          operation_id,
          uploads,
          total_size,
          file_count: uploads.length,
          status,
          completed_at,
          last_updated_at,
        };
      },
    );

    // Sort grouped operations by completion time (most recent first)
    grouped.sort((a, b) => {
      // Handle both number (timestamp) and string formats
      const aTime = a.completed_at || a.last_updated_at;
      const bTime = b.completed_at || b.last_updated_at;

      // Convert to numbers for comparison (most recent first)
      const aNum =
        typeof aTime === 'number'
          ? aTime
          : typeof aTime === 'string'
            ? parseFloat(aTime) || 0
            : 0;
      const bNum =
        typeof bTime === 'number'
          ? bTime
          : typeof bTime === 'string'
            ? parseFloat(bTime) || 0
            : 0;

      return bNum - aNum; // Most recent first (higher timestamp = more recent)
    });

    return { grouped, ungrouped };
  };

  // Memoize filtered uploads to prevent unnecessary re-renders
  const { activeUploads, completedUploads, failedUploads, groupedCompleted } =
    useMemo(() => {
      const uploadsList = uploads || [];

      const active = uploadsList.filter(
        (u) =>
          u &&
          (u.status === 'InProgress' ||
            u.status === 'Pending' ||
            u.status === 'Paused'),
      );

      const completed = uploadsList.filter(
        (u) => u && u.status === 'Completed',
      );
      const failed = uploadsList.filter((u) => u && u.status === 'Failed');

      // Group completed uploads by operation_id
      const { grouped, ungrouped } = groupUploadsByOperation(completed);
      const groupedCompleted = grouped.slice(0, showHistory ? 50 : 5);
      const ungroupedCompleted = ungrouped
        .sort((a, b) => {
          // Handle both number (timestamp) and string formats
          const aTime = a.completed_at || a.last_updated_at;
          const bTime = b.completed_at || b.last_updated_at;

          // Convert to numbers for comparison (most recent first)
          const aNum =
            typeof aTime === 'number'
              ? aTime
              : typeof aTime === 'string'
                ? parseFloat(aTime) || 0
                : 0;
          const bNum =
            typeof bTime === 'number'
              ? bTime
              : typeof bTime === 'string'
                ? parseFloat(bTime) || 0
                : 0;

          return bNum - aNum; // Most recent first (higher timestamp = more recent)
        })
        .slice(0, showHistory ? 50 : 5);

      return {
        activeUploads: active,
        completedUploads: ungroupedCompleted,
        failedUploads: failed,
        groupedCompleted,
      };
    }, [uploads, showHistory]);

  return (
    <div className="object-storage-panel">
      <div className="object-storage-header">
        <h3>Object Storage</h3>
        <button
          className="object-storage-upload-btn"
          onClick={handleUpload}
          disabled={isUploading}
          title="Upload files, folders, or a mix of both"
        >
          {isUploading ? 'Uploading...' : 'Upload'}
        </button>
      </div>

      {/* Current Uploads */}
      {activeUploads.length > 0 && (
        <div className="object-storage-section">
          <h4>Uploading ({activeUploads.length})</h4>
          <div className="upload-list">
            {activeUploads
              .map((upload) => {
                if (!upload || !upload.upload_id) return null;
                const percentage = getProgressPercentage(upload);
                const fileName = getFileName(upload);
                const bytesUploaded = upload.bytes_uploaded || 0;
                const totalSize = upload.total_size || 0;
                return (
                  <div key={upload.upload_id} className="upload-item">
                    <div className="upload-item-header">
                      <span className="upload-item-name" title={fileName}>
                        {fileName}
                      </span>
                      <span className="upload-item-percentage">
                        {percentage}%
                      </span>
                    </div>
                    <div className="upload-progress-bar">
                      <div
                        className="upload-progress-fill"
                        style={{
                          width: `${Math.min(100, Math.max(0, percentage))}%`,
                        }}
                      />
                    </div>
                    <div className="upload-item-details">
                      <span>
                        {formatBytes(bytesUploaded)} / {formatBytes(totalSize)}
                      </span>
                      {upload.speed_bytes_per_sec && (
                        <>
                          <span className="upload-separator">•</span>
                          <span>
                            {formatBytes(upload.speed_bytes_per_sec)}/s
                          </span>
                        </>
                      )}
                      {upload.estimated_time_remaining_sec && (
                        <>
                          <span className="upload-separator">•</span>
                          <span>
                            {formatTime(upload.estimated_time_remaining_sec)}{' '}
                            left
                          </span>
                        </>
                      )}
                    </div>
                  </div>
                );
              })
              .filter(Boolean)}
          </div>
        </div>
      )}

      {/* Failed Uploads */}
      {failedUploads.length > 0 && (
        <div className="object-storage-section">
          <h4>Failed ({failedUploads.length})</h4>
          <div className="upload-list">
            {failedUploads
              .map((upload) => {
                if (!upload || !upload.upload_id) return null;
                const fileName = getFileName(upload);
                return (
                  <div
                    key={upload.upload_id}
                    className="upload-item upload-item-failed"
                  >
                    <div className="upload-item-header">
                      <span className="upload-item-name" title={fileName}>
                        {fileName}
                      </span>
                    </div>
                    {upload.error && (
                      <div className="upload-item-error">{upload.error}</div>
                    )}
                  </div>
                );
              })
              .filter(Boolean)}
          </div>
        </div>
      )}

      {/* Completed Uploads */}
      {(groupedCompleted.length > 0 || completedUploads.length > 0) && (
        <div className="object-storage-section">
          <div className="object-storage-section-header">
            <h4>
              Recent Uploads (
              {groupedCompleted.length + completedUploads.length})
            </h4>
            {groupedCompleted.length + completedUploads.length > 5 && (
              <button
                className="object-storage-toggle-history"
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  try {
                    setShowHistory(!showHistory);
                  } catch (err) {
                    console.error('Error toggling history:', err);
                  }
                }}
              >
                {showHistory ? 'Show Less' : 'Show All'}
              </button>
            )}
          </div>
          <div className="upload-list">
            {/* Grouped Operations */}
            {groupedCompleted.map((operation) => {
              const isExpanded = expandedOperations.has(operation.operation_id);
              return (
                <div
                  key={operation.operation_id}
                  className={`upload-item upload-item-completed ${
                    isExpanded ? 'expanded' : ''
                  }`}
                >
                  <div
                    className="upload-item-header"
                    style={{ cursor: 'pointer' }}
                    onClick={() =>
                      toggleExpandOperation(operation.operation_id)
                    }
                    title={isExpanded ? 'Click to collapse' : 'Click to expand'}
                  >
                    <div
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '0.5rem',
                        flex: 1,
                      }}
                    >
                      <span
                        style={{
                          fontSize: '0.75rem',
                          color: 'var(--vfs-primary, #6366f1)',
                          transition: 'transform 0.2s',
                          display: 'inline-block',
                        }}
                      >
                        {isExpanded ? '▼' : '▶'}
                      </span>
                      <span className="upload-item-name">
                        {operation.file_count} file(s) uploaded
                      </span>
                    </div>
                    <span className="upload-item-size">
                      {formatBytes(operation.total_size)}
                    </span>
                  </div>
                  {isExpanded && (
                    <div
                      style={{ marginTop: '0.75rem', paddingLeft: '1.5rem' }}
                    >
                      {operation.uploads.map((upload) => {
                        if (!upload || !upload.upload_id) return null;
                        const fileName = getFileName(upload);
                        const fileSize = upload.total_size || 0;
                        return (
                          <div
                            key={upload.upload_id}
                            style={{
                              padding: '0.5rem',
                              marginBottom: '0.5rem',
                              background: 'var(--vfs-bg-secondary, #1a1a1a)',
                              borderRadius: '4px',
                              border: '1px solid var(--vfs-border, #333)',
                            }}
                          >
                            <div
                              style={{
                                display: 'flex',
                                justifyContent: 'space-between',
                                alignItems: 'center',
                                marginBottom: '0.25rem',
                              }}
                            >
                              <span
                                className="upload-item-name"
                                title={fileName}
                                style={{ fontSize: '0.8125rem' }}
                              >
                                {fileName}
                              </span>
                              <span
                                className="upload-item-size"
                                style={{ fontSize: '0.75rem' }}
                              >
                                {formatBytes(fileSize)}
                              </span>
                            </div>
                            <div className="upload-item-actions">
                              <button
                                className="upload-action-btn"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  upload.key && handleDownload(upload.key);
                                }}
                                title="Download"
                              >
                                Download
                              </button>
                              <button
                                className="upload-action-btn"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  upload.key &&
                                    handleChangeTier(upload.key, 'hot');
                                }}
                                title="Move to Hot Tier (Standard storage)"
                              >
                                Hot
                              </button>
                              <button
                                className="upload-action-btn"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  upload.key &&
                                    handleChangeTier(upload.key, 'cold');
                                }}
                                title="Move to Cold Tier (Instant retrieval, lower cost)"
                              >
                                Cold
                              </button>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              );
            })}

            {/* Ungrouped Uploads */}
            {completedUploads.map((upload) => {
              if (!upload || !upload.upload_id) return null;
              const fileName = getFileName(upload);
              const fileSize = upload.total_size || 0;
              return (
                <div
                  key={upload.upload_id}
                  className="upload-item upload-item-completed"
                >
                  <div className="upload-item-header">
                    <span className="upload-item-name" title={fileName}>
                      {fileName}
                    </span>
                    <span className="upload-item-size">
                      {formatBytes(fileSize)}
                    </span>
                  </div>
                  <div className="upload-item-actions">
                    <button
                      className="upload-action-btn"
                      onClick={() => upload.key && handleDownload(upload.key)}
                      title="Download"
                    >
                      Download
                    </button>
                    <button
                      className="upload-action-btn"
                      onClick={() =>
                        upload.key && handleChangeTier(upload.key, 'hot')
                      }
                      title="Move to Hot Tier (Standard storage)"
                    >
                      Hot
                    </button>
                    <button
                      className="upload-action-btn"
                      onClick={() =>
                        upload.key && handleChangeTier(upload.key, 'cold')
                      }
                      title="Move to Cold Tier (Instant retrieval, lower cost)"
                    >
                      Cold
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {activeUploads.length === 0 &&
        completedUploads.length === 0 &&
        failedUploads.length === 0 && (
          <div className="object-storage-empty">
            <p>No uploads yet</p>
            <button
              className="object-storage-upload-btn"
              onClick={handleUpload}
            >
              Upload Files or Folders
            </button>
          </div>
        )}
    </div>
  );
};

export default ObjectStoragePanel;
