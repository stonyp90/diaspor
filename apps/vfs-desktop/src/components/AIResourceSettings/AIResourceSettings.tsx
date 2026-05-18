/**
 * AIResourceSettings Component
 *
 * Manages resource allocation (GPU, CPU, memory) for AI features:
 * - Video transcoding (FFmpeg)
 * - Auto-tagging (Ollama)
 * - Transcription (FFmpeg + Ollama)
 */

import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Select } from '../Select/Select';
import { useToast } from '../Toast';
import './AIResourceSettings.css';

interface GpuInfo {
  id: number;
  name: string;
  vendor: string;
  memory_total_mb: number;
}

interface SystemInfo {
  os_name: string;
  os_version: string;
  kernel_version: string;
  hostname: string;
  cpu_brand: string;
  cpu_cores: number;
}

export interface TranscodingResourceLimits {
  // FFmpeg settings
  threads: number; // Number of CPU threads (0 = auto)
  useGpu: boolean; // Enable GPU acceleration
  gpuDevice: number; // GPU device index (-1 = auto)
  memoryLimitMB: number; // Memory limit in MB (0 = unlimited)
  // Quality/Performance tradeoff
  preset:
    | 'ultrafast'
    | 'superfast'
    | 'veryfast'
    | 'faster'
    | 'fast'
    | 'medium'
    | 'slow'
    | 'slower'
    | 'veryslow';
  // Concurrent jobs
  maxConcurrentJobs: number; // Max simultaneous transcoding jobs
}

export interface AutoTaggingResourceLimits {
  // Ollama settings (same as OllamaManager)
  gpuMemoryLimitGB: number; // Max GPU memory in GB (0 = unlimited)
  cpuCoresLimit: number; // Max CPU cores (0 = unlimited)
  systemMemoryLimitGB: number; // Max system RAM in GB (0 = unlimited)
  gpuUtilizationPercent: number; // Max GPU utilization % (0-100, 0 = unlimited)
  numGpu: number; // Number of GPUs to use (0 = all available)
}

const DEFAULT_TRANSCODING_LIMITS: TranscodingResourceLimits = {
  threads: 0, // Auto
  useGpu: true,
  gpuDevice: -1, // Auto
  memoryLimitMB: 0, // Unlimited
  preset: 'fast',
  maxConcurrentJobs: 1,
};

const DEFAULT_AUTO_TAGGING_LIMITS: AutoTaggingResourceLimits = {
  gpuMemoryLimitGB: 0, // Unlimited
  cpuCoresLimit: 0, // Unlimited
  systemMemoryLimitGB: 0, // Unlimited
  gpuUtilizationPercent: 0, // Unlimited
  numGpu: 0, // All GPUs
};

const STORAGE_KEY_TRANSCODING = 'ai_transcoding_resource_limits';
const STORAGE_KEY_AUTO_TAGGING = 'ai_auto_tagging_resource_limits';

interface AIResourceSettingsProps {
  onSettingsChange?: () => void;
}

