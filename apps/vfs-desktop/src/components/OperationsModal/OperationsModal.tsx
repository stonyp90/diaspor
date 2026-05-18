/**
 * OperationsModal Component
 *
 * Generic modal for displaying and managing all file operations
 * Extensible per operation type (Upload, Download, Delete, Copy, Move, etc.)
 */

import React, { useEffect, useState, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useOperationTracking } from '../../hooks/useOperationTracking';
import { POLLING_INTERVALS } from '../../utils/operationEvents';
import type { OperationType, OperationStatus } from '../../types/operations';
import './OperationsModal.css';

interface OperationFile {
  local_path: string;
  remote_path: string;
  file_size: number;
  bytes_processed: number;
  status?: OperationStatus;
  error?: string;
}

interface Operation {
  operation_id: string;
  operation_type: OperationType;
  source_id: string;
  source_path: string;
  destination_path?: string;
  file_size?: number;
  bytes_processed: number;
  status: OperationStatus;
  error?: string;
  files?: OperationFile[];
  file_count?: number;
  created_at?: string | number;
  completed_at?: string | number;
  last_updated_at?: string | number;
}

interface GroupedOperation {
  operation_id: string;
  operation_type: OperationType;
  operations: Operation[];
  total_size: number;
  bytes_processed: number;
  file_count: number;
  status: OperationStatus;
  has_paused: boolean;
  has_in_progress: boolean;
  source_id: string;
  source_path: string;
  destination_path?: string;
}

interface OperationsModalProps {
  isVisible: boolean;
  onClose?: () => void;
  filterTypes?: OperationType[]; // Filter to specific operation types
}

/**
 * Operation type configuration - extensible per action type
 */
interface OperationTypeConfig {
  label: string;
  icon: string;
  supportsPauseResume: boolean;
  supportsCancel: boolean;
  getActionLabel: (status: OperationStatus) => string;
  getProgressComponent?: (operation: Operation) => React.ReactNode;
}

const OPERATION_TYPE_CONFIGS: Record<OperationType, OperationTypeConfig> = {
  Upload: {
    label: 'Upload',
    icon: '↑',
    supportsPauseResume: true,
    supportsCancel: true,
    getActionLabel: (status) => {
      if (status === 'Paused') return 'Resume';
      if (status === 'InProgress' || status === 'Pending') return 'Pause';
      return '';
    },
  },
  Download: {
    label: 'Download',
    icon: '↓',
    supportsPauseResume: false, // Downloads don't support pause/resume yet
    supportsCancel: true,
    getActionLabel: () => '',
  },
  Delete: {
    label: 'Delete',
    icon: '🗑',
    supportsPauseResume: false,
    supportsCancel: true,
    getActionLabel: () => '',
  },
  Move: {
    label: 'Move',
    icon: '→',
    supportsPauseResume: false,
    supportsCancel: true,
    getActionLabel: () => '',
  },
  Copy: {
    label: 'Copy',
    icon: '📋',
    supportsPauseResume: false,
    supportsCancel: true,
    getActionLabel: () => '',
  },
  Paste: {
    label: 'Paste',
    icon: '📄',
    supportsPauseResume: false,
    supportsCancel: true,
    getActionLabel: () => '',
  },
  Rename: {
    label: 'Rename',
    icon: '✏️',
    supportsPauseResume: false,
    supportsCancel: false,
    getActionLabel: () => '',
  },
  CreateDir: {
    label: 'Create Folder',
    icon: '📁',
    supportsPauseResume: false,
    supportsCancel: false,
    getActionLabel: () => '',
  },
  RemoveDir: {
    label: 'Remove Folder',
    icon: '📂',
    supportsPauseResume: false,
    supportsCancel: true,
    getActionLabel: () => '',
  },
  TierChange: {
    label: 'Change Storage Tier',
    icon: '📊',
    supportsPauseResume: false,
    supportsCancel: true,
    getActionLabel: () => '',
  },
  Transcribe: {
    label: 'Transcribe',
    icon: '🎤',
    supportsPauseResume: false,
    supportsCancel: true,
    getActionLabel: () => '',
  },
  Transcode: {
    label: 'Transcode',
    icon: '🎬',
    supportsPauseResume: false,
    supportsCancel: true,
    getActionLabel: () => '',
  },
  AutoTag: {
    label: 'Auto Tag',
    icon: '🏷️',
    supportsPauseResume: false,
    supportsCancel: true,
    getActionLabel: () => '',
  },
};

