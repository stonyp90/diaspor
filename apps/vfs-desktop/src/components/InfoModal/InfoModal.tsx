/**
 * AssetDetailsPanel (InfoModal) - DAM/MAM-style asset metadata panel
 *
 * Industry-standard asset management panel displaying:
 * - General info (name, size, location, dates)
 * - Storage info (tier, cached, source)
 * - Media technical metadata (for video/audio files)
 * - Organization (tags, color labels, comments)
 * - Project/Client/Department assignment
 * - Approval workflow status
 * - Usage rights and licensing
 * - Custom metadata fields
 *
 * Terminology follows DAM/MAM industry standards:
 * - "Asset Details" instead of "Get Info"
 * - "Tags" for searchable keywords
 * - "Metadata" for technical information
 */
import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FileMetadata } from '../../types/storage';
import {
  IconFolderCyber,
  getFileIcon as getFileIconComponent,
  IconStar,
  IconTag,
} from '../CyberpunkIcons';
import { formatSize } from '../../pages/FinderPage/utils';
import { TranscriptionModal } from '../TranscriptionModal';
import './InfoModal.css';

// Microphone icon component using theme colors
const MicrophoneIcon: React.FC<{ size?: number }> = ({ size = 16 }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="var(--vfs-primary)"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
    <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
    <line x1="12" y1="19" x2="12" y2="23" />
    <line x1="8" y1="23" x2="16" y2="23" />
  </svg>
);

interface FileTag {
  name: string;
  color?: string;
}

interface InfoModalProps {
  file: FileMetadata;
  sourceId?: string;
  sourceCategory?: string; // Storage category (local, cloud, network, etc.)
  onClose: () => void;
  onToggleFavorite?: (file: FileMetadata) => void;
  onAddTag?: (
    file: FileMetadata,
    tag: string | { name: string; color?: string },
  ) => void;
  onRemoveTag?: (file: FileMetadata, tag: string) => void;
  onSetColorLabel?: (file: FileMetadata, color: string | null) => void;
  onUpdateComments?: (file: FileMetadata, comments: string) => void;
  isFavorite?: boolean;
}

// Color labels use CSS variables for theme consistency
// Colors are derived from theme with slight variations
const COLOR_LABELS = [
  { name: 'None', value: null, color: 'transparent' },
  { name: 'Primary', value: 'primary', color: 'var(--primary)' },
  { name: 'Secondary', value: 'secondary', color: 'var(--secondary)' },
  { name: 'Accent', value: 'accent', color: 'var(--accent)' },
  { name: 'Success', value: 'success', color: 'var(--success)' },
  { name: 'Warning', value: 'warning', color: 'var(--warning)' },
  { name: 'Error', value: 'error', color: 'var(--error)' },
  { name: 'Muted', value: 'muted', color: 'var(--text-muted)' },
];

function formatDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }
  return `${minutes}:${secs.toString().padStart(2, '0')}`;
}

function formatBitrate(kbps: number | undefined): string {
  if (!kbps) return '—';
  if (kbps >= 1000) {
    return `${(kbps / 1000).toFixed(1)} Mbps`;
  }
  return `${kbps} kbps`;
}

function getAudioChannelLabel(channels: number | undefined): string {
  if (!channels) return '—';
  switch (channels) {
    case 1:
      return 'Mono';
    case 2:
      return 'Stereo';
    case 6:
      return '5.1 Surround';
    case 8:
      return '7.1 Surround';
    default:
      return `${channels} channels`;
  }
}

function getResolutionLabel(
  width: number | undefined,
  height: number | undefined,
): string {
  if (!width || !height) return '—';

  // Determine resolution name
  if (height >= 2160) return `4K UHD (${width}×${height})`;
  if (height >= 1440) return `2K QHD (${width}×${height})`;
  if (height >= 1080) return `Full HD (${width}×${height})`;
  if (height >= 720) return `HD (${width}×${height})`;
  if (height >= 480) return `SD (${width}×${height})`;
  return `${width}×${height}`;
}

function getHdrLabel(hdrFormat: string | undefined): string {
  if (!hdrFormat) return 'SDR';
  switch (hdrFormat.toLowerCase()) {
    case 'hdr10':
      return 'HDR10';
    case 'hdr10+':
      return 'HDR10+';
    case 'dolby_vision':
      return 'Dolby Vision';
    case 'hlg':
      return 'HLG';
    default:
      return hdrFormat.toUpperCase();
  }
}

function getTierLabel(tier: string): { label: string; class: string } {
  switch (tier) {
    case 'hot':
      return { label: 'Hot (Instant)', class: 'tier-hot' };
    case 'warm':
      return { label: 'Warm (Fast)', class: 'tier-warm' };
    case 'cold':
      return { label: 'Cold (Minutes)', class: 'tier-cold' };
    case 'nearline':
      return { label: 'Nearline (Minutes-Hours)', class: 'tier-nearline' };
    case 'archive':
      return { label: 'Archive (Hours)', class: 'tier-archive' };
    default:
      return { label: tier, class: '' };
  }
}