export const AIResourceSettings: React.FC<AIResourceSettingsProps> = ({
  onSettingsChange,
}) => {
  const { showToast } = useToast();
  const [transcodingLimits, setTranscodingLimits] =
    useState<TranscodingResourceLimits>(DEFAULT_TRANSCODING_LIMITS);
  const [autoTaggingLimits, setAutoTaggingLimits] =
    useState<AutoTaggingResourceLimits>(DEFAULT_AUTO_TAGGING_LIMITS);
  const [gpus, setGpus] = useState<GpuInfo[]>([]);
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [activeTab, setActiveTab] = useState<'transcoding' | 'auto-tagging'>(
    'transcoding',
  );

  // Load system info
  useEffect(() => {
    const loadSystemInfo = async () => {
      try {
        const info = await invoke<SystemInfo>('get_system_info');
        setSystemInfo(info);
        const gpuList = await invoke<GpuInfo[]>('get_gpu_info');
        setGpus(gpuList || []);
      } catch (err) {
        console.error('Failed to load system/GPU info:', err);
      }
    };
    loadSystemInfo();
  }, []);

  // Load saved settings
  useEffect(() => {
    try {
      const savedTranscoding = localStorage.getItem(STORAGE_KEY_TRANSCODING);
      if (savedTranscoding) {
        const parsed = JSON.parse(
          savedTranscoding,
        ) as TranscodingResourceLimits;
        setTranscodingLimits({ ...DEFAULT_TRANSCODING_LIMITS, ...parsed });
      }

      const savedAutoTagging = localStorage.getItem(STORAGE_KEY_AUTO_TAGGING);
      if (savedAutoTagging) {
        const parsed = JSON.parse(
          savedAutoTagging,
        ) as AutoTaggingResourceLimits;
        setAutoTaggingLimits({ ...DEFAULT_AUTO_TAGGING_LIMITS, ...parsed });
      }
    } catch (err) {
      console.error('Failed to load resource limits:', err);
    }
  }, []);

  // Save transcoding limits
  const saveTranscodingLimits = useCallback(
    (limits: TranscodingResourceLimits) => {
      setTranscodingLimits(limits);
      try {
        localStorage.setItem(STORAGE_KEY_TRANSCODING, JSON.stringify(limits));
        // Also save to backend (convert camelCase to snake_case)
        invoke('save_transcoding_resource_limits', {
          limits: {
            threads: limits.threads,
            use_gpu: limits.useGpu,
            gpu_device: limits.gpuDevice,
            memory_limit_mb: limits.memoryLimitMB,
            preset: limits.preset,
            max_concurrent_jobs: limits.maxConcurrentJobs,
          },
        }).catch((err) => {
          console.error('Failed to save transcoding limits:', err);
        });
        onSettingsChange?.();
      } catch (err) {
        console.error('Failed to save transcoding limits:', err);
      }
    },
    [onSettingsChange],
  );

  // Save auto-tagging limits
  const saveAutoTaggingLimits = useCallback(
    (limits: AutoTaggingResourceLimits) => {
      setAutoTaggingLimits(limits);
      try {
        localStorage.setItem(STORAGE_KEY_AUTO_TAGGING, JSON.stringify(limits));
        // Also save to backend (convert camelCase to snake_case)
        invoke('save_auto_tagging_resource_limits', {
          limits: {
            gpu_memory_limit_gb: limits.gpuMemoryLimitGB,
            cpu_cores_limit: limits.cpuCoresLimit,
            system_memory_limit_gb: limits.systemMemoryLimitGB,
            gpu_utilization_percent: limits.gpuUtilizationPercent,
            num_gpu: limits.numGpu,
          },
        })
          .then(() => {
            showToast({
              type: 'success',
              message: 'Auto-tagging settings saved successfully',
            });
          })
          .catch((err) => {
            console.error('Failed to save auto-tagging limits:', err);
            showToast({
              type: 'error',
              message: 'Failed to save auto-tagging settings',
            });
          });
        onSettingsChange?.();
      } catch (err) {
        console.error('Failed to save auto-tagging limits:', err);
        showToast({
          type: 'error',
          message: 'Failed to save auto-tagging settings',
        });
      }
    },
    [onSettingsChange, showToast],
  );

  return (
    <div className="ai-resource-settings">
      <div className="resource-settings-header">
        <h3>AI Resource Allocation</h3>
        <p className="resource-settings-description">
          Control how much GPU, CPU, and memory resources are allocated to AI
          features. This prevents these features from consuming all system
          resources.
        </p>
      </div>

      <div className="resource-settings-tabs">
        <button
          className={`resource-tab ${activeTab === 'transcoding' ? 'active' : ''}`}
          onClick={() => setActiveTab('transcoding')}
        >
          Video Transcoding
        </button>
        <button
          className={`resource-tab ${activeTab === 'auto-tagging' ? 'active' : ''}`}
          onClick={() => setActiveTab('auto-tagging')}
        >
          Auto-Tagging
        </button>
      </div>

      {activeTab === 'transcoding' && (
        <TranscodingSettings
          limits={transcodingLimits}
          onSave={saveTranscodingLimits}
          gpus={gpus}
          systemInfo={systemInfo}
        />
      )}

      {activeTab === 'auto-tagging' && (
        <AutoTaggingSettings
          limits={autoTaggingLimits}
          onSave={saveAutoTaggingLimits}
          gpus={gpus}
          systemInfo={systemInfo}
        />
      )}
    </div>
  );
};

