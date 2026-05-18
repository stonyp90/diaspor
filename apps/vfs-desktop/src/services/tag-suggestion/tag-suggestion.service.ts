/**
 * Tag Suggestion Service
 *
 * Service for generating AI tag suggestions for videos using Ollama
 * Only processes videos from mounted storage (local/network)
 */

import { invoke } from '@tauri-apps/api/core';

export interface TagSuggestionConfig {
  enabled: boolean;
  mode: 'background' | 'on-demand';
  model: string;
  autoApprove: boolean;
  onlyMountedStorage: boolean;
}

export interface SuggestedTag {
  name: string;
  confidence: number;
}

export interface TagSuggestionRequest {
  sourceId: string;
  filePath: string;
  fileName: string;
}

export interface TagSuggestionResult {
  suggestionId: string;
  tags: SuggestedTag[];
}

const STORAGE_KEY = 'ai_tag_suggestions_config';

/**
 * Load configuration from localStorage
 */
export function loadTagSuggestionConfig(): TagSuggestionConfig {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved) {
      return JSON.parse(saved) as TagSuggestionConfig;
    }
  } catch (err) {
    console.error('Failed to load tag suggestion config:', err);
  }
  return {
    enabled: false,
    mode: 'on-demand',
    model: 'llama3.2',
    autoApprove: false,
    onlyMountedStorage: true,
  };
}

/**
 * Save configuration to localStorage
 */
export function saveTagSuggestionConfig(config: TagSuggestionConfig): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
  } catch (err) {
    console.error('Failed to save tag suggestion config:', err);
  }
}

/**
 * Check if a storage source is mounted (local/network)
 */
export function isMountedStorage(category: string): boolean {
  return category === 'local' || category === 'network' || category === 'block';
}

/**
 * Check if a file is a video
 */
export function isVideoFile(fileName: string, mimeType?: string): boolean {
  const videoExtensions = [
    'mp4',
    'mov',
    'avi',
    'mkv',
    'webm',
    'wmv',
    'flv',
    'm4v',
    'mpg',
    'mpeg',
    '3gp',
    'mxf',
    'prores',
    'r3d',
    'braw',
  ];
  const ext = fileName.split('.').pop()?.toLowerCase() || '';
  return (
    videoExtensions.includes(ext) || mimeType?.startsWith('video/') === true
  );
}

/**
 * Generate tag suggestions for a video file using Ollama
 */
export async function suggestTags(
  request: TagSuggestionRequest,
): Promise<TagSuggestionResult> {
  try {
    const result = await invoke<TagSuggestionResult>('suggest_video_tags', {
      sourceId: request.sourceId,
      filePath: request.filePath,
      fileName: request.fileName,
    });
    return result;
  } catch (err) {
    console.error('Failed to suggest tags:', err);
    throw err;
  }
}

/**
 * Request tag suggestions for a video (on-demand)
 */
export async function requestTagSuggestions(
  sourceId: string,
  filePath: string,
  fileName: string,
  category: string,
): Promise<void> {
  const config = loadTagSuggestionConfig();

  // Check if enabled
  if (!config.enabled) {
    return;
  }

  // Check if only mounted storage is enabled
  if (config.onlyMountedStorage && !isMountedStorage(category)) {
    return;
  }

  // Check if it's a video file
  if (!isVideoFile(fileName)) {
    return;
  }

  // Generate suggestions
  try {
    const result = await suggestTags({ sourceId, filePath, fileName });

    // If auto-approve is enabled, automatically approve tags
    if (config.autoApprove && result.tags.length > 0) {
      await invoke('approve_tag_suggestions', {
        suggestionId: result.suggestionId,
        tags: result.tags.map((t) => t.name),
      });
    }
  } catch (err) {
    console.error('Failed to request tag suggestions:', err);
  }
}

/**
 * Start background task for tag suggestions
 */
export async function startBackgroundTagSuggestions(): Promise<void> {
  const config = loadTagSuggestionConfig();
  if (!config.enabled || config.mode !== 'background') {
    return;
  }

  try {
    await invoke('start_background_tag_suggestions', {
      model: config.model,
      onlyMountedStorage: config.onlyMountedStorage,
    });
  } catch (err) {
    console.error('Failed to start background tag suggestions:', err);
  }
}

/**
 * Stop background task for tag suggestions
 */
export async function stopBackgroundTagSuggestions(): Promise<void> {
  try {
    await invoke('stop_background_tag_suggestions');
  } catch (err) {
    console.error('Failed to stop background tag suggestions:', err);
  }
}