// Unused helper functions - kept for potential future use
// function getAssetCategoryLabel(category: string | undefined): string {
//   switch (category) {
//     case 'raw':
//       return 'Raw Footage';
//     case 'edit':
//       return 'Edit/Work in Progress';
//     case 'final':
//       return 'Final/Approved';
//     case 'archive':
//       return 'Archive';
//     case 'proxy':
//       return 'Proxy/Preview';
//     case 'other':
//       return 'Other';
//     default:
//       return 'Not Categorized';
//   }
// }

// function getApprovalStatusLabel(status: string | undefined): string {
//   switch (status) {
//     case 'pending':
//       return 'Pending Review';
//     case 'approved':
//       return 'Approved';
//     case 'rejected':
//       return 'Rejected';
//     case 'review':
//       return 'In Review';
//     default:
//       return 'Pending Review';
//   }
// }

/**
 * Get suggested tags based on file type (DAM/MAM feature)
 * Provides intelligent tag suggestions for better asset organization
 */
function getSuggestedTags(file: FileMetadata): string[] {
  const extension = file.name.split('.').pop()?.toLowerCase() || '';
  const mimeType = file.mimeType?.toLowerCase() || '';
  const suggestions: string[] = [];

  // Video suggestions
  if (
    mimeType.startsWith('video/') ||
    ['mp4', 'mov', 'avi', 'mkv', 'prores', 'mxf'].includes(extension)
  ) {
    suggestions.push(
      'video',
      'footage',
      'b-roll',
      'interview',
      'raw',
      'edit',
      'final',
      'proxy',
    );
  }

  // Audio suggestions
  if (
    mimeType.startsWith('audio/') ||
    ['mp3', 'wav', 'aiff', 'flac', 'm4a'].includes(extension)
  ) {
    suggestions.push(
      'audio',
      'music',
      'voiceover',
      'sfx',
      'soundtrack',
      'mix',
      'stem',
    );
  }

  // Image suggestions
  if (
    mimeType.startsWith('image/') ||
    ['jpg', 'jpeg', 'png', 'tiff', 'psd', 'raw', 'cr2', 'arw'].includes(
      extension,
    )
  ) {
    suggestions.push(
      'image',
      'photo',
      'still',
      'thumbnail',
      'screenshot',
      'artwork',
      'logo',
    );
  }

  // Document suggestions
  if (['pdf', 'doc', 'docx', 'txt', 'rtf'].includes(extension)) {
    suggestions.push(
      'document',
      'script',
      'storyboard',
      'contract',
      'brief',
      'notes',
    );
  }

  // Project file suggestions
  if (['prproj', 'aep', 'psd', 'ai', 'fcp', 'drp'].includes(extension)) {
    suggestions.push('project', 'source', 'working', 'master');
  }

  // Common workflow tags
  suggestions.push('review', 'approved', 'archive', 'hero', 'selects');

  return [...new Set(suggestions)]; // Remove duplicates
}