// Transcoding Settings Component
interface TranscodingSettingsProps {
  limits: TranscodingResourceLimits;
  onSave: (limits: TranscodingResourceLimits) => void;
  gpus: GpuInfo[];
  systemInfo: SystemInfo | null;
}

function TranscodingSettings({
  limits,
  onSave,
  gpus,
  systemInfo,
}: TranscodingSettingsProps) {
  const [localLimits, setLocalLimits] =
    useState<TranscodingResourceLimits>(limits);

  useEffect(() => {
    setLocalLimits(limits);
  }, [limits]);

  const handleSave = () => {
    onSave(localLimits);
  };

  return (
    <div className="resource-settings-content">
      <div className="resource-setting">
        <label htmlFor="transcoding-threads">
          <span className="setting-name">CPU Threads</span>
          <span className="setting-desc">
            Number of CPU threads for transcoding (0 = auto-detect). More
            threads = faster but uses more CPU.
            {systemInfo && ` Available: ${systemInfo.cpu_cores} cores`}
          </span>
        </label>
        <div className="setting-input-group">
          <input
            type="number"
            id="transcoding-threads"
            min="0"
            max={systemInfo?.cpu_cores || 32}
            step="1"
            value={localLimits.threads}
            onChange={(e) =>
              setLocalLimits({
                ...localLimits,
                threads: parseInt(e.target.value) || 0,
              })
            }
          />
          <span className="input-unit">
            {localLimits.threads === 0 ? 'Auto' : 'threads'}
          </span>
        </div>
      </div>

      <div className="resource-setting">
        <label htmlFor="transcoding-use-gpu">
          <input
            type="checkbox"
            id="transcoding-use-gpu"
            checked={localLimits.useGpu}
            onChange={(e) =>
              setLocalLimits({ ...localLimits, useGpu: e.target.checked })
            }
          />
          <span className="setting-name">Enable GPU Acceleration</span>
          <span className="setting-desc">
            Use GPU for video encoding/decoding (much faster, but requires
            compatible GPU and drivers)
            {gpus.length > 0 &&
              `. Available: ${gpus.map((g) => g.name).join(', ')}`}
          </span>
        </label>
      </div>

      {localLimits.useGpu && gpus.length > 0 && (
        <div className="resource-setting">
          <div className="setting-label-group">
            <span className="setting-name">GPU Device</span>
            <span className="setting-desc">
              Select which GPU to use for transcoding (-1 = auto-select)
            </span>
          </div>
          <div className="setting-input-group">
            <Select<number>
              value={localLimits.gpuDevice}
              options={[
                { value: -1, label: 'Auto-select' },
                ...gpus.map((gpu, idx) => ({
                  value: idx,
                  label: `${gpu.name} (${Math.round(gpu.memory_total_mb / 1024)}GB)`,
                })),
              ]}
              onChange={(value) =>
                setLocalLimits({
                  ...localLimits,
                  gpuDevice: value,
                })
              }
              fullWidth
            />
          </div>
        </div>
      )}

      <div className="resource-setting">
        <div className="setting-label-group">
          <span className="setting-name">Encoding Preset</span>
          <span className="setting-desc">
            Balance between encoding speed and file size. Faster = larger files
            but quicker encoding.
          </span>
        </div>
        <div className="setting-input-group">
          <Select<TranscodingResourceLimits['preset']>
            value={localLimits.preset}
            options={[
              {
                value: 'ultrafast',
                label: 'Ultrafast (fastest, largest files)',
              },
              { value: 'superfast', label: 'Superfast' },
              { value: 'veryfast', label: 'Veryfast' },
              { value: 'faster', label: 'Faster' },
              { value: 'fast', label: 'Fast (recommended)' },
              { value: 'medium', label: 'Medium' },
              { value: 'slow', label: 'Slow' },
              { value: 'slower', label: 'Slower' },
              {
                value: 'veryslow',
                label: 'Veryslow (slowest, smallest files)',
              },
            ]}
            onChange={(value) =>
              setLocalLimits({
                ...localLimits,
                preset: value,
              })
            }
            fullWidth
          />
        </div>
      </div>

      <div className="resource-setting">
        <label htmlFor="transcoding-memory">
          <span className="setting-name">Memory Limit</span>
          <span className="setting-desc">
            Maximum memory for transcoding in MB (0 = unlimited). Prevents
            transcoding from using all available RAM.
          </span>
        </label>
        <div className="setting-input-group">
          <input
            type="number"
            id="transcoding-memory"
            min="0"
            max="32768"
            step="512"
            value={localLimits.memoryLimitMB}
            onChange={(e) =>
              setLocalLimits({
                ...localLimits,
                memoryLimitMB: parseInt(e.target.value) || 0,
              })
            }
          />
          <span className="input-unit">
            {localLimits.memoryLimitMB === 0 ? 'Unlimited' : 'MB'}
          </span>
        </div>
      </div>

      <div className="resource-setting">
        <label htmlFor="transcoding-concurrent">
          <span className="setting-name">Max Concurrent Jobs</span>
          <span className="setting-desc">
            Maximum number of simultaneous transcoding jobs. More jobs = faster
            batch processing but uses more resources.
          </span>
        </label>
        <div className="setting-input-group">
          <input
            type="number"
            id="transcoding-concurrent"
            min="1"
            max="8"
            step="1"
            value={localLimits.maxConcurrentJobs}
            onChange={(e) =>
              setLocalLimits({
                ...localLimits,
                maxConcurrentJobs: parseInt(e.target.value) || 1,
              })
            }
          />
          <span className="input-unit">jobs</span>
        </div>
      </div>

      <div className="resource-settings-actions">
        <button className="save-btn" onClick={handleSave}>
          Save Transcoding Settings
        </button>
      </div>
    </div>
  );
}

