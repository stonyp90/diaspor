import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './StorageTierDialog.css';

export interface TierTarget {
  tier: string;
  tier_name: string;
  description: string;
  target_source_id: string | null;
  requires_target_source: boolean;
  provider_name?: string | null;
  storage_class?: string | null;
}

interface StorageTierDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (tier: string, targetSourceId?: string) => void;
  sourceId: string;
  filePaths: string[];
  onAddProvider?: () => void;
}

export const StorageTierDialog: React.FC<StorageTierDialogProps> = ({
  isOpen,
  onClose,
  onConfirm,
  sourceId,
  filePaths,
  onAddProvider,
}) => {
  const [tierTargets, setTierTargets] = useState<TierTarget[]>([]);
  const [selectedTarget, setSelectedTarget] = useState<TierTarget | null>(null);
  const [loadingTargets, setLoadingTargets] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen && sourceId) {
      setLoadingTargets(true);
      setError(null);
      setTierTargets([]);
      setSelectedTarget(null);
      loadTierTargets();
    }
  }, [isOpen, sourceId]);

  const loadTierTargets = async () => {
    if (!sourceId) {
      setError('No storage source selected.');
      setLoadingTargets(false);
      return;
    }

    try {
      setLoadingTargets(true);
      console.log('Loading tier targets for source:', sourceId);
      const targets = await invoke<TierTarget[]>('vfs_get_tier_targets', {
        sourceId,
      });
      console.log('Loaded tier targets:', targets);
      setTierTargets(targets);
      if (targets.length > 0) {
        setSelectedTarget(targets[0]);
      } else {
        setError('No tier options available for this storage source.');
      }
    } catch (err) {
      console.error('Failed to load tier targets:', err);
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(`Failed to load tier options: ${errorMessage}`);
    } finally {
      setLoadingTargets(false);
    }
  };

  const handleConfirm = () => {
    if (!selectedTarget) return;

    onConfirm(
      selectedTarget.tier,
      selectedTarget.target_source_id || undefined,
    );
  };

  // Close on Escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        e.preventDefault();
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <div className="storage-tier-dialog-overlay" onClick={onClose}>
      <div className="storage-tier-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="storage-tier-dialog-header">
          <h2>Move to Storage Tier</h2>
          <button
            className="storage-tier-dialog-close"
            onClick={onClose}
            aria-label="Close"
          >
            ×
          </button>
        </div>

        <div className="storage-tier-dialog-content">
          <div className="storage-tier-dialog-info">
            <p>
              Moving <strong>{filePaths.length}</strong> file
              {filePaths.length !== 1 ? 's' : ''} to a different storage tier.
            </p>
            {filePaths.length <= 3 && (
              <ul className="storage-tier-file-list">
                {filePaths.map((path, idx) => (
                  <li key={idx}>{path.split('/').pop() || path}</li>
                ))}
              </ul>
            )}
          </div>

          <div className="storage-tier-options">
            <label className="storage-tier-label">Select Target Tier:</label>
            {loadingTargets ? (
              <div className="storage-tier-loading">
                <p>Loading tier options...</p>
              </div>
            ) : error || tierTargets.length === 0 ? (
              <div className="storage-tier-empty">
                <p>
                  {error ||
                    'No tier options available for this storage source.'}
                </p>
                {onAddProvider && (
                  <button
                    className="storage-tier-btn-add-provider"
                    onClick={() => {
                      onClose();
                      onAddProvider();
                    }}
                  >
                    + Add Storage Provider
                  </button>
                )}
              </div>
            ) : (
              <>
                <div className="storage-tier-grid">
                  {tierTargets.map((target, index) => {
                    const isSelected =
                      selectedTarget?.tier === target.tier &&
                      selectedTarget?.target_source_id ===
                        target.target_source_id;
                    return (
                      <button
                        key={`${target.tier}-${target.target_source_id || 'none'}-${index}`}
                        className={`storage-tier-option ${isSelected ? 'selected' : ''}`}
                        onClick={() => setSelectedTarget(target)}
                      >
                        <div className="storage-tier-option-header">
                          <div className="storage-tier-option-title-group">
                            <span className="storage-tier-option-name">
                              {target.tier_name}
                            </span>
                            {target.storage_class && (
                              <span
                                className="storage-tier-storage-class"
                                title={`${target.provider_name || 'Storage'} Storage Class: ${target.storage_class}`}
                              >
                                {target.provider_name && (
                                  <span className="storage-tier-provider-tag">
                                    {target.provider_name}
                                  </span>
                                )}
                                <span className="storage-tier-class-tag">
                                  {target.storage_class}
                                </span>
                              </span>
                            )}
                          </div>
                          <span
                            className={`storage-tier-badge tier-${target.tier}`}
                            title={`${target.tier_name} Tier`}
                          >
                            {target.tier === 'local'
                              ? 'L'
                              : target.tier === 'nearline'
                                ? 'N'
                                : target.tier === 'cold'
                                  ? 'C'
                                  : target.tier === 'hot'
                                    ? 'H'
                                    : target.tier.charAt(0).toUpperCase()}
                          </span>
                        </div>
                        <p className="storage-tier-option-description">
                          {target.description}
                        </p>
                      </button>
                    );
                  })}
                </div>
                {onAddProvider && (
                  <button
                    className="storage-tier-btn-add-provider"
                    onClick={() => {
                      onClose();
                      onAddProvider();
                    }}
                  >
                    + Add Provider
                  </button>
                )}
              </>
            )}
          </div>
        </div>

        <div className="storage-tier-dialog-footer">
          <button
            className="storage-tier-btn storage-tier-btn-cancel"
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            className="storage-tier-btn storage-tier-btn-confirm"
            onClick={handleConfirm}
            disabled={
              !selectedTarget || loadingTargets || tierTargets.length === 0
            }
          >
            {loadingTargets ? 'Moving...' : 'Move to Tier'}
          </button>
        </div>
      </div>
    </div>
  );
};