export const InfoModal: React.FC<InfoModalProps> = ({
  file,
  sourceId,
  sourceCategory,
  onClose,
  onToggleFavorite,
  onAddTag,
  onRemoveTag,
  onSetColorLabel,
  onUpdateComments,
  isFavorite = false,
}) => {
  const [newTag, setNewTag] = useState('');
  const [newTagColor, setNewTagColor] = useState('#8E8E93');
  const [comments, setComments] = useState(file.comments || '');
  const [isEditingComments, setIsEditingComments] = useState(false);
  const [availableTags, setAvailableTags] = useState<FileTag[]>([]);
  const [tagColors, setTagColors] = useState<Record<string, string>>({});
  const [isAutoTagging, setIsAutoTagging] = useState(false);
  const [showTranscriptionModal, setShowTranscriptionModal] = useState(false);
  const [isTaggingModelRunning, setIsTaggingModelRunning] = useState(false);
  const [isTranscriptionModelRunning, setIsTranscriptionModelRunning] = useState(false);
  const [pendingTags, setPendingTags] = useState<Array<{ name: string; confidence: number; color?: string }>>([]);
  const [selectedTagsForApproval, setSelectedTagsForApproval] = useState<Set<string>>(new Set());
  const [showTagApproval, setShowTagApproval] = useState(false);
  const [tagColorsForApproval, setTagColorsForApproval] = useState<Record<string, string>>({});
  const [maxTagsToGenerate, setMaxTagsToGenerate] = useState(() => {
    const saved = localStorage.getItem('ai_max_tags_per_asset');
    return saved ? parseInt(saved, 10) : 5;
  });
  const [showTagSettings, setShowTagSettings] = useState(false);
  const modalRef = React.useRef<HTMLDivElement>(null);

  // Save max tags setting
  const handleMaxTagsChange = (value: number) => {
    const clampedValue = Math.min(10, Math.max(1, value));
    setMaxTagsToGenerate(clampedValue);
    localStorage.setItem('ai_max_tags_per_asset', clampedValue.toString());
  };

  // Auto-focus modal when it opens for keyboard navigation
  useEffect(() => {
    if (modalRef.current) {
      modalRef.current.focus();
    }
  }, []);

  // Close on Escape key - ensure it works even when modal is focused
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' || e.key === 'Esc') {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }
    };

    // Add listener to window and document to catch all escape presses
    window.addEventListener('keydown', handleKeyDown, true);
    document.addEventListener('keydown', handleKeyDown, true);

    return () => {
      window.removeEventListener('keydown', handleKeyDown, true);
      document.removeEventListener('keydown', handleKeyDown, true);
    };
  }, [onClose]);

  // Load available tags with colors
  useEffect(() => {
    const loadTags = async () => {
      try {
        // Try to get tags from Tauri backend if available
        if (
          sourceId &&
          typeof window !== 'undefined' &&
          '__TAURI_INTERNALS__' in window
        ) {
          const tags = await invoke<FileTag[]>('vfs_list_all_tags', {
            sourceId,
          });
          setAvailableTags(tags);

          // Create a map of tag names to colors
          const colorMap: Record<string, string> = {};
          tags.forEach((tag) => {
            if (tag.color) {
              colorMap[tag.name] = tag.color;
            }
          });
          setTagColors(colorMap);
        }
      } catch (error) {
        // Fallback: try to get from localStorage
      }
    };

    loadTags();
  }, [sourceId]);

  // Check if models are running
  useEffect(() => {
    const checkModels = async () => {
      try {
        const response = await invoke<{
          models?: Array<{ name: string; model?: string }>;
        }>('ollama_ps');
        const runningModels = response.models || [];
        const runningModelNames = runningModels.map((m) => {
          const modelName = (m.model || m.name || '').toLowerCase();
          return modelName;
        });
        const taggingRunning = runningModelNames.some(
          (n) =>
            n === 'llava' ||
            n === 'llava:latest' ||
            n.includes('llava:') ||
            n.includes('llava'),
        );
        // Transcription uses FFmpeg Whisper (built-in), not Ollama
        // Check FFmpeg availability and if transcription was explicitly started
        try {
          const ffmpegInstalled = await invoke<boolean>('check_ffmpeg_installed').catch(() => false);
          const transcriptionAvailable = await invoke<boolean>(
            'vfs_is_transcription_available',
          ).catch(() => ffmpegInstalled);
          // Check if transcription was explicitly started
          const wasStarted = localStorage.getItem('transcription_model_started') === 'true';
          setIsTranscriptionModelRunning(transcriptionAvailable && wasStarted);
        } catch {
          setIsTranscriptionModelRunning(false);
        }
        setIsTaggingModelRunning(taggingRunning);
      } catch (error) {
        // Ollama not running or not available
        setIsTaggingModelRunning(false);
        setIsTranscriptionModelRunning(false);
      }
    };

    checkModels();
    // Check periodically
    const interval = setInterval(checkModels, 5000);
    return () => clearInterval(interval);
  }, []);

  const isFolder = file.mimeType === 'folder' || file.path.endsWith('/');
  
  // Extension-based detection for media files
  const fileExtension = file.name.split('.').pop()?.toLowerCase() || '';
  const videoExtensionsList = ['mp4', 'mov', 'avi', 'mkv', 'webm', 'wmv', 'flv', 'm4v', 'mpg', 'mpeg'];
  const audioExtensionsList = ['mp3', 'wav', 'aiff', 'flac', 'm4a', 'aac', 'ogg', 'wma'];
  
  const isVideo = file.mimeType?.startsWith('video/') || videoExtensionsList.includes(fileExtension);
  const isAudio = file.mimeType?.startsWith('audio/') || audioExtensionsList.includes(fileExtension);
  const isMedia = isVideo || isAudio;

  const tierInfo = getTierLabel(file.tierStatus);

  const handleAddTag = () => {
    if (newTag.trim() && onAddTag) {
      onAddTag(file, { name: newTag.trim(), color: newTagColor });
      setNewTag('');
      setNewTagColor('#8E8E93'); // Reset to default color
    }
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleAddTag();
    }
  };

  const handleSaveComments = () => {
    if (onUpdateComments) {
      onUpdateComments(file, comments);
    }
    setIsEditingComments(false);
  };

  const handleAutoTag = async () => {
    if (!sourceId || isAutoTagging) return;
    
    setIsAutoTagging(true);
    try {
      const result = await invoke<{
        success: boolean;
        tags: string[];
        message: string;
      }>('vfs_auto_tag_file', {
        sourceId,
        filePath: file.path,
        maxTags: maxTagsToGenerate,
      });

      if (result.success && result.tags.length > 0) {
        // Add all suggested tags (limited by maxTags)
        const limitedTags = result.tags.slice(0, maxTagsToGenerate);
        for (const tag of limitedTags) {
          if (onAddTag) {
            onAddTag(file, tag);
          }
        }
      }
    } catch (error) {
      console.error('Auto-tagging failed:', error);
      // Could show a toast notification here
    } finally {
      setIsAutoTagging(false);
    }
  };

  const handleRegenerateTags = async () => {
    if (!sourceId || isAutoTagging) return;
    
    setIsAutoTagging(true);
    try {
      const result = await invoke<{
        success: boolean;
        tags: string[];
        message: string;
      }>('vfs_auto_tag_file', {
        sourceId,
        filePath: file.path,
        maxTags: maxTagsToGenerate,
      });

      if (result.success && result.tags.length > 0) {
        // Show tags for approval instead of auto-adding
        const limitedTags = result.tags.slice(0, maxTagsToGenerate);
        const tagsWithConfidence = limitedTags.map((tag, index) => ({
          name: tag,
          confidence: Math.max(0.5, 1 - (index * 0.1)), // Higher confidence for earlier tags
        }));
        setPendingTags(tagsWithConfidence);
        // Pre-select up to 5 tags for AI-based search
        const searchTags = limitedTags.slice(0, 5);
        setSelectedTagsForApproval(new Set(searchTags));
        setShowTagApproval(true);
      }
    } catch (error) {
      console.error('Tag regeneration failed:', error);
    } finally {
      setIsAutoTagging(false);
    }
  };

  // Helper to check if file is taggable (image, video, or audio)
  const isTaggableFile = () => {
    const extension = file.name.split('.').pop()?.toLowerCase() || '';
    const videoExtensions = ['mp4', 'mov', 'avi', 'mkv', 'webm', 'wmv', 'flv', 'm4v', 'mpg', 'mpeg'];
    const imageExtensions = ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'tiff', 'tif', 'heic', 'heif'];
    const audioExtensions = ['mp3', 'wav', 'aiff', 'flac', 'm4a', 'aac', 'ogg', 'wma'];
    
    return (
      file.mimeType?.startsWith('video/') ||
      file.mimeType?.startsWith('image/') ||
      file.mimeType?.startsWith('audio/') ||
      videoExtensions.includes(extension) ||
      imageExtensions.includes(extension) ||
      audioExtensions.includes(extension)
    );
  };

  const handleGenerateTags = async () => {
    if (!sourceId || isAutoTagging) return;

    // Check if it's an image or video file
    if (!isTaggableFile()) {
      console.warn('AI tagging only works on images and videos');
      return;
    }
    
    setIsAutoTagging(true);
    try {
      const result = await invoke<{
        success: boolean;
        tags: string[];
        message: string;
      }>('vfs_auto_tag_file', {
        sourceId,
        filePath: file.path,
        maxTags: maxTagsToGenerate,
      });

      if (result.success && result.tags.length > 0) {
        // Limit to max tags and show for approval
        const limitedTags = result.tags.slice(0, maxTagsToGenerate);
        const tagsWithConfidence = limitedTags.map((tag, index) => ({
          name: tag,
          confidence: Math.max(0.5, 1 - (index * 0.1)), // Higher confidence for earlier tags
        }));
        setPendingTags(tagsWithConfidence);
        // Pre-select up to 5 tags for AI-based search
        const searchTags = limitedTags.slice(0, 5);
        setSelectedTagsForApproval(new Set(searchTags));
        setShowTagApproval(true);
      }
    } catch (error) {
      console.error('Tag generation failed:', error);
    } finally {
      setIsAutoTagging(false);
    }
  };

  const handleApproveTags = (tagsToApprove: string[]) => {
    tagsToApprove.forEach((tag) => {
      if (onAddTag) {
        const tagColor = tagColors[tag];
        onAddTag(file, tagColor ? { name: tag, color: tagColor } : tag);
      }
    });
    setPendingTags([]);
    setTagColorsForApproval({});
    setShowTagApproval(false);
  };

  const handleRejectTags = () => {
    setPendingTags([]);
    setSelectedTagsForApproval(new Set());
    setTagColorsForApproval({});
    setShowTagApproval(false);
  };

  const toggleTagSelection = (tagName: string) => {
    setSelectedTagsForApproval((prev) => {
      const newSet = new Set(prev);
      if (newSet.has(tagName)) {
        newSet.delete(tagName);
      } else {
        newSet.add(tagName);
      }
      return newSet;
    });
  };

  const handleTagColorChange = (tagName: string, color: string) => {
    setTagColorsForApproval((prev) => ({
      ...prev,
      [tagName]: color,
    }));
  };

  const handleApprovalTagColorChange = (tagName: string, color: string) => {
    setTagColorsForApproval((prev) => ({
      ...prev,
      [tagName]: color,
    }));
  };

  // Get file icon
  const FileIcon = isFolder
    ? () => <IconFolderCyber size={64} glow />
    : () => {
        const IconComponent = getFileIconComponent(file.name, file.mimeType);
        return <IconComponent size={64} glow />;
      };

  return (
    <>
      {showTranscriptionModal && (
        <TranscriptionModal
          file={file}
          sourceId={sourceId}
          onClose={() => setShowTranscriptionModal(false)}
          onStart={(operationId) => {
            console.log('Transcription started:', operationId);
            setShowTranscriptionModal(false);
          }}
        />
      )}
      {showTagApproval && pendingTags.length > 0 && (
        <div className="tag-approval-overlay" onClick={() => setShowTagApproval(false)}>
          <div className="tag-approval-modal" onClick={(e) => e.stopPropagation()}>
            <div className="tag-approval-header">
              <div className="tag-approval-title-section">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--vfs-accent)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M12 3l1.912 5.813a2 2 0 0 0 1.275 1.275L21 12l-5.813 1.912a2 2 0 0 0-1.275 1.275L12 21l-1.912-5.813a2 2 0 0 0-1.275-1.275L3 12l5.813-1.912a2 2 0 0 0 1.275-1.275L12 3z" />
                </svg>
                <h3>Review AI Generated Tags</h3>
              </div>
              <button
                className="tag-approval-close"
                onClick={() => setShowTagApproval(false)}
                aria-label="Close"
                title="Close (Esc)"
              >
                ×
              </button>
            </div>
            <div className="tag-approval-content">
              <div className="tag-approval-description">
                <p>Select tags to add and customize their colors. Tags with higher confidence are more accurate.</p>
                <div className="tag-approval-stats">
                  <span className="tag-approval-stat">
                    <strong>{pendingTags.length}</strong> tags generated
                  </span>
                  <span className="tag-approval-stat">
                    <strong>{selectedTagsForApproval.size}</strong> selected
                  </span>
                </div>
              </div>
              <div className="tag-approval-list">
                {pendingTags.map((tag, index) => {
                  const isSelected = selectedTagsForApproval.has(tag.name);
                  const tagColor = tagColors[tag.name] || tag.color || '#8E8E93';
                  return (
                    <div key={index} className={`tag-approval-item ${isSelected ? 'selected' : ''}`}>
                      <label className="tag-approval-checkbox-wrapper">
                        <input
                          type="checkbox"
                          checked={isSelected}
                          onChange={() => toggleTagSelection(tag.name)}
                          className="tag-approval-checkbox"
                        />
                        <span className="tag-approval-checkbox-custom">
                          {isSelected && (
                            <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                              <path d="M10 3L4.5 8.5L2 6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                            </svg>
                          )}
                        </span>
                      </label>
                      <div className="tag-approval-tag-preview" style={{ 
                        borderColor: tagColor,
                        backgroundColor: `${tagColor}15`
                      }}>
                        <span className="tag-approval-tag-dot" style={{ backgroundColor: tagColor }} />
                        <span className="tag-approval-name">{tag.name}</span>
                      </div>
                      {tag.confidence && (
                        <div className="tag-approval-confidence-bar">
                          <div 
                            className="tag-approval-confidence-fill" 
                            style={{ 
                              width: `${tag.confidence * 100}%`,
                              backgroundColor: tagColor
                            }}
                          />
                          <span className="tag-approval-confidence-text">
                            {Math.round(tag.confidence * 100)}%
                          </span>
                        </div>
                      )}
                      <input
                        type="color"
                        value={tagColor}
                        onChange={(e) => handleApprovalTagColorChange(tag.name, e.target.value)}
                        className="tag-approval-color-picker"
                        title="Choose tag color"
                        aria-label={`Color for ${tag.name}`}
                      />
                    </div>
                  );
                })}
              </div>
            </div>
            <div className="tag-approval-actions">
              <button
                className="tag-approval-reject"
                onClick={handleRejectTags}
                title="Discard all tags"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
                <span>Reject All</span>
              </button>
              <button
                className="tag-approval-approve"
                onClick={() => {
                  const tagsToApprove = Array.from(selectedTagsForApproval);
                  handleApproveTags(tagsToApprove);
                }}
                disabled={selectedTagsForApproval.size === 0}
                title={selectedTagsForApproval.size === 0 ? 'Select tags to approve' : `Add ${selectedTagsForApproval.size} tags`}
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="20 6 9 17 4 12"></polyline>
                </svg>
                <span>Add {selectedTagsForApproval.size > 0 ? `${selectedTagsForApproval.size} ` : ''}Tag{selectedTagsForApproval.size !== 1 ? 's' : ''}</span>
              </button>
            </div>
          </div>
        </div>
      )}
      <div
        className="info-modal-overlay"
        onClick={onClose}
        role="dialog"
        aria-modal="true"
        aria-labelledby="info-modal-title"
      >
        <div
          ref={modalRef}
          className="info-modal"
          onClick={(e) => e.stopPropagation()}
          role="document"
          tabIndex={-1}
        >
          {/* Header */}
          <div className="info-header">
            <div className="info-header-icon">
              <FileIcon />
            </div>
            <div className="info-header-title">
              <h2 id="info-modal-title">{file.name}</h2>
              <span className="info-kind">{file.mimeType || 'Unknown'}</span>
            </div>
            <button
              className="info-close"
              onClick={onClose}
              aria-label="Close Asset Details"
              title="Close (Esc)"
            >
              ×
            </button>
          </div>

        {/* Content */}
        <div className="info-content">
          {/* General Info Section */}
          <section className="info-section">
            <h3 className="info-section-title">General</h3>
            <div className="info-grid">
              <div className="info-field">
                <span className="info-label">Kind</span>
                <span className="info-value">
                  {file.mimeType || (isFolder ? 'Folder' : 'File')}
                </span>
              </div>
              <div className="info-field">
                <span className="info-label">Size</span>
                <span className="info-value">
                  {file.size_human || formatSize(file.size)}
                </span>
              </div>
              <div className="info-field full-width">
                <span className="info-label">Location</span>
                <span className="info-value path">{file.path}</span>
              </div>
              <div className="info-field">
                <span className="info-label">Modified</span>
                <span className="info-value">
                  {new Date(file.lastModified).toLocaleString()}
                </span>
              </div>
              {file.createdAt && (
                <div className="info-field">
                  <span className="info-label">Created</span>
                  <span className="info-value">
                    {new Date(file.createdAt).toLocaleString()}
                  </span>
                </div>
              )}
            </div>
          </section>

          {/* Storage Info Section */}
          <section className="info-section">
            <h3 className="info-section-title">Storage</h3>
            <div className="info-grid">
              {/* For local storage, show "Local" instead of tier */}
              {sourceCategory === 'local' ? (
                <div className="info-field">
                  <span className="info-label">Storage</span>
                  <span className="info-value tier-badge tier-local">
                    Local
                  </span>
                </div>
              ) : (
                <div className="info-field">
                  <span className="info-label">Tier</span>
                  <span className={`info-value tier-badge ${tierInfo.class}`}>
                    {tierInfo.label}
                  </span>
                </div>
              )}
              <div className="info-field">
                <span className="info-label">Cached</span>
                <span
                  className={`info-value ${file.isCached ? 'cached-yes' : 'cached-no'}`}
                >
                  {file.isCached ? 'Yes' : 'No'}
                </span>
              </div>
              {file.canWarm && sourceCategory !== 'local' && (
                <div className="info-field">
                  <span className="info-label">Status</span>
                  <span className="info-value">
                    {file.isWarmed ? 'Warmed' : 'Requires warming'}
                  </span>
                </div>
              )}
            </div>
          </section>

          {/* Media Info Section (only for video/audio) */}
          {isMedia && (
            <section className="info-section">
              <h3 className="info-section-title">Media</h3>
              <div className="info-grid">
                {file.duration !== undefined && (
                  <div className="info-field">
                    <span className="info-label">Duration</span>
                    <span className="info-value">
                      {formatDuration(file.duration)}
                    </span>
                  </div>
                )}
                {isVideo && (
                  <>
                    <div className="info-field">
                      <span className="info-label">Resolution</span>
                      <span className="info-value">
                        {getResolutionLabel(file.width, file.height)}
                      </span>
                    </div>
                    <div className="info-field">
                      <span className="info-label">Frame Rate</span>
                      <span className="info-value">
                        {file.frameRate
                          ? `${file.frameRate.toFixed(2)} fps`
                          : '—'}
                      </span>
                    </div>
                    <div className="info-field">
                      <span className="info-label">Video Codec</span>
                      <span className="info-value codec">
                        {file.videoCodec?.toUpperCase() || '—'}
                      </span>
                    </div>
                    <div className="info-field">
                      <span className="info-label">Video Bitrate</span>
                      <span className="info-value">
                        {formatBitrate(file.videoBitrate)}
                      </span>
                    </div>
                    <div className="info-field">
                      <span className="info-label">Color Space</span>
                      <span className="info-value">
                        {file.colorSpace || '—'}
                      </span>
                    </div>
                    <div className="info-field">
                      <span className="info-label">HDR</span>
                      <span
                        className={`info-value ${file.hdrFormat ? 'hdr-badge' : ''}`}
                      >
                        {getHdrLabel(file.hdrFormat)}
                      </span>
                    </div>
                  </>
                )}
                <div className="info-field">
                  <span className="info-label">Audio Codec</span>
                  <span className="info-value codec">
                    {file.audioCodec?.toUpperCase() || '—'}
                  </span>
                </div>
                <div className="info-field">
                  <span className="info-label">Audio Channels</span>
                  <span className="info-value">
                    {getAudioChannelLabel(file.audioChannels)}
                  </span>
                </div>
                {file.audioSampleRate && (
                  <div className="info-field">
                    <span className="info-label">Sample Rate</span>
                    <span className="info-value">
                      {(file.audioSampleRate / 1000).toFixed(1)} kHz
                    </span>
                  </div>
                )}
                {file.audioBitrate && (
                  <div className="info-field">
                    <span className="info-label">Audio Bitrate</span>
                    <span className="info-value">
                      {formatBitrate(file.audioBitrate)}
                    </span>
                  </div>
                )}
                {file.container && (
                  <div className="info-field">
                    <span className="info-label">Container</span>
                    <span className="info-value">
                      {file.container.toUpperCase()}
                    </span>
                  </div>
                )}
              </div>
            </section>
          )}

          {/* Metadata & Tags Section - DAM/MAM Standard */}
          <section className="info-section">
            <h3 className="info-section-title">Metadata & Tags</h3>

            {/* Favorite Toggle */}
            <div className="info-field inline">
              <button
                className={`favorite-button ${isFavorite ? 'active' : ''}`}
                onClick={() => onToggleFavorite?.(file)}
              >
                <IconStar size={20} glow={isFavorite} />
                <span>{isFavorite ? 'Favorited' : 'Add to Favorites'}</span>
              </button>
            </div>

            {/* AI Powered Features Section */}
            {(isTaggableFile() || isMedia) && (
              <div className="ai-features-section">
                <div className="ai-features-header">
                  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--vfs-accent)" strokeWidth="2" className="ai-features-icon">
                    <path d="M12 3l1.912 5.813a2 2 0 0 0 1.275 1.275L21 12l-5.813 1.912a2 2 0 0 0-1.275 1.275L12 21l-1.912-5.813a2 2 0 0 0-1.275-1.275L3 12l5.813-1.912a2 2 0 0 0 1.275-1.275L12 3z" />
                  </svg>
                  <div className="ai-features-title-section">
                    <h4 className="ai-features-title">AI Powered Features</h4>
                    <span className="ai-features-subtitle">Automatic analysis and generation</span>
                  </div>
                </div>

                <div className="ai-features-grid">
                  {/* AI Tag Generation Card */}
                  {isTaggableFile() && (
                    <div className="ai-feature-card">
                      <div className="ai-feature-card-header">
                        <div className="ai-feature-icon-wrapper tag-icon">
                          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                            <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"></path>
                            <line x1="7" y1="7" x2="7.01" y2="7"></line>
                          </svg>
                        </div>
                        <div className="ai-feature-card-title-section">
                          <h5 className="ai-feature-card-title">Smart Tagging</h5>
                          <p className="ai-feature-card-description">
                            AI analyzes content and suggests relevant tags
                          </p>
                        </div>
                        <button
                          className="ai-feature-settings-btn"
                          onClick={() => setShowTagSettings(!showTagSettings)}
                          title="Tag settings"
                          aria-label="Toggle tag settings"
                        >
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                            <circle cx="12" cy="12" r="3" />
                            <path d="M12 1v6m0 6v6m6-11h-6M1 12h6" />
                          </svg>
                        </button>
                      </div>

                      {showTagSettings && (
                        <div className="ai-feature-settings">
                          <div className="ai-feature-setting-row">
                            <label htmlFor="max-tags-input">Tags to generate:</label>
                            <div className="ai-feature-number-input">
                              <button 
                                onClick={() => handleMaxTagsChange(maxTagsToGenerate - 1)}
                                disabled={maxTagsToGenerate <= 1}
                                aria-label="Decrease"
                              >
                                −
                              </button>
                              <input 
                                id="max-tags-input"
                                type="number"
                                value={maxTagsToGenerate}
                                onChange={(e) => handleMaxTagsChange(parseInt(e.target.value) || 1)}
                                min="1"
                                max="10"
                                className="ai-feature-number-value"
                              />
                              <button 
                                onClick={() => handleMaxTagsChange(maxTagsToGenerate + 1)}
                                disabled={maxTagsToGenerate >= 10}
                                aria-label="Increase"
                              >
                                +
                              </button>
                            </div>
                          </div>
                          <p className="ai-feature-setting-hint">
                            Up to 5 tags used for AI-powered search
                          </p>
                        </div>
                      )}

                      <button
                        className="ai-feature-action-btn primary"
                        onClick={handleGenerateTags}
                        disabled={isAutoTagging || !sourceId}
                      >
                        {isAutoTagging ? (
                          <>
                            <span className="spinner-small" />
                            <span>Analyzing...</span>
                          </>
                        ) : (
                          <>
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                              <path d="M12 3l1.912 5.813a2 2 0 0 0 1.275 1.275L21 12l-5.813 1.912a2 2 0 0 0-1.275 1.275L12 21l-1.912-5.813a2 2 0 0 0-1.275-1.275L3 12l5.813-1.912a2 2 0 0 0 1.275-1.275L12 3z" />
                            </svg>
                            <span>Generate {maxTagsToGenerate} Tag{maxTagsToGenerate !== 1 ? 's' : ''}</span>
                          </>
                        )}
                      </button>
                    </div>
                  )}

                  {/* AI Transcription Card */}
                  {isMedia && (
                    <div className="ai-feature-card">
                      <div className="ai-feature-card-header">
                        <div className="ai-feature-icon-wrapper transcription-icon">
                          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                            <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"></path>
                            <path d="M19 10v2a7 7 0 0 1-14 0v-2"></path>
                            <line x1="12" y1="19" x2="12" y2="23"></line>
                            <line x1="8" y1="23" x2="16" y2="23"></line>
                          </svg>
                        </div>
                        <div className="ai-feature-card-title-section">
                          <h5 className="ai-feature-card-title">Transcription</h5>
                          <p className="ai-feature-card-description">
                            Extract speech to text with timestamps
                          </p>
                        </div>
                      </div>

                      <button
                        className="ai-feature-action-btn secondary"
                        onClick={() => setShowTranscriptionModal(true)}
                      >
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                          <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"></path>
                          <path d="M19 10v2a7 7 0 0 1-14 0v-2"></path>
                        </svg>
                        <span>Generate Transcript</span>
                      </button>
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Tags - Searchable keywords for DAM/MAM */}
            <div className="info-field tags-field">
              <div className="tags-header">
                <span className="info-label">Tags</span>
                <span className="tags-count">({file.tags?.length || 0})</span>
              </div>
              <span className="info-hint">
                Use tag:keyword in search to find assets
              </span>
              <div className="tags-container">
                {(file.tags || []).map((tag) => {
                  const tagName = typeof tag === 'string' ? tag : tag.name;
                  const tagColor =
                    typeof tag === 'string'
                      ? tagColors[tag] || '#8E8E93'
                      : tag.color || tagColors[tagName] || '#8E8E93';
                  return (
                    <span
                      key={tagName}
                      className="tag-chip"
                      style={{
                        borderColor: tagColor,
                        backgroundColor: `${tagColor}15`,
                      }}
                    >
                      <span
                        className="tag-dot"
                        style={{ backgroundColor: tagColor }}
                      />
                      <span className="tag-name">{tagName}</span>
                      <button
                        className="tag-remove"
                        onClick={() => onRemoveTag?.(file, tagName)}
                        title={`Remove "${tagName}" tag`}
                        aria-label={`Remove ${tagName}`}
                      >
                        ×
                      </button>
                    </span>
                  );
                })}
                <div className="tag-input-wrapper">
                  <input
                    type="color"
                    value={newTagColor}
                    onChange={(e) => setNewTagColor(e.target.value)}
                    className="tag-color-picker"
                    title="Choose tag color"
                    aria-label="Tag color"
                  />
                  <input
                    type="text"
                    placeholder="Add tag..."
                    value={newTag}
                    onChange={(e) => setNewTag(e.target.value)}
                    onKeyPress={handleKeyPress}
                    className="tag-input"
                    aria-label="New tag name"
                  />
                  <button
                    className="tag-add"
                    onClick={handleAddTag}
                    disabled={!newTag.trim()}
                    title="Add tag (Enter)"
                    aria-label="Add tag"
                  >
                    +
                  </button>
                </div>
              </div>

              {/* Available Tags (existing tags with colors) */}
              {availableTags.length > 0 && (
                <div className="available-tags">
                  <span className="available-tags-label">Available:</span>
                  <div className="available-tags-list">
                    {availableTags
                      .filter((t) => {
                        const fileTagNames = (file.tags || []).map((tag) =>
                          typeof tag === 'string' ? tag : tag.name,
                        );
                        return !fileTagNames.includes(t.name);
                      })
                      .slice(0, 8)
                      .map((tag) => {
                        const tagColor = tag.color || '#8E8E93';
                        return (
                          <button
                            key={tag.name}
                            className="available-tag"
                            onClick={() =>
                              onAddTag?.(file, {
                                name: tag.name,
                                color: tag.color,
                              })
                            }
                            style={{
                              borderColor: tagColor,
                              backgroundColor: `${tagColor}15`,
                            }}
                            title={`Add "${tag.name}" tag`}
                          >
                            <span
                              className="tag-dot"
                              style={{ backgroundColor: tagColor }}
                            />
                            {tag.name}
                          </button>
                        );
                      })}
                  </div>
                </div>
              )}

              {/* Suggested Tags based on file type */}
              <div className="suggested-tags">
                <span className="suggested-label">Suggested:</span>
                {getSuggestedTags(file)
                  .filter((tag) => {
                    const fileTagNames = (file.tags || []).map((t) =>
                      typeof t === 'string' ? t : t.name,
                    );
                    return !fileTagNames.includes(tag);
                  })
                  .slice(0, 5)
                  .map((tag) => (
                    <button
                      key={tag}
                      className="suggested-tag"
                      onClick={() => onAddTag?.(file, tag)}
                      title={`Add "${tag}" tag`}
                    >
                      + {tag}
                    </button>
                  ))}
              </div>
            </div>

            {/* Color Label */}
            <section className="info-section color-label-section">
              <h3 className="info-section-title">Color Label</h3>
              <div className="color-labels">
                {COLOR_LABELS.map(({ name, value }) => (
                  <button
                    key={name}
                    className={`color-label-btn ${file.colorLabel === value ? 'active' : ''}`}
                    data-color={value || undefined}
                    onClick={() => onSetColorLabel?.(file, value)}
                    title={name}
                  >
                    {file.colorLabel === value && (
                      <span className="check">✓</span>
                    )}
                  </button>
                ))}
              </div>
            </section>

            {/* Comments */}
            <div className="info-field comments-field">
              <span className="info-label">Comments</span>
              {isEditingComments ? (
                <div className="comments-editor">
                  <textarea
                    value={comments}
                    onChange={(e) => setComments(e.target.value)}
                    placeholder="Add comments about this file..."
                    rows={3}
                  />
                  <div className="comments-actions">
                    <button
                      className="cancel-btn"
                      onClick={() => setIsEditingComments(false)}
                    >
                      Cancel
                    </button>
                    <button className="save-btn" onClick={handleSaveComments}>
                      Save
                    </button>
                  </div>
                </div>
              ) : (
                <div
                  className="comments-display"
                  onClick={() => setIsEditingComments(true)}
                >
                  {comments || (
                    <span className="placeholder">
                      Click to add comments...
                    </span>
                  )}
                </div>
              )}
            </div>
          </section>
        </div>
        </div>
      </div>
    </>
  );
};

export default InfoModal;