// Auto-Tagging Settings Component
interface AutoTaggingSettingsProps {
  limits: AutoTaggingResourceLimits;
  onSave: (limits: AutoTaggingResourceLimits) => void;
  gpus: GpuInfo[];
  systemInfo: SystemInfo | null;
}

function AutoTaggingSettings({
  limits,
  onSave,
  gpus,
  systemInfo,
}: AutoTaggingSettingsProps) {
  const [localLimits, setLocalLimits] =
    useState<AutoTaggingResourceLimits>(limits);

  useEffect(() => {
    setLocalLimits(limits);
  }, [limits]);

  const handleSave = () => {
    onSave(localLimits);
  };

  return (
    <div className="resource-settings-content">
      <div className="resource-setting">
        <label htmlFor="tagging-gpu-memory">
          <span className="setting-name">GPU Memory Limit</span>
          <span className="setting-desc">
            Maximum GPU memory for auto-tagging in GB (0 = unlimited). Limits
            how much GPU memory Ollama can use.
            {gpus.length > 0 &&
              ` Available: ${gpus.map((g) => `${Math.round(g.memory_total_mb / 1024)}GB`).join(', ')}`}
          </span>
        </label>
        <div className="setting-input-group">
          <input
            type="number"
            id="tagging-gpu-memory"
            min="0"
            max={
              gpus.length > 0
                ? Math.max(...gpus.map((g) => g.memory_total_mb / 1024))
                : 100
            }
            step="0.5"
            value={localLimits.gpuMemoryLimitGB}
            onChange={(e) =>
              setLocalLimits({
                ...localLimits,
                gpuMemoryLimitGB: parseFloat(e.target.value) || 0,
              })
            }
          />
          <span className="input-unit">
            {localLimits.gpuMemoryLimitGB === 0 ? 'Unlimited' : 'GB'}
          </span>
        </div>
      </div>

      <div className="resource-setting">
        <label htmlFor="tagging-cpu-cores">
          <span className="setting-name">CPU Cores Limit</span>
          <span className="setting-desc">
            Maximum CPU cores for auto-tagging (0 = unlimited). Limits CPU usage
            by Ollama models.
            {systemInfo && ` Available: ${systemInfo.cpu_cores} cores`}
          </span>
        </label>
        <div className="setting-input-group">
          <input
            type="number"
            id="tagging-cpu-cores"
            min="0"
            max={systemInfo?.cpu_cores || 32}
            step="1"
            value={localLimits.cpuCoresLimit}
            onChange={(e) =>
              setLocalLimits({
                ...localLimits,
                cpuCoresLimit: parseInt(e.target.value) || 0,
              })
            }
          />
          <span className="input-unit">
            {localLimits.cpuCoresLimit === 0 ? 'Unlimited' : 'cores'}
          </span>
        </div>
      </div>

      <div className="resource-setting">
        <label htmlFor="tagging-system-memory">
          <span className="setting-name">System Memory Limit</span>
          <span className="setting-desc">
            Maximum system RAM for auto-tagging in GB (0 = unlimited). Prevents
            Ollama from using all available RAM.
          </span>
        </label>
        <div className="setting-input-group">
          <input
            type="number"
            id="tagging-system-memory"
            min="0"
            max="128"
            step="1"
            value={localLimits.systemMemoryLimitGB}
            onChange={(e) =>
              setLocalLimits({
                ...localLimits,
                systemMemoryLimitGB: parseFloat(e.target.value) || 0,
              })
            }
          />
          <span className="input-unit">
            {localLimits.systemMemoryLimitGB === 0 ? 'Unlimited' : 'GB'}
          </span>
        </div>
      </div>

      <div className="resource-setting">
        <label htmlFor="tagging-gpu-utilization">
          <span className="setting-name">GPU Utilization Limit</span>
          <span className="setting-desc">
            Maximum GPU utilization percentage (0-100, 0 = unlimited).
            Recommended: 50-70% to leave resources for other apps.
          </span>
        </label>
        <div className="setting-input-group">
          <input
            type="number"
            id="tagging-gpu-utilization"
            min="0"
            max="100"
            step="5"
            value={localLimits.gpuUtilizationPercent}
            onChange={(e) =>
              setLocalLimits({
                ...localLimits,
                gpuUtilizationPercent: parseInt(e.target.value) || 0,
              })
            }
          />
          <span className="input-unit">
            {localLimits.gpuUtilizationPercent === 0 ? 'Unlimited' : '%'}
          </span>
        </div>
        {localLimits.gpuUtilizationPercent > 0 && (
          <div className="slider-container">
            <input
              type="range"
              min="0"
              max="100"
              step="5"
              value={localLimits.gpuUtilizationPercent}
              onChange={(e) =>
                setLocalLimits({
                  ...localLimits,
                  gpuUtilizationPercent: parseInt(e.target.value),
                })
              }
              className="resource-slider"
            />
          </div>
        )}
      </div>

      {gpus.length > 1 && (
        <div className="resource-setting">
          <label htmlFor="tagging-num-gpu">
            <span className="setting-name">Number of GPUs</span>
            <span className="setting-desc">
              How many GPUs to use for auto-tagging (0 = use all available
              GPUs). Available: {gpus.length} GPU{gpus.length > 1 ? 's' : ''}
            </span>
          </label>
          <div className="setting-input-group">
            <input
              type="number"
              id="tagging-num-gpu"
              min="0"
              max={gpus.length}
              step="1"
              value={localLimits.numGpu}
              onChange={(e) =>
                setLocalLimits({
                  ...localLimits,
                  numGpu: parseInt(e.target.value) || 0,
                })
              }
            />
            <span className="input-unit">
              {localLimits.numGpu === 0
                ? 'All'
                : `GPU${localLimits.numGpu > 1 ? 's' : ''}`}
            </span>
          </div>
        </div>
      )}

      <div className="resource-settings-actions">
        <button className="save-btn" onClick={handleSave}>
          Save Auto-Tagging Settings
        </button>
      </div>
    </div>
  );
}

export default AIResourceSettings;
