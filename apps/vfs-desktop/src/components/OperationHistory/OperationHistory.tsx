/**
 * OperationHistory - Ultra-compact timeline view
 *
 * All operations in a single list sorted by timestamp.
 * Lean, modern design with expandable details.
 */
import { useEffect, useState, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { POLLING_INTERVALS } from '../../utils/operationEvents';
import type { OperationType, OperationStatus } from '../../types/operations';
import './OperationHistory.css';

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
  created_at?: string;
  completed_at?: string;
}

interface OperationHistoryProps {
  limit?: number;
}

// Helpers
const getFileName = (path: string | undefined): string => {
  if (!path) return 'Unknown';
  const parts = path.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] || 'Unknown';
};

const formatBytes = (bytes: number | undefined): string => {
  if (!bytes || isNaN(bytes)) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(i > 0 ? 1 : 0)} ${sizes[i]}`;
};

const formatTime = (dateStr?: string): string => {
  if (!dateStr) return '';
  try {
    const date = new Date(dateStr);
    const diff = Date.now() - date.getTime();
    if (diff < 60000) return 'now';
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h`;
    if (diff < 604800000) return `${Math.floor(diff / 86400000)}d`;
    return date.toLocaleDateString();
  } catch {
    return '';
  }
};

const formatDuration = (start?: string, end?: string): string => {
  if (!start || !end) return '';
  try {
    const ms = new Date(end).getTime() - new Date(start).getTime();
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
    return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
  } catch {
    return '';
  }
};

const getIcon = (type: OperationType): string => {
  const icons: Record<string, string> = {
    Upload: '↑',
    Download: '↓',
    Delete: '×',
    Copy: '⧉',
    Move: '→',
    Rename: '✎',
    Paste: '📋',
    CreateDir: '+',
    RemoveDir: '−',
    TierChange: '◐',
    Transcribe: '♪',
  };
  return icons[type] || '•';
};