export const OperationsModal: React.FC<OperationsModalProps> = ({
  isVisible,
  onClose,
  filterTypes,
}) => {
  const [, setOperations] = useState<Operation[]>([]);
  const [uploads, setUploads] = useState<
    Array<{
      upload_id: string;
      operation_id?: string;
      status: string;
    }>
  >([]);
  const [groupedOperations, setGroupedOperations] = useState<
    GroupedOperation[]
  >([]);
  const [visibleOperations, setVisibleOperations] = useState<Set<string>>(
    new Set(),
  );
  const [expandedOperations, setExpandedOperations] = useState<Set<string>>(
    new Set(),
  );
  const [activeTab, setActiveTab] = useState<'active' | 'history'>('active');

  const loadOperations = useCallback(async () => {
    try {
      // Load all operations
      const operationList = await invoke<Operation[]>('vfs_list_operations');

      // Filter by operation types if specified
      const filteredOps = filterTypes
        ? operationList.filter((op) =>
            filterTypes.includes(op.operation_type as OperationType),
          )
        : operationList;

      // Load uploads for pause/resume functionality
      const uploadList = await invoke<
        Array<{
          upload_id: string;
          operation_id?: string;
          status: string;
        }>
      >('vfs_list_uploads').catch(() => []);

      setUploads(uploadList);

      // Group operations by operation_id (bulk operations)
      const opsByOperationId = new Map<string, Operation[]>();
      const ungroupedOps: Operation[] = [];

      for (const op of filteredOps) {
        if (op.operation_id) {
          if (!opsByOperationId.has(op.operation_id)) {
            opsByOperationId.set(op.operation_id, []);
          }
          const opsList = opsByOperationId.get(op.operation_id);
          if (opsList) {
            opsList.push(op);
          }
        } else {
          ungroupedOps.push(op);
        }
      }

      // Create grouped operations
      const grouped: GroupedOperation[] = [];

      for (const [operationId, ops] of opsByOperationId.entries()) {
        const op = ops[0]; // Use first operation for metadata
        const total_size =
          op.file_size || ops.reduce((sum, o) => sum + (o.file_size || 0), 0);
        const bytes_processed =
          op.bytes_processed ||
          ops.reduce((sum, o) => sum + o.bytes_processed, 0);
        const file_count = op.file_count || op.files?.length || ops.length;
        const statuses = ops.map((o) => o.status);
        const has_paused = statuses.includes('Paused');
        const has_in_progress =
          statuses.includes('InProgress') || statuses.includes('Pending');

        // Determine overall status
        // Check if all files are actually at 100% before marking as Completed
        const allFiles = ops.flatMap((o) => o.files || []);
        const allFilesComplete =
          allFiles.length === 0 ||
          (allFiles.length > 0 &&
            allFiles.every((file) => {
              const fileStatus = file.status || 'Pending';
              const statusComplete =
                fileStatus === 'Completed' || fileStatus === 'Failed';
              // File is truly complete only if status is Completed/Failed AND bytes_processed >= file_size
              // Allow small tolerance (1 byte) for rounding issues
              const bytesComplete = file.bytes_processed >= file.file_size - 1;
              return statusComplete && bytesComplete;
            }));

        let overallStatus: OperationStatus = 'Completed';
        if (has_in_progress) overallStatus = 'InProgress';
        else if (has_paused) overallStatus = 'Paused';
        else if (statuses.some((s) => s === 'Failed')) overallStatus = 'Failed';
        else if (statuses.every((s) => s === 'Completed') && allFilesComplete)
          overallStatus = 'Completed';
        else if (statuses.every((s) => s === 'Completed') && !allFilesComplete)
          // Operations are marked Completed but files aren't at 100% yet
          overallStatus = 'InProgress';

        grouped.push({
          operation_id: operationId,
          operation_type: op.operation_type as OperationType,
          operations: ops,
          total_size,
          bytes_processed,
          file_count,
          status: overallStatus,
          has_paused,
          has_in_progress,
          source_id: op.source_id,
          source_path: op.source_path,
          destination_path: op.destination_path,
        });
      }

      // Sort: active first, then by creation time
      grouped.sort((a, b) => {
        const aActive = a.has_in_progress || a.has_paused;
        const bActive = b.has_in_progress || b.has_paused;
        if (aActive !== bActive) return aActive ? -1 : 1;
        const aTime =
          typeof a.operations[0].created_at === 'number'
            ? a.operations[0].created_at
            : typeof a.operations[0].created_at === 'string'
              ? parseFloat(a.operations[0].created_at) || 0
              : 0;
        const bTime =
          typeof b.operations[0].created_at === 'number'
            ? b.operations[0].created_at
            : typeof b.operations[0].created_at === 'string'
              ? parseFloat(b.operations[0].created_at) || 0
              : 0;
        return bTime - aTime;
      });

      setGroupedOperations(grouped);
      setOperations(filteredOps);

      // Auto-add new operations to visible set
      setVisibleOperations((prev) => {
        const next = new Set(prev);
        grouped.forEach((op) => {
          if (!prev.has(op.operation_id)) {
            next.add(op.operation_id);
          }
        });
        return next;
      });
    } catch (err) {
      console.error('Failed to load operations:', err);
      setOperations([]);
      setGroupedOperations([]);
    }
  }, [filterTypes]);

  // Listen for operation-started events and immediately add operationId to visibleOperations
  useEffect(() => {
    const handleOperationEvent = (event: Event) => {
      const customEvent = event as CustomEvent<{ operationId?: string }>;
      const operationId = customEvent.detail?.operationId;
      if (operationId) {
        console.log(
          '[OperationsModal] Operation started event received with operationId:',
          operationId,
        );
        // Immediately add to visibleOperations so it shows up even if it completes quickly
        setVisibleOperations((prev) => {
          const next = new Set(prev);
          next.add(operationId);
          console.log(
            '[OperationsModal] Added operationId to visibleOperations:',
            operationId,
            'Total visible:',
            next.size,
          );
          return next;
        });
      } else {
        console.warn(
          '[OperationsModal] Operation started event received but no operationId in detail:',
          customEvent.detail,
        );
      }
    };

    // Listen to all operation events
    const events = [
      'copy-started',
      'move-started',
      'upload-started',
      'download-started',
      'delete-started',
    ];
    events.forEach((eventName) => {
      window.addEventListener(eventName, handleOperationEvent);
    });

    return () => {
      events.forEach((eventName) => {
        window.removeEventListener(eventName, handleOperationEvent);
      });
    };
  }, []);

  useEffect(() => {
    if (isVisible) {
      loadOperations();
      const interval = setInterval(loadOperations, POLLING_INTERVALS.NORMAL);
      return () => clearInterval(interval);
    }
  }, [isVisible, loadOperations]);

  // Use standardized operation tracking hook
  // This handles all operation-started events and provides consistent polling
  useOperationTracking({
    onOperationStarted: loadOperations,
    pollingInterval: POLLING_INTERVALS.NORMAL,
    enabled: isVisible,
    immediateRefresh: true,
    delayedRefreshes: [200, 500], // Keep existing delayed refresh pattern
  });

  const visibleOpsList = useMemo(() => {
    return groupedOperations.filter((op) =>
      visibleOperations.has(op.operation_id),
    );
  }, [groupedOperations, visibleOperations]);

  const activeOps = useMemo(() => {
    return visibleOpsList.filter((op) => op.has_in_progress || op.has_paused);
  }, [visibleOpsList]);

  const completedOps = useMemo(() => {
    return visibleOpsList.filter(
      (op) => op.status === 'Completed' || op.status === 'Failed',
    );
  }, [visibleOpsList]);

  const hasActiveOperations = activeOps.length > 0;

  // Group operations by type for display
  const operationsByType = useMemo(() => {
    const grouped = new Map<OperationType, GroupedOperation[]>();
    const typeOrder: OperationType[] = [
      'Upload',
      'Download',
      'Copy',
      'Move',
      'Paste',
      'Delete',
      'Rename',
      'CreateDir',
      'RemoveDir',
      'TierChange',
      'Transcribe',
    ];

    visibleOpsList.forEach((op) => {
      if (!grouped.has(op.operation_type)) {
        grouped.set(op.operation_type, []);
      }
      const opsList = grouped.get(op.operation_type);
      if (opsList) {
        opsList.push(op);
      }
    });

    // Sort within each type
    grouped.forEach((ops) => {
      ops.sort((a, b) => {
        const aActive = a.has_in_progress || a.has_paused;
        const bActive = b.has_in_progress || b.has_paused;
        if (aActive !== bActive) return aActive ? -1 : 1;
        return 0;
      });
    });

    return Array.from(grouped.entries()).sort(([typeA], [typeB]) => {
      const indexA = typeOrder.indexOf(typeA);
      const indexB = typeOrder.indexOf(typeB);
      if (indexA === -1 && indexB === -1) return typeA.localeCompare(typeB);
      if (indexA === -1) return 1;
      if (indexB === -1) return -1;
      return indexA - indexB;
    });
  }, [visibleOpsList]);

  const handlePauseOperation = async (op: GroupedOperation) => {
    if (op.operation_type !== 'Upload') return;

    // Get uploads for this operation
    const operationUploads = uploads.filter(
      (u) => u.operation_id === op.operation_id,
    );
    for (const upload of operationUploads) {
      if (upload.status === 'InProgress' || upload.status === 'Pending') {
        try {
          await invoke('vfs_pause_upload', { uploadId: upload.upload_id });
        } catch (err) {
          console.error(`Failed to pause upload ${upload.upload_id}:`, err);
        }
      }
    }
    loadOperations();
  };

  const handleResumeOperation = async (op: GroupedOperation) => {
    if (op.operation_type !== 'Upload') return;

    // Get uploads for this operation
    const operationUploads = uploads.filter(
      (u) => u.operation_id === op.operation_id,
    );
    for (const upload of operationUploads) {
      if (upload.status === 'Paused') {
        try {
          await invoke('vfs_resume_upload', { uploadId: upload.upload_id });
        } catch (err) {
          console.error(`Failed to resume upload ${upload.upload_id}:`, err);
        }
      }
    }
    loadOperations();
  };

  const handleCancelOperation = async (op: GroupedOperation) => {
    // Cancel operation based on type
    if (op.operation_type === 'Upload') {
      const operationUploads = uploads.filter(
        (u) => u.operation_id === op.operation_id,
      );
      for (const upload of operationUploads) {
        try {
          await invoke('vfs_cancel_upload', { uploadId: upload.upload_id });
        } catch (err) {
          console.error(`Failed to cancel upload ${upload.upload_id}:`, err);
        }
      }
    }
    // For other types, we can delete the operation
    try {
      await invoke('vfs_delete_operation', { operationId: op.operation_id });
    } catch (err) {
      console.error(`Failed to delete operation ${op.operation_id}:`, err);
    }
    loadOperations();
  };

  const handleCloseOperation = async (op: GroupedOperation) => {
    try {
      await invoke('vfs_delete_operation', { operationId: op.operation_id });

      // Also remove uploads if it's an upload operation
      if (op.operation_type === 'Upload') {
        const operationUploads = uploads.filter(
          (u) => u.operation_id === op.operation_id,
        );
        for (const upload of operationUploads) {
          try {
            await invoke('vfs_remove_upload', { uploadId: upload.upload_id });
          } catch (err) {
            console.error(`Failed to remove upload ${upload.upload_id}:`, err);
          }
        }
      }

      setVisibleOperations((prev) => {
        const next = new Set(prev);
        next.delete(op.operation_id);
        return next;
      });

      loadOperations();
    } catch (err) {
      console.error(`Failed to close operation ${op.operation_id}:`, err);
    }
  };

  const handleDismissAll = async () => {
    const completedOps = visibleOpsList.filter(
      (op) => op.status === 'Completed' || op.status === 'Failed',
    );

    for (const op of completedOps) {
      try {
        await invoke('vfs_delete_operation', { operationId: op.operation_id });

        // Also remove uploads if it's an upload operation
        if (op.operation_type === 'Upload') {
          const operationUploads = uploads.filter(
            (u) => u.operation_id === op.operation_id,
          );
          for (const upload of operationUploads) {
            try {
              await invoke('vfs_remove_upload', { uploadId: upload.upload_id });
            } catch (err) {
              console.error(
                `Failed to remove upload ${upload.upload_id}:`,
                err,
              );
            }
          }
        }
      } catch (err) {
        console.error(`Failed to delete operation ${op.operation_id}:`, err);
      }
    }

    setVisibleOperations((prev) => {
      const next = new Set(prev);
      completedOps.forEach((op) => next.delete(op.operation_id));
      return next;
    });

    loadOperations();
  };

  const getFileName = (path: string): string => {
    return path.split('/').pop() || path.split('\\').pop() || path;
  };

  const formatBytes = (bytes: number | undefined | null): string => {
    if (bytes === undefined || bytes === null || isNaN(bytes)) return '0 B';
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  };

  const getProgressPercentage = (bytes: number, total: number): number => {
    if (!bytes || !total || total === 0) return 0;
    return Math.round((bytes / total) * 100);
  };

  if (!isVisible) return null;

  return (
    <div className="operations-modal-overlay" onClick={onClose}>
      <div className="operations-modal" onClick={(e) => e.stopPropagation()}>
        <div className="operations-modal-header">
          <div className="operations-modal-header-left">
            <h3>Operations</h3>
            {hasActiveOperations && (
              <span
                className="operations-modal-active-indicator"
                title="Active operations"
              >
                <span className="operations-modal-pulse-dot" />
              </span>
            )}
          </div>
          <div className="operations-modal-header-right">
            {completedOps.length > 0 && (
              <button
                className="operations-modal-dismiss-all"
                onClick={handleDismissAll}
                title="Dismiss all completed operations"
              >
                Dismiss All
              </button>
            )}
            {onClose && (
              <button
                className="operations-modal-close"
                onClick={onClose}
                title="Close"
              >
                ×
              </button>
            )}
          </div>
        </div>

        <div className="operations-modal-tabs">
          <button
            className={`operations-modal-tab ${activeTab === 'active' ? 'active' : ''}`}
            onClick={() => setActiveTab('active')}
          >
            Active ({activeOps.length})
          </button>
          <button
            className={`operations-modal-tab ${activeTab === 'history' ? 'active' : ''}`}
            onClick={() => setActiveTab('history')}
          >
            History ({completedOps.length})
          </button>
        </div>

        <div className="operations-modal-content">
          {activeTab === 'active' ? (
            <>
              {activeOps.length === 0 ? (
                <div className="operations-modal-empty">
                  <p>No active operations</p>
                </div>
              ) : (
                operationsByType.map(([operationType, ops]) => {
                  const config = OPERATION_TYPE_CONFIGS[operationType];
                  if (!config || ops.length === 0) return null;

                  return (
                    <div
                      key={operationType}
                      className="operations-modal-section"
                    >
                      <h4 className="operations-modal-section-title">
                        {config.icon} {config.label}
                        <span className="operations-modal-section-count">
                          ({ops.length})
                        </span>
                      </h4>
                      {ops.map((op) => {
                        const isOpExpanded = expandedOperations.has(
                          op.operation_id,
                        );
                        const toggleOpExpand = () => {
                          setExpandedOperations((prev) => {
                            const next = new Set(prev);
                            if (next.has(op.operation_id)) {
                              next.delete(op.operation_id);
                            } else {
                              next.add(op.operation_id);
                            }
                            return next;
                          });
                        };

                        const percentage = getProgressPercentage(
                          op.bytes_processed,
                          op.total_size,
                        );
                        const config =
                          OPERATION_TYPE_CONFIGS[op.operation_type];

                        return (
                          <div
                            key={op.operation_id}
                            className="operations-modal-item"
                          >
                            <div className="operations-modal-item-header">
                              <button
                                className="operations-modal-item-expand-btn"
                                onClick={toggleOpExpand}
                                title={isOpExpanded ? 'Collapse' : 'Expand'}
                              >
                                {isOpExpanded ? '▼' : '▶'}
                              </button>
                              <span className="operations-modal-item-name">
                                {op.file_count > 1
                                  ? `${op.file_count} files`
                                  : getFileName(op.source_path)}
                              </span>
                              <div className="operations-modal-item-progress">
                                {op.status === 'Completed' &&
                                percentage >= 100 ? (
                                  <span className="operations-modal-status-icon">
                                    ✓
                                  </span>
                                ) : op.status === 'Failed' ? (
                                  <span className="operations-modal-status-icon failed">
                                    ✕
                                  </span>
                                ) : (
                                  <span className="operations-modal-item-percentage">
                                    {percentage}%
                                  </span>
                                )}
                              </div>
                              {/* Action buttons based on operation type config */}
                              {config.supportsPauseResume &&
                                op.has_in_progress &&
                                !op.has_paused && (
                                  <button
                                    className="operations-modal-item-action-btn pause"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      handlePauseOperation(op);
                                    }}
                                    title="Pause all"
                                  >
                                    ⏸
                                  </button>
                                )}
                              {config.supportsPauseResume && op.has_paused && (
                                <button
                                  className="operations-modal-item-action-btn resume"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    handleResumeOperation(op);
                                  }}
                                  title="Resume all"
                                >
                                  ▶
                                </button>
                              )}
                              {config.supportsCancel &&
                                (op.has_in_progress || op.has_paused) && (
                                  <button
                                    className="operations-modal-item-action-btn cancel"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      handleCancelOperation(op);
                                    }}
                                    title="Cancel"
                                  >
                                    ✕
                                  </button>
                                )}
                            </div>
                            <div className="operations-modal-item-details">
                              <span>
                                {formatBytes(op.bytes_processed)} /{' '}
                                {formatBytes(op.total_size)}
                              </span>
                            </div>
                            {op.status !== 'Completed' &&
                              op.status !== 'Failed' && (
                                <div className="operations-modal-progress-bar">
                                  <div
                                    className="operations-modal-progress-fill"
                                    style={{
                                      width: `${Math.min(100, Math.max(0, percentage))}%`,
                                    }}
                                  />
                                </div>
                              )}
                            {isOpExpanded &&
                              op.operations[0].files &&
                              op.operations[0].files.length > 0 && (
                                <div className="operations-modal-item-files">
                                  {op.operations[0].files.map((file, idx) => {
                                    const filePercentage =
                                      getProgressPercentage(
                                        file.bytes_processed,
                                        file.file_size,
                                      );
                                    return (
                                      <div
                                        key={`${file.remote_path}-${idx}`}
                                        className="operations-modal-file"
                                      >
                                        <span className="operations-modal-file-name">
                                          {getFileName(file.remote_path)}
                                        </span>
                                        <span className="operations-modal-file-progress">
                                          {filePercentage}% (
                                          {formatBytes(file.bytes_processed)} /{' '}
                                          {formatBytes(file.file_size)})
                                        </span>
                                      </div>
                                    );
                                  })}
                                </div>
                              )}
                            {(op.status === 'Completed' ||
                              op.status === 'Failed') && (
                              <button
                                className="operations-modal-item-close"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  handleCloseOperation(op);
                                }}
                                title="Close"
                              >
                                ×
                              </button>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  );
                })
              )}
            </>
          ) : (
            <>
              {completedOps.length === 0 ? (
                <div className="operations-modal-empty">
                  <p>No operation history</p>
                </div>
              ) : (
                operationsByType.map(([operationType, ops]) => {
                  const config = OPERATION_TYPE_CONFIGS[operationType];
                  if (!config || ops.length === 0) return null;

                  const completedOpsOfType = ops.filter(
                    (op) => op.status === 'Completed' || op.status === 'Failed',
                  );

                  if (completedOpsOfType.length === 0) return null;

                  return (
                    <div
                      key={operationType}
                      className="operations-modal-section"
                    >
                      <h4 className="operations-modal-section-title">
                        {config.icon} {config.label}
                        <span className="operations-modal-section-count">
                          ({completedOpsOfType.length})
                        </span>
                      </h4>
                      {completedOpsOfType.map((op) => {
                        const isOpExpanded = expandedOperations.has(
                          op.operation_id,
                        );
                        const toggleOpExpand = () => {
                          setExpandedOperations((prev) => {
                            const next = new Set(prev);
                            if (next.has(op.operation_id)) {
                              next.delete(op.operation_id);
                            } else {
                              next.add(op.operation_id);
                            }
                            return next;
                          });
                        };

                        return (
                          <div
                            key={op.operation_id}
                            className="operations-modal-item"
                          >
                            <div className="operations-modal-item-header">
                              <button
                                className="operations-modal-item-expand-btn"
                                onClick={toggleOpExpand}
                                title={isOpExpanded ? 'Collapse' : 'Expand'}
                              >
                                {isOpExpanded ? '▼' : '▶'}
                              </button>
                              <span className="operations-modal-item-name">
                                {op.file_count > 1
                                  ? `${op.file_count} files`
                                  : getFileName(op.source_path)}
                              </span>
                              <div className="operations-modal-item-progress">
                                {op.status === 'Completed' ? (
                                  <span className="operations-modal-status-icon">
                                    ✓
                                  </span>
                                ) : (
                                  <span className="operations-modal-status-icon failed">
                                    ✕
                                  </span>
                                )}
                              </div>
                              <button
                                className="operations-modal-item-close"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  handleCloseOperation(op);
                                }}
                                title="Close"
                              >
                                ×
                              </button>
                            </div>
                            {isOpExpanded &&
                              op.operations[0].files &&
                              op.operations[0].files.length > 0 && (
                                <div className="operations-modal-item-files">
                                  {op.operations[0].files.map((file, idx) => (
                                    <div
                                      key={`${file.remote_path}-${idx}`}
                                      className="operations-modal-file"
                                    >
                                      <span className="operations-modal-file-name">
                                        {getFileName(file.remote_path)}
                                      </span>
                                      <span className="operations-modal-file-size">
                                        {formatBytes(file.file_size)}
                                      </span>
                                    </div>
                                  ))}
                                </div>
                              )}
                          </div>
                        );
                      })}
                    </div>
                  );
                })
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
};

export default OperationsModal;
