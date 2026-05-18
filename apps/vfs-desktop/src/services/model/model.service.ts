/**
 * Model Service
 *
 * Abstraction layer for AI models - exposes Transcription and VideoTagging categories
 * instead of raw model names
 */
import { invoke } from '@tauri-apps/api/core';

export enum ModelCategory {
  Transcription = 'Transcription',
  VideoTagging = 'VideoTagging',
  ImageTagging = 'ImageTagging',
  TextGeneration = 'TextGeneration',
  CodeGeneration = 'CodeGeneration',
  Embedding = 'Embedding',
  Other = 'Other',
}

export interface ModelMetadata {
  id: string;
  name: string;
  category: ModelCategory;
  description?: string;
  sizeBytes?: number;
  tags: string[];
  provider: string;
  isInstalled: boolean;
  isRunning: boolean;
  canInstall: boolean;
  canUninstall: boolean;
  canRun: boolean;
  canStop: boolean;
  downloadUrl?: string;
  homepageUrl?: string;
}

export interface ModelProgress {
  modelId: string;
  status: string;
  percentage: number;
  currentBytes: number;
  totalBytes: number;
  speedBytesPerSec?: number;
  estimatedTimeRemainingSec?: number;
  error?: string;
}

/**
 * Get all available models, grouped by category
 */
export async function getModelsByCategory(): Promise<
  Record<ModelCategory, ModelMetadata[]>
> {
  try {
    const models = await invoke<ModelMetadata[]>('vfs_list_models');

    const grouped: Record<ModelCategory, ModelMetadata[]> = {
      [ModelCategory.Transcription]: [],
      [ModelCategory.VideoTagging]: [],
      [ModelCategory.ImageTagging]: [],
      [ModelCategory.TextGeneration]: [],
      [ModelCategory.CodeGeneration]: [],
      [ModelCategory.Embedding]: [],
      [ModelCategory.Other]: [],
    };

    models.forEach((model) => {
      if (grouped[model.category]) {
        grouped[model.category].push(model);
      } else {
        grouped[ModelCategory.Other].push(model);
      }
    });

    return grouped;
  } catch (error) {
    console.error('Failed to get models by category:', error);
    return {
      [ModelCategory.Transcription]: [],
      [ModelCategory.VideoTagging]: [],
      [ModelCategory.ImageTagging]: [],
      [ModelCategory.TextGeneration]: [],
      [ModelCategory.CodeGeneration]: [],
      [ModelCategory.Embedding]: [],
      [ModelCategory.Other]: [],
    };
  }
}

/**
 * Get transcription models only
 */
export async function getTranscriptionModels(): Promise<ModelMetadata[]> {
  try {
    return await invoke<ModelMetadata[]>('vfs_list_models_by_category', {
      category: ModelCategory.Transcription,
    });
  } catch (error) {
    console.error('Failed to get transcription models:', error);
    return [];
  }
}

/**
 * Get video tagging models only
 */
export async function getVideoTaggingModels(): Promise<ModelMetadata[]> {
  try {
    return await invoke<ModelMetadata[]>('vfs_list_models_by_category', {
      category: ModelCategory.VideoTagging,
    });
  } catch (error) {
    console.error('Failed to get video tagging models:', error);
    return [];
  }
}

/**
 * Install a model
 */
export async function installModel(
  modelId: string,
): Promise<{ operationId: string }> {
  const result = await invoke<{ operation_id: string }>('vfs_install_model', {
    model_id: modelId,
  });
  return { operationId: result.operation_id };
}

/**
 * Get model installation progress
 */
export async function getModelProgress(
  operationId: string,
): Promise<ModelProgress> {
  return await invoke<ModelProgress>('vfs_get_model_operation_progress', {
    operation_id: operationId,
  });
}

/**
 * Start serving a model
 */
export async function startModel(
  modelId: string,
): Promise<{ operationId: string }> {
  const result = await invoke<{ operation_id: string }>('vfs_start_model', {
    model_id: modelId,
  });
  return { operationId: result.operation_id };
}

/**
 * Stop serving a model
 */
export async function stopModel(modelId: string): Promise<void> {
  await invoke('vfs_stop_model', {
    model_id: modelId,
  });
}