export function OperationHistory({ limit = 100 }: OperationHistoryProps) {
  const [operations, setOperations] = useState<Operation[]>([]);
  const [loading, setLoading] = useState(true);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [filter, setFilter] = useState<'all' | 'completed' | 'failed'>('all');
  const [deletingIds, setDeletingIds] = useState<Set<string>>(new Set());

  const loadHistory = useCallback(async () => {
    try {
      setLoading(true);
      const result = await invoke<Operation[]>('vfs_get_audit_history', {
        limit,
      });

      // Deduplicate by operation_id
      const map = new Map<string, Operation>();
      result.forEach((op) => {
        const existing = map.get(op.operation_id);
        if (
          !existing ||
          (op.files?.length || 0) > (existing.files?.length || 0)
        ) {
          map.set(op.operation_id, op);
        }
      });

      setOperations(Array.from(map.values()));
    } catch (err) {
      console.error('Failed to load history:', err);
      setOperations([]);
    } finally {
      setLoading(false);
    }
  }, [limit]);

  useEffect(() => {
    loadHistory();
    const interval = setInterval(loadHistory, POLLING_INTERVALS.SLOW);
    return () => clearInterval(interval);
  }, [loadHistory]);

  useEffect(() => {
    const handler = () => loadHistory();
    const events = [
      'copy-started',
      'move-started',
      'rename-started',
      'delete-started',
      'mkdir-started',
      'rmdir-started',
      'upload-started',
      'download-started',
    ];
    events.forEach((e) => window.addEventListener(e, handler));
    return () => events.forEach((e) => window.removeEventListener(e, handler));
  }, [loadHistory]);

  // Sort by timestamp (most recent first)
  const sorted = useMemo(() => {
    return [...operations].sort((a, b) => {
      const aTime = a.completed_at || a.created_at || '';
      const bTime = b.completed_at || b.created_at || '';
      return new Date(bTime).getTime() - new Date(aTime).getTime();
    });
  }, [operations]);

  // Filter
  const filtered = useMemo(() => {
    if (filter === 'completed')
      return sorted.filter((op) => op.status === 'Completed');
    if (filter === 'failed')
      return sorted.filter((op) => op.status === 'Failed');
    return sorted;
  }, [sorted, filter]);

  const toggle = (id: string) =>
    setExpandedIds((prev) => {
      const n = new Set(prev);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  const deleteOp = async (id: string) => {
    setDeletingIds((prev) => new Set(prev).add(id));
    try {
      await invoke('vfs_delete_operation', { operation_id: id });
      setOperations((prev) => prev.filter((op) => op.operation_id !== id));
      setExpandedIds((prev) => {
        const n = new Set(prev);
        n.delete(id);
        return n;
      });
    } catch (err) {
      console.error('Failed to delete:', err);
    } finally {
      setDeletingIds((prev) => {
        const n = new Set(prev);
        n.delete(id);
        return n;
      });
    }
  };

  if (loading && operations.length === 0) {
    return (
      <div className="hist">
        <div className="hist__empty">Loading...</div>
      </div>
    );
  }

  return (
    <div className="hist">
      {/* Header */}
      <div className="hist__hdr">
        <h2 className="hist__title">History</h2>
        <button className="hist__refresh" onClick={loadHistory} title="Refresh">
          ↻
        </button>
      </div>

      {/* Filters */}
      <div className="hist__filters">
        <div className="hist__filter-group">
          {(['all', 'completed', 'failed'] as const).map((f) => (
            <button
              key={f}
              className={`hist__filter ${filter === f ? 'hist__filter--on' : ''}`}
              onClick={() => setFilter(f)}
            >
              {f === 'all' ? 'All' : f === 'completed' ? '✓ Done' : '! Failed'}
            </button>
          ))}
        </div>
        <span className="hist__count">
          {filtered.length} of {operations.length}
        </span>
      </div>

      {/* List */}
      <div className="hist__body">
        {filtered.length === 0 ? (
          <div className="hist__empty">No operations found</div>
        ) : (
          filtered.map((op) => {
            const exp = expandedIds.has(op.operation_id);
            const deleting = deletingIds.has(op.operation_id);
            const name =
              op.file_count && op.file_count > 1
                ? `${op.file_count} items`
                : getFileName(op.source_path);
            const pct =
              op.file_size && op.file_size > 0
                ? Math.round((op.bytes_processed / op.file_size) * 100)
                : 0;
            const isDone = op.status === 'Completed';
            const isFail = op.status === 'Failed';
            const isActive =
              op.status === 'InProgress' || op.status === 'Pending';

            return (
              <div
                key={op.operation_id}
                className={`hop ${exp ? 'hop--exp' : ''} ${isFail ? 'hop--fail' : ''} ${isActive ? 'hop--active' : ''}`}
              >
                <div
                  className="hop__row"
                  onClick={() => toggle(op.operation_id)}
                >
                  <span
                    className={`hop__icon ${isDone ? 'hop__icon--done' : isFail ? 'hop__icon--fail' : isActive ? 'hop__icon--active' : ''}`}
                  >
                    {getIcon(op.operation_type)}
                  </span>
                  <div className="hop__main">
                    <div className="hop__top">
                      <span className="hop__type">{op.operation_type}</span>
                      <span className="hop__name" title={op.source_path}>
                        {name}
                      </span>
                    </div>
                    <div className="hop__bottom">
                      {op.file_size && op.file_size > 0 && (
                        <span className="hop__size">
                          {formatBytes(op.file_size)}
                        </span>
                      )}
                      {isActive && <span className="hop__pct">{pct}%</span>}
                      <span className="hop__time">
                        {formatTime(op.completed_at || op.created_at)}
                      </span>
                    </div>
                  </div>
                  <div className="hop__meta">
                    {isDone ? (
                      <span className="hop__stat hop__stat--done">✓</span>
                    ) : isFail ? (
                      <span className="hop__stat hop__stat--fail">!</span>
                    ) : isActive ? (
                      <span className="hop__stat hop__stat--active">●</span>
                    ) : (
                      <span className="hop__stat">—</span>
                    )}
                    <button
                      className="hop__del"
                      onClick={(e) => {
                        e.stopPropagation();
                        deleteOp(op.operation_id);
                      }}
                      disabled={deleting}
                      title="Delete"
                    >
                      {deleting ? '⋯' : '×'}
                    </button>
                  </div>
                  <svg
                    className={`hop__chev ${exp ? 'hop__chev--open' : ''}`}
                    width="8"
                    height="8"
                    viewBox="0 0 8 8"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.5"
                  >
                    <polyline points="2.5,1.5 5.5,4 2.5,6.5" />
                  </svg>
                </div>

                {isActive && (
                  <div className="hop__bar">
                    <div
                      className="hop__bar-fill"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                )}

                {exp && (
                  <div className="hop__det">
                    <div className="hop__det-grid">
                      <div className="hop__det-item">
                        <span className="hop__det-l">Status</span>
                        <span
                          className={`hop__det-v hop__det-v--${op.status.toLowerCase()}`}
                        >
                          {op.status}
                        </span>
                      </div>
                      <div className="hop__det-item">
                        <span className="hop__det-l">Source</span>
                        <span
                          className="hop__det-v hop__det-v--path"
                          title={op.source_path}
                        >
                          {op.source_path}
                        </span>
                      </div>
                      {op.destination_path && (
                        <div className="hop__det-item">
                          <span className="hop__det-l">Dest</span>
                          <span
                            className="hop__det-v hop__det-v--path"
                            title={op.destination_path}
                          >
                            {op.destination_path}
                          </span>
                        </div>
                      )}
                      {op.file_size && op.file_size > 0 && (
                        <div className="hop__det-item">
                          <span className="hop__det-l">Size</span>
                          <span className="hop__det-v">
                            {formatBytes(op.bytes_processed)} /{' '}
                            {formatBytes(op.file_size)}
                          </span>
                        </div>
                      )}
                      {op.created_at && op.completed_at && (
                        <div className="hop__det-item">
                          <span className="hop__det-l">Duration</span>
                          <span className="hop__det-v">
                            {formatDuration(op.created_at, op.completed_at)}
                          </span>
                        </div>
                      )}
                    </div>
                    {op.error && <div className="hop__err">{op.error}</div>}
                    {op.files && op.files.length > 0 && (
                      <div className="hop__files">
                        <div className="hop__files-title">
                          Files ({op.files.length})
                        </div>
                        {op.files.slice(0, 10).map((f, i) => (
                          <div key={i} className="hop__file">
                            <span className="hop__file-name">
                              {getFileName(f.local_path || f.remote_path)}
                            </span>
                            <span className="hop__file-size">
                              {formatBytes(f.file_size)}
                            </span>
                            <span
                              className={`hop__file-stat ${f.status === 'Completed' ? 'hop__file-stat--done' : f.status === 'Failed' ? 'hop__file-stat--fail' : ''}`}
                            >
                              {f.status === 'Completed'
                                ? '✓'
                                : f.status === 'Failed'
                                  ? '!'
                                  : `${Math.round((f.bytes_processed / f.file_size) * 100)}%`}
                            </span>
                          </div>
                        ))}
                        {op.files.length > 10 && (
                          <div className="hop__files-more">
                            +{op.files.length - 10} more
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
