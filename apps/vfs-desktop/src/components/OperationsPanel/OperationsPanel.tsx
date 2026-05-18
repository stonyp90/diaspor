/**
 * Unified OperationsPanel Component
 *
 * Compact, timeline-based view of all operations sorted by timestamp.
 * Active and completed operations shown together with expand for details.
 */
import React, {
  useEffect,
  useState,
  useCallback,
  useMemo,
  useRef,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DialogService } from '../../services/dialog/dialog.service';
import { getProviderInfo, type StorageSource } from '../../types/storage';
import type { OperationType } from '../../types/operations';
import './OperationsPanel.css';

interface OperationsPanelProps {
  operationTypes?: OperationType[];
  onViewDetails?: () => void;
}

interface Operation {
  operation_id: string;
  operation_type: string;
  source_id: string;
  source_path: string;
  destination_path?: string;
  file_size?: number;
  bytes_processed: number;
  status: string;
  file_count?: number;
  error?: string;
  files?: Array<{
    local_path: string;
    remote_path: string;
    file_size: number;
    bytes_processed: number;
    status?: string;
    error?: string;
  }>;
  created_at?: string | number;
  completed_at?: string | number;
}

interface UploadState {
  upload_id: string;
  operation_id?: string;
  status: string;
}

interface UnifiedOperation {
  operation_id: string;
  operation_type: OperationType;
  operations: Operation[];
  total_size: number;
  bytes_processed: number;
  file_count: number;
  status: string;
  is_active: boolean;
  source_path: string;
  destination_path?: string;
  source_id: string;
  storage_provider?: string;
  timestamp: number; // For sorting
  error?: string;
}

// Helper functions
const getFileName = (path: string): string => {
  if (!path) return '';
  const parts = path.replace(/\\/g, '/').split('/').filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : path;
};

const formatBytes = (bytes: number | undefined | null): string => {
  if (!bytes || isNaN(bytes)) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(i > 0 ? 1 : 0)} ${sizes[i]}`;
};

const formatTime = (ts: string | number | undefined): string => {
  if (!ts) return '';
  const time = typeof ts === 'number' ? ts : parseFloat(ts) || 0;
  const ms = time < 1e12 ? time * 1000 : time;
  const diff = Date.now() - ms;
  if (diff < 60000) return 'now';
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h`;
  return `${Math.floor(diff / 86400000)}d`;
};

const getIcon = (type: OperationType): string => {
  const icons: Record<string, string> = {
    Upload: '↑',
    Download: '↓',
    Delete: '×',
    Copy: '⧉',
    Move: '→',
    Rename: '✎',
    Transcribe: '🎤',
    AutoTag: '🏷️',
    Transcode: '🎬',
    TierChange: '📦',
  };
  return icons[type] || '•';
};

const AUTO_DISMISS_DELAY = 5000;
const SOURCES_CACHE_TTL = 10000;

