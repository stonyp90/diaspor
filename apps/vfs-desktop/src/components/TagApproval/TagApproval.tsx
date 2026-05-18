/**
 * TagApproval Component
 *
 * Dialog for reviewing and approving AI-suggested tags for videos
 */
import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, X, Tag } from 'lucide-react';
import './TagApproval.css';

interface SuggestedTag {
  id: string;
  sourceId: string;
  filePath: string;
  fileName: string;
  suggestedTags: Array<{ name: string; confidence: number }>;
  createdAt: string;
}

interface TagApprovalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const TagApproval: React.FC<TagApprovalProps> = ({
  isOpen,
  onClose,
}) => {
  const [suggestions, setSuggestions] = useState<SuggestedTag[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedTags, setSelectedTags] = useState<Record<string, Set<string>>>(
    {},
  );

  // Load pending suggestions
  useEffect(() => {
    if (isOpen) {
      loadSuggestions();
    }
  }, [isOpen]);

  const loadSuggestions = async () => {
    setLoading(true);
    try {
      const pending = await invoke<SuggestedTag[]>(
        'get_pending_tag_suggestions',
      );
      setSuggestions(pending || []);
      // Initialize selected tags (all selected by default)
      const initial: Record<string, Set<string>> = {};
      pending.forEach((s) => {
        initial[s.id] = new Set(s.suggestedTags.map((t) => t.name));
      });
      setSelectedTags(initial);
    } catch (err) {
      console.error('Failed to load tag suggestions:', err);
      setSuggestions([]);
    }
    setLoading(false);
  };

  // Toggle tag selection
  const toggleTag = (suggestionId: string, tagName: string) => {
    setSelectedTags((prev) => {
      const newSelected = { ...prev };
      if (!newSelected[suggestionId]) {
        newSelected[suggestionId] = new Set();
      }
      const tags = new Set(newSelected[suggestionId]);
      if (tags.has(tagName)) {
        tags.delete(tagName);
      } else {
        tags.add(tagName);
      }
      newSelected[suggestionId] = tags;
      return newSelected;
    });
  };

  // Approve selected tags for a suggestion
  const approveSuggestion = async (suggestion: SuggestedTag) => {
    const tagsToApprove = Array.from(selectedTags[suggestion.id] || new Set());
    if (tagsToApprove.length === 0) {
      // Reject all tags
      await rejectSuggestion(suggestion.id);
      return;
    }

    try {
      await invoke('approve_tag_suggestions', {
        suggestionId: suggestion.id,
        tags: tagsToApprove,
      });
      // Remove from list
      setSuggestions((prev) => prev.filter((s) => s.id !== suggestion.id));
      delete selectedTags[suggestion.id];
    } catch (err) {
      console.error('Failed to approve tags:', err);
    }
  };

  // Reject all tags for a suggestion
  const rejectSuggestion = async (suggestionId: string) => {
    try {
      await invoke('reject_tag_suggestions', { suggestionId });
      setSuggestions((prev) => prev.filter((s) => s.id !== suggestionId));
      delete selectedTags[suggestionId];
    } catch (err) {
      console.error('Failed to reject tags:', err);
    }
  };

  // Approve all suggestions
  const approveAll = async () => {
    for (const suggestion of suggestions) {
      await approveSuggestion(suggestion);
    }
  };

  // Reject all suggestions
  const rejectAll = async () => {
    for (const suggestion of suggestions) {
      await rejectSuggestion(suggestion.id);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="tag-approval-overlay" onClick={onClose}>
      <div className="tag-approval-dialog" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="tag-approval-header">
          <div className="tag-approval-title">
            <Tag size={20} />
            <h2>Review Tag Suggestions</h2>
            <span className="tag-approval-count">
              {suggestions.length} pending
            </span>
          </div>
          <button className="tag-approval-close" onClick={onClose}>
            <X size={20} />
          </button>
        </div>

        {/* Content */}
        <div className="tag-approval-content">
          {loading ? (
            <div className="tag-approval-loading">
              <div className="spinner" />
              <span>Loading suggestions...</span>
            </div>
          ) : suggestions.length === 0 ? (
            <div className="tag-approval-empty">
              <Tag size={48} />
              <p>No pending tag suggestions</p>
              <span>All caught up! New suggestions will appear here.</span>
            </div>
          ) : (
            <>
              {/* Bulk Actions */}
              <div className="tag-approval-bulk">
                <button
                  className="tag-approval-btn tag-approval-btn-primary"
                  onClick={approveAll}
                >
                  <Check size={16} />
                  Approve All
                </button>
                <button
                  className="tag-approval-btn tag-approval-btn-secondary"
                  onClick={rejectAll}
                >
                  <X size={16} />
                  Reject All
                </button>
              </div>

              {/* Suggestions List */}
              <div className="tag-approval-list">
                {suggestions.map((suggestion) => (
                  <div key={suggestion.id} className="tag-approval-item">
                    <div className="tag-approval-item-header">
                      <div className="tag-approval-file-info">
                        <span className="tag-approval-file-name">
                          {suggestion.fileName}
                        </span>
                        <span className="tag-approval-file-path">
                          {suggestion.filePath}
                        </span>
                      </div>
                      <div className="tag-approval-item-actions">
                        <button
                          className="tag-approval-btn tag-approval-btn-primary small"
                          onClick={() => approveSuggestion(suggestion)}
                        >
                          <Check size={14} />
                          Approve Selected
                        </button>
                        <button
                          className="tag-approval-btn tag-approval-btn-secondary small"
                          onClick={() => rejectSuggestion(suggestion.id)}
                        >
                          <X size={14} />
                          Reject All
                        </button>
                      </div>
                    </div>

                    {/* Tags */}
                    <div className="tag-approval-tags">
                      {suggestion.suggestedTags.map((tag) => {
                        const isSelected = selectedTags[suggestion.id]?.has(
                          tag.name,
                        );
                        return (
                          <button
                            key={tag.name}
                            className={`tag-approval-tag ${isSelected ? 'selected' : ''}`}
                            onClick={() => toggleTag(suggestion.id, tag.name)}
                          >
                            <span className="tag-approval-tag-name">
                              {tag.name}
                            </span>
                            <span className="tag-approval-tag-confidence">
                              {Math.round(tag.confidence * 100)}%
                            </span>
                            {isSelected && (
                              <Check size={12} className="tag-approval-check" />
                            )}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
};

export default TagApproval;
