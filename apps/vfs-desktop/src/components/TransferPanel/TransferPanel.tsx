/**
 * TransferPanel - Modern timeline-based operations view
 *
 * All operations displayed in a single timeline sorted by timestamp.
 * Compact, lean design with expandable details.
 */

import React, { useEffect, useState, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { POLLING_INTERVALS } from '../../utils/operationEvents';
import './TransferPanel.css';

interface OperationFile {
  local_path: string;
  remote_path: string;
  file_size: number;
  bytes_processed: number;
  status?: 'Pending' | 'InProgress' | 'Completed' | 'Failed' | 'Canceled';
  error?: string;
}

interface OperationState {
  operation_id: string;
  operation_type: string;
  source_id: string;
  source_path: string;
  destination_path?: string;
  file_size?: number;
  bytes_processed: number;
  status: 'Pending' | 'InProgress' | 'Completed' | 'Failed' | 'Canceled';
  error?: string;
  files?: OperationFile[];
  file_count?: number;
  created_at?: string | number;
  completed_at?: string | number;
}

interface TransferPanelProps {
  isVisible: boolean;
  onClose?: () => void;
  onMinimizeChange?: (isMinimized: boolean) => void;
  filterSources?: ('network' | 'cloud')[];
  sources?: Array<{
    id: string;
    name: string;
    providerId: string;
    category: string;
    source_type?: string;
  }>;
}

// Helper functions
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

const formatTime = (ts: string | number | undefined): string => {
  if (!ts) return '';
  const time =
    typeof ts === 'number' ? ts : parseFloat(ts) || Date.parse(ts) || 0;
  const ms = time < 1e12 ? time * 1000 : time;
  const diff = Date.now() - ms;
  if (diff < 60000) return 'now';
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h`;
  return `${Math.floor(diff / 86400000)}d`;
};

const getIcon = (type: string): string => {
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

const getTimestamp = (op: OperationState): number => {
  const ts = op.completed_at || op.created_at;
  if (!ts) return 0;
  const time =
    typeof ts === 'number' ? ts : parseFloat(ts) || Date.parse(ts) || 0;
  return time < 1e12 ? time * 1000 : time;
};

export const TransferPanel: React.FC<TransferPanelProps> = ({
  isVisible,
  onClose,
  // These props are accepted for backward compatibility but not used in the new design
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  onMinimizeChange: _onMinimizeChange,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  filterSources: _filterSources,
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  sources: _sources,
}) => {
  const [operations, setOperations] = useState<OperationState[]>([]);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [activeTab, setActiveTab] = useState<'active' | 'all'>('active');

  const loadOperations = useCallback(async () => {
    try {
      const result = await invoke<OperationState[]>('vfs_list_operations');
      setOperations(result || []);
    } catch (err) {
      console.error('Failed to load operations:', err);
      setOperations([]);
    }
  }, []);

  useEffect(() => {
    if (!isVisible) return;
    loadOperations();
    const interval = setInterval(loadOperations, POLLING_INTERVALS.NORMAL);
    return () => clearInterval(interval);
  }, [isVisible, loadOperations]);

  useEffect(() => {
    const handler = () => loadOperations();
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
    ];
    events.forEach((e) => window.addEventListener(e, handler));
    return () => events.forEach((e) => window.removeEventListener(e, handler));
  }, [loadOperations]);

  // Sort all operations by timestamp (most recent first)
  const sortedOps = useMemo(() => {
    return [...operations].sort((a, b) => getTimestamp(b) - getTimestamp(a));
  }, [operations]);

  // Filter based on tab
  const filteredOps = useMemo(() => {
    if (activeTab === 'active') {
      return sortedOps.filter(
        (op) =>
          op.status === 'InProgress' ||
          op.status === 'Pending' ||
          Date.now() - getTimestamp(op) < 60000, // Include recently completed
      );
    }
    return sortedOps;
  }, [sortedOps, activeTab]);

  const activeCount = useMemo(
    () =>
      operations.filter(
        (op) => op.status === 'InProgress' || op.status === 'Pending',
      ).length,
    [operations],
  );

  const toggle = (id: string) =>
    setExpandedIds((prev) => {
      const n = new Set(prev);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  const dismiss = async (opId: string) => {
    await invoke('vfs_delete_operation', { operationId: opId }).catch(
      console.error,
    );
    loadOperations();
  };

  const dismissAll = async () => {
    const completed = operations.filter(
      (op) => op.status === 'Completed' || op.status === 'Failed',
    );
    await Promise.all(
      completed.map((op) =>
        invoke('vfs_delete_operation', { operationId: op.operation_id }).catch(
          console.error,
        ),
      ),
    );
    loadOperations();
  };

  if (!isVisible) return null;

  return (
    <div className="xfer">
      {/* Header */}
      <div className="xfer__hdr">
        <div className="xfer__hdr-l">
          <h3 className="xfer__title">Operations</h3>
          {activeCount > 0 && (
            <span className="xfer__badge">{activeCount}</span>
          )}
        </div>
        <div className="xfer__hdr-r">
          {operations.length > 0 && (
            <button className="xfer__btn xfer__btn--clear" onClick={dismissAll}>
              Clear All
            </button>
          )}
          {onClose && (
            <button className="xfer__btn xfer__btn--close" onClick={onClose}>
              ×
            </button>
          )}
        </div>
      </div>

      {/* Tabs */}
      <div className="xfer__tabs">
        <button
          className={`xfer__tab ${activeTab === 'active' ? 'xfer__tab--on' : ''}`}
          onClick={() => setActiveTab('active')}
        >
          Active {activeCount > 0 && `(${activeCount})`}
        </button>
        <button
          className={`xfer__tab ${activeTab === 'all' ? 'xfer__tab--on' : ''}`}
          onClick={() => setActiveTab('all')}
        >
          All ({operations.length})
        </button>
      </div>

      {/* Operations List */}
      <div className="xfer__body">
        {filteredOps.length === 0 ? (
          <div className="xfer__empty">
            {activeTab === 'active'
              ? 'No active operations'
              : 'No operations yet'}
          </div>
        ) : (
          filteredOps.map((op) => {
            const name =
              op.file_count && op.file_count > 1
                ? `${op.file_count} items`
                : getFileName(op.source_path);
            const pct =
              op.file_size && op.file_size > 0
                ? Math.min(
                    100,
                    Math.round((op.bytes_processed / op.file_size) * 100),
                  )
                : 0;
            const exp = expandedIds.has(op.operation_id);
            const isActive =
              op.status === 'InProgress' || op.status === 'Pending';
            const isDone = op.status === 'Completed';
            const isFail = op.status === 'Failed';

            return (
              <div
                key={op.operation_id}
                className={`xop ${isActive ? 'xop--active' : ''} ${isFail ? 'xop--fail' : ''} ${exp ? 'xop--exp' : ''}`}
              >
                <div
                  className="xop__row"
                  onClick={() => toggle(op.operation_id)}
                >
                  <span
                    className={`xop__icon ${isActive ? 'xop__icon--active' : isFail ? 'xop__icon--fail' : isDone ? 'xop__icon--done' : ''}`}
                  >
                    {getIcon(op.operation_type)}
                  </span>
                  <div className="xop__main">
                    <span className="xop__name" title={op.source_path}>
                      {name}
                    </span>
                    <span className="xop__type">{op.operation_type}</span>
                  </div>
                  <div className="xop__meta">
                    {isActive ? (
                      <span className="xop__pct">{pct}%</span>
                    ) : isDone ? (
                      <span className="xop__stat xop__stat--done">✓</span>
                    ) : isFail ? (
                      <span className="xop__stat xop__stat--fail">!</span>
                    ) : (
                      <span className="xop__stat">—</span>
                    )}
                    <span className="xop__time">
                      {formatTime(getTimestamp(op))}
                    </span>
                  </div>
                  <button
                    className="xop__x"
                    onClick={(e) => {
                      e.stopPropagation();
                      dismiss(op.operation_id);
                    }}
                    title="Dismiss"
                  >
                    ×
                  </button>
                  <svg
                    className={`xop__chev ${exp ? 'xop__chev--open' : ''}`}
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
                  <div className="xop__bar">
                    <div
                      className="xop__bar-fill"
                      style={{ width: `${pct}%` }}
                    />
                  </div>
                )}

                {exp && (
                  <div className="xop__det">
                    <div className="xop__det-row">
                      <span className="xop__det-l">Status</span>
                      <span
                        className={`xop__det-v xop__det-v--${op.status.toLowerCase()}`}
                      >
                        {op.status}
                      </span>
                    </div>
                    {op.file_size && op.file_size > 0 && (
                      <div className="xop__det-row">
                        <span className="xop__det-l">Size</span>
                        <span className="xop__det-v">
                          {formatBytes(op.bytes_processed)} /{' '}
                          {formatBytes(op.file_size)}
                        </span>
                      </div>
                    )}
                    {op.file_count && op.file_count > 1 && (
                      <div className="xop__det-row">
                        <span className="xop__det-l">Files</span>
                        <span className="xop__det-v">{op.file_count}</span>
                      </div>
                    )}
                    {op.destination_path && (
                      <div className="xop__det-row">
                        <span className="xop__det-l">To</span>
                        <span
                          className="xop__det-v xop__det-v--path"
                          title={op.destination_path}
                        >
                          {getFileName(op.destination_path)}
                        </span>
                      </div>
                    )}
                    {op.error && (
                      <div className="xop__det-row xop__det-row--err">
                        <span className="xop__det-l">Error</span>
                        <span className="xop__det-v xop__det-v--err">
                          {op.error}
                        </span>
                      </div>
                    )}
                    {op.files &&
                      op.files.length > 0 &&
                      op.files.length <= 5 && (
                        <div className="xop__files">
                          {op.files.map((f, i) => (
                            <div key={i} className="xop__file">
                              <span className="xop__file-name">
                                {getFileName(f.local_path || f.remote_path)}
                              </span>
                              <span
                                className={`xop__file-stat ${f.status === 'Completed' ? 'xop__file-stat--done' : f.status === 'Failed' ? 'xop__file-stat--fail' : ''}`}
                              >
                                {f.status === 'Completed'
                                  ? '✓'
                                  : f.status === 'Failed'
                                    ? '!'
                                    : `${Math.round((f.bytes_processed / f.file_size) * 100)}%`}
                              </span>
                            </div>
                          ))}
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
};

export default TransferPanel;