export const OperationsPanel: React.FC<OperationsPanelProps> = ({
  operationTypes,
  onViewDetails,
}) => {
  // ========== ALL HOOKS AT TOP ==========
  const [uploads, setUploads] = useState<UploadState[]>([]);
  const [operations, setOperations] = useState<UnifiedOperation[]>([]);
  const [optimisticOps, setOptimisticOps] = useState<Map<string, UnifiedOperation>>(new Map());
  const [visibleIds, setVisibleIds] = useState<Set<string>>(new Set());
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [isOpen, setIsOpen] = useState(false);

  const sourcesCacheRef = useRef<Map<string, StorageSource>>(new Map());
  const sourcesCacheTimeRef = useRef<number>(0);
  const pollRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const opsRef = useRef<UnifiedOperation[]>([]);

  const loadOperations = useCallback(async () => {
    try {
      const opList = await invoke<Operation[]>('vfs_list_operations').catch(
        () => [],
      );
      const filtered = operationTypes
        ? (opList || []).filter((op) =>
            operationTypes.includes(op.operation_type as OperationType),
          )
        : opList || [];

      const uploadList = await invoke<UploadState[]>('vfs_list_uploads').catch(
        () => [],
      );
      setUploads(uploadList);

      const now = Date.now();
      let sourceMap = sourcesCacheRef.current;
      if (
        sourceMap.size === 0 ||
        now - sourcesCacheTimeRef.current > SOURCES_CACHE_TTL
      ) {
        const sources = await invoke<StorageSource[]>('vfs_list_sources').catch(
          () => [],
        );
        sourceMap = new Map((sources || []).map((s) => [s.id, s]));
        sourcesCacheRef.current = sourceMap;
        sourcesCacheTimeRef.current = now;
      }

      const byId = new Map<string, Operation[]>();
      for (const op of filtered) {
        if (op.operation_id) {
          if (!byId.has(op.operation_id)) byId.set(op.operation_id, []);
          const ops = byId.get(op.operation_id);
          if (ops) {
            ops.push(op);
          }
        }
      }

      const unified: UnifiedOperation[] = [];
      for (const [opId, ops] of byId.entries()) {
        const op = ops[0];
        const source = sourceMap.get(op.source_id);

        const files: { file_size: number; bytes_processed: number }[] = [];
        for (const o of ops) {
          if (o.files?.length) files.push(...o.files);
          else
            files.push({
              file_size: o.file_size || 0,
              bytes_processed: o.bytes_processed,
            });
        }

        const total_size =
          files.reduce((s, f) => s + f.file_size, 0) ||
          ops.reduce((s, o) => s + (o.file_size || 0), 0);
        const bytes_processed =
          files.reduce((s, f) => s + f.bytes_processed, 0) ||
          ops.reduce((s, o) => s + o.bytes_processed, 0);
        const file_count = files.length || op.file_count || ops.length;
        const statuses = ops.map((o) => o.status);
        const is_active = statuses.some(
          (s) => s === 'InProgress' || s === 'Pending' || s === 'Paused',
        );

        let status = 'Completed';
        if (statuses.some((s) => s === 'InProgress' || s === 'Pending'))
          status = 'InProgress';
        else if (statuses.some((s) => s === 'Paused')) status = 'Paused';
        else if (statuses.some((s) => s === 'Canceled')) status = 'Canceled';
        else if (statuses.some((s) => s === 'Failed')) status = 'Failed';

        // Get timestamp for sorting
        let timestamp = 0;
        if (is_active && op.created_at) {
          timestamp =
            typeof op.created_at === 'number'
              ? op.created_at
              : parseFloat(op.created_at) || 0;
        } else if (op.completed_at) {
          timestamp =
            typeof op.completed_at === 'number'
              ? op.completed_at
              : parseFloat(op.completed_at) || 0;
        } else if (op.created_at) {
          timestamp =
            typeof op.created_at === 'number'
              ? op.created_at
              : parseFloat(op.created_at) || 0;
        }
        if (timestamp < 1e12) timestamp *= 1000;

        let storage_provider = 'Unknown';
        if (source) {
          const info = getProviderInfo(source);
          storage_provider =
            info?.name || source.name || source.providerId || 'Unknown';
        }

        const error = ops.find((o) => o.error)?.error;

        unified.push({
          operation_id: opId,
          operation_type: op.operation_type as OperationType,
          operations: ops,
          total_size,
          bytes_processed,
          file_count,
          status,
          is_active,
          source_path: op.source_path,
          destination_path: op.destination_path,
          source_id: op.source_id,
          storage_provider,
          timestamp,
          error,
        });
      }

      // Sort by timestamp descending (most recent first)
      unified.sort((a, b) => b.timestamp - a.timestamp);
      setOperations(unified);

      // Remove optimistic operations that now have real data
      setOptimisticOps((prev) => {
        const next = new Map(prev);
        unified.forEach((op) => {
          if (next.has(op.operation_id)) {
            next.delete(op.operation_id);
          }
        });
        return next;
      });

      // Update visible IDs - show active operations and recently completed (within AUTO_DISMISS_DELAY)
      // This allows completed operations to show briefly before auto-dismissing
      setVisibleIds((prev) => {
        const next = new Set(prev);
        unified.forEach((op) => {
          if (op.is_active) {
            next.add(op.operation_id);
          } else {
            // Show completed operations briefly (within AUTO_DISMISS_DELAY) before auto-dismissing
            const age = now - op.timestamp;
            if (age < AUTO_DISMISS_DELAY) {
              next.add(op.operation_id);
            }
          }
        });
        return next;
      });
    } catch (err) {
      console.error('Failed to load operations:', err);
      setOperations([]);
    }
  }, [operationTypes]);

  const visible = useMemo(() => {
    // Merge real operations with optimistic operations
    const realOps = operations.filter((op) => visibleIds.has(op.operation_id));
    const optimisticArray = Array.from(optimisticOps.values()).filter(
      (op) => visibleIds.has(op.operation_id) && !operations.some((realOp) => realOp.operation_id === op.operation_id)
    );
    return [...optimisticArray, ...realOps];
  }, [operations, visibleIds, optimisticOps]);
  const activeCount = useMemo(
    () => visible.filter((op) => op.is_active).length,
    [visible],
  );
  const completedCount = useMemo(
    () => visible.filter((op) => !op.is_active).length,
    [visible],
  );

  const overallProgress = useMemo(() => {
    const active = visible.filter((op) => op.is_active);
    if (active.length === 0) return 100;
    const total = active.reduce((s, op) => s + op.total_size, 0);
    const processed = active.reduce((s, op) => s + op.bytes_processed, 0);
    return total > 0 ? Math.round((processed / total) * 100) : 0;
  }, [visible]);

  useEffect(() => {
    opsRef.current = operations;
  }, [operations]);

  useEffect(() => {
    let mounted = true;
    const poll = () => {
      if (!mounted) return;
      if (pollRef.current) clearTimeout(pollRef.current);
      const hasActive = opsRef.current.some((op) => op.is_active);
      
      // For Transcribe/AutoTag operations, poll more frequently (500ms)
      // These are fast operations where we need to catch progress updates
      const hasQuickOps = opsRef.current.some(
        (op) => op.is_active && (op.operation_type === 'Transcribe' || op.operation_type === 'AutoTag')
      );
      
      const interval = hasQuickOps
        ? 500  // Fast polling for quick operations
        : hasActive
          ? 1000  // Normal polling for active operations
          : opsRef.current.length === 0
            ? 10000  // Slow polling when idle
            : 5000;  // Medium polling for recently completed
      
      pollRef.current = setTimeout(() => {
        if (mounted) loadOperations().finally(poll);
      }, interval);
    };
    loadOperations().finally(() => {
      if (mounted) poll();
    });
    return () => {
      mounted = false;
      if (pollRef.current) clearTimeout(pollRef.current);
    };
  }, [loadOperations]);

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      const opId = detail?.operationId;
      if (opId) {
        // Extract operation type from event name
        const eventType = (e as Event).type;
        const operationType = eventType.replace('-started', '');
        const operationTypeMap: Record<string, OperationType> = {
          'copy': 'Copy',
          'move': 'Move',
          'rename': 'Rename',
          'delete': 'Delete',
          'mkdir': 'CreateDir',
          'rmdir': 'RemoveDir',
          'upload': 'Upload',
          'download': 'Download',
          'paste': 'Paste',
          'transcribe': 'Transcribe',
          'autotag': 'AutoTag',
          'transcode': 'Transcode',
        };

        // Create optimistic operation immediately
        const optimisticOp: UnifiedOperation = {
          operation_id: opId,
          operation_type: operationTypeMap[operationType] || 'Copy',
          operations: [],
          total_size: detail.fileSize || 100, // Default to 100 for percentage calculations
          bytes_processed: 0,
          file_count: 1,
          status: 'InProgress',
          is_active: true,
          source_path: detail.filePath || detail.fileName || 'Processing...',
          destination_path: detail.destinationPath,
          source_id: detail.sourceId || '',
          storage_provider: detail.storageProvider,
          timestamp: Date.now(),
        };

        setOptimisticOps((prev) => {
          const next = new Map(prev);
          next.set(opId, optimisticOp);
          return next;
        });

        setVisibleIds((prev) => new Set(prev).add(opId));
        setIsOpen(true);
        
        // Load real data from backend
        loadOperations();
        setTimeout(loadOperations, 100);
        setTimeout(loadOperations, 500);
      }
    };
    const events = [
      'copy-started',
      'move-started',
      'rename-started',
      'delete-started',
      'mkdir-started',
      'rmdir-started',
      'upload-started',
      'download-started',
      'paste-started',
      'transcribe-started',
      'autotag-started',
      'transcode-started',
    ];
    events.forEach((e) => window.addEventListener(e, handler));
    return () => events.forEach((e) => window.removeEventListener(e, handler));
  }, [loadOperations]);

  useEffect(() => {
    const completed = visible.filter((op) => !op.is_active);
    if (completed.length === 0) return;
    const timers = completed.map((op) =>
      setTimeout(
        () =>
          setVisibleIds((prev) => {
            const n = new Set(prev);
            n.delete(op.operation_id);
            return n;
          }),
        AUTO_DISMISS_DELAY,
      ),
    );
    return () => timers.forEach(clearTimeout);
  }, [visible]);

  // ========== END HOOKS ==========

  const pause = async (op: UnifiedOperation) => {
    if (op.operation_type !== 'Upload') return;
    for (const u of uploads.filter(
      (u) =>
        u.operation_id === op.operation_id &&
        (u.status === 'InProgress' || u.status === 'Pending'),
    )) {
      await invoke('vfs_pause_upload', { uploadId: u.upload_id }).catch(
        console.error,
      );
    }
    loadOperations();
  };

  const resume = async (op: UnifiedOperation) => {
    if (op.operation_type !== 'Upload') return;
    for (const u of uploads.filter(
      (u) => u.operation_id === op.operation_id && u.status === 'Paused',
    )) {
      await invoke('vfs_resume_upload', { uploadId: u.upload_id }).catch(
        console.error,
      );
    }
    loadOperations();
  };

  const cancel = async (op: UnifiedOperation) => {
    await invoke('vfs_cancel_operation', {
      operationId: op.operation_id,
    }).catch(console.error);
    loadOperations();
  };

  const restart = async (op: UnifiedOperation) => {
    try {
      await invoke('vfs_restart_operation', { operationId: op.operation_id });
      loadOperations();
    } catch (err) {
      DialogService.error(
        `Failed to restart: ${err instanceof Error ? err.message : String(err)}`,
        'Error',
      );
    }
  };

  const dismiss = async (op: UnifiedOperation) => {
    setVisibleIds((prev) => {
      const n = new Set(prev);
      n.delete(op.operation_id);
      return n;
    });
    await invoke('vfs_delete_operation', {
      operationId: op.operation_id,
    }).catch(console.error);
    loadOperations();
  };

  const dismissAll = async () => {
    const toDel = visible.filter((op) => !op.is_active);
    setVisibleIds((prev) => {
      const n = new Set(prev);
      toDel.forEach((op) => n.delete(op.operation_id));
      return n;
    });
    await Promise.all(
      toDel.map((op) =>
        invoke('vfs_delete_operation', { operationId: op.operation_id }).catch(
          console.error,
        ),
      ),
    );
    loadOperations();
  };

  const toggle = (id: string) =>
    setExpandedIds((prev) => {
      const n = new Set(prev);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  if (visible.length === 0 && visibleIds.size === 0) return null;

  // Only hide the entire panel if there are no visible operations
  if (visible.length === 0) {
    return null;
  }

  return (
    <div
      className={`ops ${activeCount > 0 ? 'ops--active' : ''} ${isOpen ? 'ops--open' : ''}`}
    >
      {/* Header */}
      <div className="ops__hdr" onClick={() => setIsOpen(!isOpen)}>
        <div className="ops__hdr-l">
          {activeCount > 0 && <span className="ops__dot" />}
          <span className="ops__title">
            {activeCount > 0
              ? `${activeCount} active`
              : completedCount > 0
                ? `${completedCount} done`
                : 'Operations'}
          </span>
          {activeCount > 0 && !isOpen && (
            <span className="ops__pct">{overallProgress}%</span>
          )}
        </div>
        <div className="ops__hdr-r">
          {completedCount > 0 && isOpen && (
            <button
              className="ops__clear"
              onClick={(e) => {
                e.stopPropagation();
                dismissAll();
              }}
            >
              Clear
            </button>
          )}
          {onViewDetails && (
            <button
              className="ops__all"
              onClick={(e) => {
                e.stopPropagation();
                onViewDetails();
              }}
            >
              All
            </button>
          )}
          <button
            className="ops__tog"
            onClick={(e) => {
              e.stopPropagation();
              setIsOpen(!isOpen);
            }}
          >
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              style={{
                transform: isOpen ? 'rotate(180deg)' : 'none',
                transition: 'transform 0.15s',
              }}
            >
              <polyline points="2,3.5 5,6.5 8,3.5" />
            </svg>
          </button>
        </div>
      </div>

      {/* Mini progress bar when collapsed */}
      {!isOpen && activeCount > 0 && (
        <div className="ops__minibar">
          <div
            className="ops__minibar-fill"
            style={{ width: `${overallProgress}%` }}
          />
        </div>
      )}

      {/* Content */}
      {isOpen && (
        <div className="ops__body">
          {visible.map((op) => {
            const name =
              op.file_count > 1
                ? `${op.file_count} items`
                : getFileName(op.source_path);
            const pct =
              op.total_size > 0
                ? Math.min(
                    100,
                    Math.round((op.bytes_processed / op.total_size) * 100),
                  )
                : 0;
            const exp = expandedIds.has(op.operation_id);
            const isPaused = op.status === 'Paused';
            const isFailed = op.status === 'Failed';
            const isCanceled = op.status === 'Canceled';
            const isDone = op.status === 'Completed';

            return (
              <div
                key={op.operation_id}
                className={`op ${op.is_active ? 'op--active' : ''} ${isFailed ? 'op--fail' : ''} ${exp ? 'op--exp' : ''}`}
              >
                <div
                  className="op__row"
                  onClick={() => toggle(op.operation_id)}
                >
                  <span
                    className={`op__icon ${op.is_active ? 'op__icon--active' : isFailed ? 'op__icon--fail' : isDone ? 'op__icon--done' : ''}`}
                  >
                    {getIcon(op.operation_type)}
                  </span>
                  <span className="op__name" title={op.source_path}>
                    {name}
                  </span>

                  {/* Status indicator */}
                  {op.is_active ? (
                    <span className="op__pct">{pct}%</span>
                  ) : isDone ? (
                    <span className="op__stat op__stat--done">✓</span>
                  ) : isFailed ? (
                    <span className="op__stat op__stat--fail">!</span>
                  ) : isCanceled ? (
                    <span className="op__stat op__stat--cancel">—</span>
                  ) : null}

                  {/* Time */}
                  {!op.is_active && (
                    <span className="op__time">{formatTime(op.timestamp)}</span>
                  )}

                  {/* Actions */}
                  <div className="op__acts">
                    {op.is_active &&
                      op.operation_type === 'Upload' &&
                      isPaused && (
                        <button
                          className="op__btn op__btn--go"
                          onClick={(e) => {
                            e.stopPropagation();
                            resume(op);
                          }}
                          title="Resume"
                        >
                          ▶
                        </button>
                      )}
                    {op.is_active &&
                      op.operation_type === 'Upload' &&
                      !isPaused && (
                        <button
                          className="op__btn op__btn--pause"
                          onClick={(e) => {
                            e.stopPropagation();
                            pause(op);
                          }}
                          title="Pause"
                        >
                          ⏸
                        </button>
                      )}
                    {op.is_active && (
                      <button
                        className="op__btn op__btn--stop"
                        onClick={(e) => {
                          e.stopPropagation();
                          cancel(op);
                        }}
                        title="Cancel"
                      >
                        ⏹
                      </button>
                    )}
                    {(isFailed || isCanceled) && (
                      <button
                        className="op__btn op__btn--retry"
                        onClick={(e) => {
                          e.stopPropagation();
                          restart(op);
                        }}
                        title="Retry"
                      >
                        ↻
                      </button>
                    )}
                    <button
                      className="op__btn op__btn--x"
                      onClick={(e) => {
                        e.stopPropagation();
                        dismiss(op);
                      }}
                      title="Dismiss"
                    >
                      ×
                    </button>
                  </div>

                  <svg
                    className={`op__chev ${exp ? 'op__chev--open' : ''}`}
                    width="8"
                    height="8"
                    viewBox="0 0 8 8"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                  >
                    <polyline points="2.5,1.5 5.5,4 2.5,6.5" />
                  </svg>
                </div>

                {/* Progress bar for active */}
                {op.is_active && (
                  <div className="op__bar">
                    <div
                      className="op__bar-fill"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                )}

                {/* Expanded details */}
                {exp && (
                  <div className="op__det">
                    <div className="op__det-row">
                      <span className="op__det-lbl">Type:</span>
                      <span className="op__det-val">{op.operation_type}</span>
                    </div>
                    {op.total_size > 0 && (
                      <div className="op__det-row">
                        <span className="op__det-lbl">Size:</span>
                        <span className="op__det-val">
                          {formatBytes(op.bytes_processed)} /{' '}
                          {formatBytes(op.total_size)}
                        </span>
                      </div>
                    )}
                    {op.file_count > 1 && (
                      <div className="op__det-row">
                        <span className="op__det-lbl">Files:</span>
                        <span className="op__det-val">{op.file_count}</span>
                      </div>
                    )}
                    {op.destination_path && (
                      <div className="op__det-row">
                        <span className="op__det-lbl">To:</span>
                        <span
                          className="op__det-val op__det-val--path"
                          title={op.destination_path}
                        >
                          {getFileName(op.destination_path) ||
                            op.destination_path}
                        </span>
                      </div>
                    )}
                    {op.storage_provider &&
                      op.storage_provider !== 'Unknown' && (
                        <div className="op__det-row">
                          <span className="op__det-lbl">Provider:</span>
                          <span className="op__det-val">
                            {op.storage_provider}
                          </span>
                        </div>
                      )}
                    {op.error && (
                      <div className="op__det-row op__det-row--err">
                        <span className="op__det-lbl">Error:</span>
                        <span className="op__det-val op__det-val--err">
                          {op.error}
                        </span>
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

export default OperationsPanel;
