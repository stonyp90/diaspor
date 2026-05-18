/**
 * AIAdvancedSettings - User-Friendly AI Configuration
 *
 * Designed for non-technical users with:
 * - Clear visual status indicators
 * - Smart defaults with accessible customization
 * - Progressive disclosure (simple → advanced)
 * - Beautiful glass morphism design
 */

import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Select } from '../Select/Select';
import { useToast } from '../Toast';
import './AIAdvancedSettings.css';

// ============================================================================
// Types and Interfaces
// ============================================================================

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

interface TranscodingResourceLimits {
  threads: number;
  useGpu: boolean;
  gpuDevice: number;
  memoryLimitMB: number;
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
  maxConcurrentJobs: number;
}

interface AutoTaggingResourceLimits {
  gpuMemoryLimitGB: number;
  cpuCoresLimit: number;
  systemMemoryLimitGB: number;
  gpuUtilizationPercent: number;
  numGpu: number;
}

interface AITagConfig {
  enabled: boolean;
  allowOnObjectStorage: boolean;
}

type DependencyStatus =
  | 'checking'
  | 'installed'
  | 'not-installed'
  | 'not-running';

interface DependencyCheck {
  name: string;
  status: DependencyStatus;
  required: boolean;
  description: string;
  installUrl?: string;
}

// ============================================================================
// Default Values
// ============================================================================

const DEFAULT_TRANSCODING_LIMITS: TranscodingResourceLimits = {
  threads: 0,
  useGpu: true,
  gpuDevice: -1,
  memoryLimitMB: 0,
  preset: 'fast',
  maxConcurrentJobs: 1,
};

const DEFAULT_AUTO_TAGGING_LIMITS: AutoTaggingResourceLimits = {
  gpuMemoryLimitGB: 0,
  cpuCoresLimit: 0,
  systemMemoryLimitGB: 0,
  gpuUtilizationPercent: 0,
  numGpu: 0,
};

const DEFAULT_AI_TAG_CONFIG: AITagConfig = {
  enabled: false,
  allowOnObjectStorage: false,
};

const STORAGE_KEY_TRANSCODING = 'ai_transcoding_resource_limits';
const STORAGE_KEY_AUTO_TAGGING = 'ai_auto_tagging_resource_limits';
const STORAGE_KEY_AI_TAG = 'ai_tag_suggestions_config';

// ============================================================================
// Icon Components
// ============================================================================

const CheckCircleIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
    <polyline points="22 4 12 14.01 9 11.01" />
  </svg>
);

const AlertCircleIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <circle cx="12" cy="12" r="10" />
    <line x1="12" y1="8" x2="12" y2="12" />
    <line x1="12" y1="16" x2="12.01" y2="16" />
  </svg>
);

const SparklesIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M12 3l1.912 5.813a2 2 0 0 0 1.275 1.275L21 12l-5.813 1.912a2 2 0 0 0-1.275 1.275L12 21l-1.912-5.813a2 2 0 0 0-1.275-1.275L3 12l5.813-1.912a2 2 0 0 0 1.275-1.275L12 3z" />
  </svg>
);

const CpuIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <rect x="4" y="4" width="16" height="16" rx="2" ry="2" />
    <rect x="9" y="9" width="6" height="6" />
    <line x1="9" y1="1" x2="9" y2="4" />
    <line x1="15" y1="1" x2="15" y2="4" />
    <line x1="9" y1="20" x2="9" y2="23" />
    <line x1="15" y1="20" x2="15" y2="23" />
    <line x1="20" y1="9" x2="23" y2="9" />
    <line x1="20" y1="14" x2="23" y2="14" />
    <line x1="1" y1="9" x2="4" y2="9" />
    <line x1="1" y1="14" x2="4" y2="14" />
  </svg>
);

const ZapIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
  </svg>
);

const SettingsIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
  </svg>
);

const RefreshIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <polyline points="23 4 23 10 17 10" />
    <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
  </svg>
);

// ============================================================================
// Main Component
// ============================================================================

export const AIAdvancedSettings: React.FC = () => {
  const { showToast } = useToast();
  const [aiTagConfig, setAiTagConfig] = useState<AITagConfig>(
    DEFAULT_AI_TAG_CONFIG,
  );
  const [transcodingLimits, setTranscodingLimits] =
    useState<TranscodingResourceLimits>(DEFAULT_TRANSCODING_LIMITS);
  const [autoTaggingLimits, setAutoTaggingLimits] =
    useState<AutoTaggingResourceLimits>(DEFAULT_AUTO_TAGGING_LIMITS);
  const [gpus, setGpus] = useState<GpuInfo[]>([]);
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [dependencies, setDependencies] = useState<DependencyCheck[]>([]);
  const [activeSection, setActiveSection] = useState<'tagging' | 'resources'>(
    'tagging',
  );
  const [expandedResource, setExpandedResource] = useState<
    'transcoding' | 'tagging' | null
  >(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  // ============================================================================
  // Dependency Checks
  // ============================================================================

  const checkDocker = useCallback(async (): Promise<DependencyStatus> => {
    try {
      const isInstalled = await invoke<boolean>('check_docker_installed');
      if (!isInstalled) return 'not-installed';
      const isRunning = await invoke<boolean>('check_docker_running').catch(
        () => true,
      );
      return isRunning ? 'installed' : 'not-running';
    } catch {
      return 'not-installed';
    }
  }, []);

  const checkOllama = useCallback(async (): Promise<DependencyStatus> => {
    try {
      const isRunning = await invoke<boolean>('check_ollama_running');
      if (!isRunning) return 'not-running';
      await invoke<{ models: Array<{ name: string }> }>('ollama_list');
      return 'installed';
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      if (
        errorMsg.includes('connection refused') ||
        errorMsg.includes('not running')
      ) {
        return 'not-running';
      }
      return 'not-installed';
    }
  }, []);

  const checkFFmpeg = useCallback(async (): Promise<DependencyStatus> => {
    try {
      const isAvailable = await invoke<boolean>('check_ffmpeg_installed');
      return isAvailable ? 'installed' : 'not-installed';
    } catch {
      return 'not-installed';
    }
  }, []);

  const checkAllDependencies = useCallback(async () => {
    setIsRefreshing(true);
    const deps: DependencyCheck[] = [
      {
        name: 'Ollama',
        status: 'checking',
        required: true,
        description: 'Powers AI tagging and analysis',
        installUrl: 'https://ollama.ai',
      },
      {
        name: 'FFmpeg',
        status: 'checking',
        required: true,
        description: 'Enables video transcription',
        installUrl: 'https://ffmpeg.org',
      },
      {
        name: 'Docker',
        status: 'checking',
        required: false,
        description: 'Optional for advanced deployments',
        installUrl: 'https://docker.com',
      },
    ];
    setDependencies(deps);

    const [ollamaStatus, ffmpegStatus, dockerStatus] = await Promise.all([
      checkOllama(),
      checkFFmpeg(),
      checkDocker(),
    ]);

    setDependencies([
      {
        name: 'Ollama',
        status: ollamaStatus,
        required: true,
        description: 'Powers AI tagging and analysis',
        installUrl: 'https://ollama.ai',
      },
      {
        name: 'FFmpeg',
        status: ffmpegStatus,
        required: true,
        description: 'Enables video transcription',
        installUrl: 'https://ffmpeg.org',
      },
      {
        name: 'Docker',
        status: dockerStatus,
        required: false,
        description: 'Optional for advanced deployments',
        installUrl: 'https://docker.com',
      },
    ]);
    setIsRefreshing(false);
  }, [checkDocker, checkOllama, checkFFmpeg]);

  // ============================================================================
  // Load Settings
  // ============================================================================

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

    try {
      const savedTag = localStorage.getItem(STORAGE_KEY_AI_TAG);
      if (savedTag) {
        const parsed = JSON.parse(savedTag) as AITagConfig;
        setAiTagConfig({ ...DEFAULT_AI_TAG_CONFIG, ...parsed });
      }
    } catch (err) {
      console.error('Failed to load AI tag config:', err);
    }

    try {
      const savedTranscoding = localStorage.getItem(STORAGE_KEY_TRANSCODING);
      if (savedTranscoding) {
        const parsed = JSON.parse(
          savedTranscoding,
        ) as TranscodingResourceLimits;
        setTranscodingLimits({ ...DEFAULT_TRANSCODING_LIMITS, ...parsed });
      }
    } catch (err) {
      console.error('Failed to load transcoding limits:', err);
    }

    try {
      const savedAutoTagging = localStorage.getItem(STORAGE_KEY_AUTO_TAGGING);
      if (savedAutoTagging) {
        const parsed = JSON.parse(
          savedAutoTagging,
        ) as AutoTaggingResourceLimits;
        setAutoTaggingLimits({ ...DEFAULT_AUTO_TAGGING_LIMITS, ...parsed });
      }
    } catch (err) {
      console.error('Failed to load auto-tagging limits:', err);
    }

    checkAllDependencies();
  }, [checkAllDependencies]);

  // ============================================================================
  // Save Functions
  // ============================================================================

  const saveAITagConfig = useCallback(
    (config: AITagConfig) => {
      setAiTagConfig(config);
      try {
        localStorage.setItem(STORAGE_KEY_AI_TAG, JSON.stringify(config));
        showToast({
          type: 'success',
          message: config.enabled
            ? 'AI Tag Suggestions enabled'
            : 'AI Tag Suggestions disabled',
        });
      } catch (err) {
        console.error('Failed to save AI tag config:', err);
        showToast({
          type: 'error',
          message: 'Failed to save settings',
        });
      }
    },
    [showToast],
  );

  const saveTranscodingLimits = useCallback(
    (limits: TranscodingResourceLimits) => {
      setTranscodingLimits(limits);
      try {
        localStorage.setItem(STORAGE_KEY_TRANSCODING, JSON.stringify(limits));
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
          console.error('Failed to save transcoding limits to backend:', err);
        });
        showToast({
          type: 'success',
          message: 'Transcoding settings saved',
        });
      } catch (err) {
        console.error('Failed to save transcoding limits:', err);
        showToast({
          type: 'error',
          message: 'Failed to save settings',
        });
      }
    },
    [showToast],
  );

  const saveAutoTaggingLimits = useCallback(
    (limits: AutoTaggingResourceLimits) => {
      setAutoTaggingLimits(limits);
      try {
        localStorage.setItem(STORAGE_KEY_AUTO_TAGGING, JSON.stringify(limits));
        invoke('save_auto_tagging_resource_limits', {
          limits: {
            gpu_memory_limit_gb: limits.gpuMemoryLimitGB,
            cpu_cores_limit: limits.cpuCoresLimit,
            system_memory_limit_gb: limits.systemMemoryLimitGB,
            gpu_utilization_percent: limits.gpuUtilizationPercent,
            num_gpu: limits.numGpu,
          },
        }).catch((err) => {
          console.error('Failed to save auto-tagging limits to backend:', err);
        });
        showToast({
          type: 'success',
          message: 'Auto-tagging settings saved',
        });
      } catch (err) {
        console.error('Failed to save auto-tagging limits:', err);
        showToast({
          type: 'error',
          message: 'Failed to save settings',
        });
      }
    },
    [showToast],
  );

  // ============================================================================
  // Computed Values
  // ============================================================================

  const allRequiredDepsInstalled = dependencies
    .filter((d) => d.required)
    .every((d) => d.status === 'installed');

  const installedCount = dependencies.filter(
    (d) => d.status === 'installed',
  ).length;
  const totalRequired = dependencies.filter((d) => d.required).length;

  return (
    <div className="ai-settings-container">
      {/* Header Section */}
      <div className="ai-settings-header">
        <div className="header-content">
          <div className="header-icon">
            <SparklesIcon />
          </div>
          <div className="header-text">
            <h3>AI Configuration</h3>
            <p>Fine-tune how AI features work on your system</p>
          </div>
        </div>
      </div>

      {/* Quick Status Overview */}
      <div className="status-overview">
        <div className="status-header">
          <span className="status-title">System Requirements</span>
          <button
            className={`refresh-btn ${isRefreshing ? 'refreshing' : ''}`}
            onClick={checkAllDependencies}
            disabled={isRefreshing}
            title="Check again"
          >
            <RefreshIcon />
          </button>
        </div>
        <div className="status-badges">
          {dependencies.map((dep) => (
            <div
              key={dep.name}
              className={`status-badge ${dep.status} ${dep.required ? '' : 'optional'}`}
              title={dep.description}
            >
              <span className="badge-indicator">
                {dep.status === 'checking' && (
                  <span className="badge-spinner" />
                )}
                {dep.status === 'installed' && <CheckCircleIcon />}
                {(dep.status === 'not-installed' ||
                  dep.status === 'not-running') && <AlertCircleIcon />}
              </span>
              <span className="badge-name">{dep.name}</span>
              <span className="badge-status">
                {dep.status === 'installed'
                  ? '✓'
                  : dep.status === 'not-running'
                    ? '!'
                    : dep.status === 'checking'
                      ? '...'
                      : '✗'}
              </span>
            </div>
          ))}
        </div>
        {!allRequiredDepsInstalled && (
          <div className="status-warning">
            <AlertCircleIcon />
            <span>
              Some required components are missing. AI features may not work
              correctly.
            </span>
          </div>
        )}
        {allRequiredDepsInstalled && (
          <div className="status-success">
            <CheckCircleIcon />
            <span>All required components are ready</span>
          </div>
        )}
      </div>

      {/* Navigation Pills */}
      <div className="section-nav">
        <button
          className={`nav-pill ${activeSection === 'tagging' ? 'active' : ''}`}
          onClick={() => setActiveSection('tagging')}
        >
          <SparklesIcon />
          <span>AI Features</span>
        </button>
        <button
          className={`nav-pill ${activeSection === 'resources' ? 'active' : ''}`}
          onClick={() => setActiveSection('resources')}
        >
          <CpuIcon />
          <span>Performance</span>
        </button>
      </div>

      {/* Content Sections */}
      <div className="settings-content">
        {/* AI Features Section */}
        {activeSection === 'tagging' && (
          <div className="content-section animate-in">
            {/* AI Tag Suggestions Card */}
            <div className="feature-card glass">
              <div className="feature-card-header">
                <div className="feature-icon-wrapper primary">
                  <SparklesIcon />
                </div>
                <div className="feature-info">
                  <h4>AI Tag Suggestions</h4>
                  <p>
                    Automatically analyze your media and suggest relevant tags
                  </p>
                </div>
                <label className="modern-toggle">
                  <input
                    type="checkbox"
                    checked={aiTagConfig.enabled}
                    onChange={(e) =>
                      saveAITagConfig({
                        ...aiTagConfig,
                        enabled: e.target.checked,
                      })
                    }
                    disabled={!allRequiredDepsInstalled}
                  />
                  <span className="toggle-track">
                    <span className="toggle-thumb" />
                  </span>
                </label>
              </div>

              {aiTagConfig.enabled && (
                <div className="feature-card-body">
                  <div className="info-callout">
                    <ZapIcon />
                    <div>
                      <strong>How it works</strong>
                      <p>
                        When you view media files, AI will analyze the content
                        and suggest descriptive tags. You can review and approve
                        suggestions before they're applied.
                      </p>
                    </div>
                  </div>
                  
                  {/* Object Storage Option */}
                  <div className="setting-row">
                    <div className="setting-info">
                      <label htmlFor="allow-object-storage">
                        Enable for Cloud Storage
                      </label>
                      <p className="setting-description">
                        Allow AI tagging and transcription on cloud storage files.
                        Files will be cached locally for processing (first access may take longer).
                      </p>
                    </div>
                    <label className="modern-toggle">
                      <input
                        id="allow-object-storage"
                        type="checkbox"
                        checked={aiTagConfig.allowOnObjectStorage}
                        onChange={(e) =>
                          saveAITagConfig({
                            ...aiTagConfig,
                            allowOnObjectStorage: e.target.checked,
                          })
                        }
                      />
                      <span className="toggle-track">
                        <span className="toggle-thumb" />
                      </span>
                    </label>
                  </div>
                </div>
              )}

              {!allRequiredDepsInstalled && (
                <div className="feature-card-footer warning">
                  <AlertCircleIcon />
                  <span>Requires Ollama and FFmpeg to be installed</span>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Performance Section */}
        {activeSection === 'resources' && (
          <div className="content-section animate-in">
            {/* System Info Banner */}
            {systemInfo && (
              <div className="system-info-banner glass">
                <div className="info-item">
                  <CpuIcon />
                  <div>
                    <span className="info-label">CPU</span>
                    <span className="info-value">
                      {systemInfo.cpu_cores} cores
                    </span>
                  </div>
                </div>
                {gpus.length > 0 && (
                  <div className="info-item">
                    <ZapIcon />
                    <div>
                      <span className="info-label">GPU</span>
                      <span className="info-value">
                        {gpus[0].name.slice(0, 30)}
                        {gpus[0].name.length > 30 ? '...' : ''}
                      </span>
                    </div>
                  </div>
                )}
              </div>
            )}

            {/* Video Transcoding Settings */}
            <div className="resource-card glass">
              <button
                className={`resource-card-header ${expandedResource === 'transcoding' ? 'expanded' : ''}`}
                onClick={() =>
                  setExpandedResource(
                    expandedResource === 'transcoding' ? null : 'transcoding',
                  )
                }
              >
                <div className="resource-icon-wrapper accent">
                  <SettingsIcon />
                </div>
                <div className="resource-info">
                  <h4>Video Transcoding</h4>
                  <p>
                    Control CPU/GPU usage for video processing and conversion
                  </p>
                </div>
                <svg
                  className="chevron"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>

              {expandedResource === 'transcoding' && (
                <div className="resource-card-body">
                  <TranscodingSettings
                    limits={transcodingLimits}
                    onSave={saveTranscodingLimits}
                    gpus={gpus}
                    systemInfo={systemInfo}
                  />
                </div>
              )}
            </div>

            {/* Auto-Tagging Resource Settings */}
            <div className="resource-card glass">
              <button
                className={`resource-card-header ${expandedResource === 'tagging' ? 'expanded' : ''}`}
                onClick={() =>
                  setExpandedResource(
                    expandedResource === 'tagging' ? null : 'tagging',
                  )
                }
              >
                <div className="resource-icon-wrapper primary">
                  <SparklesIcon />
                </div>
                <div className="resource-info">
                  <h4>Auto-Tagging Resources</h4>
                  <p>Limit resource usage for AI model inference</p>
                </div>
                <svg
                  className="chevron"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>

              {expandedResource === 'tagging' && (
                <div className="resource-card-body">
                  <AutoTaggingSettings
                    limits={autoTaggingLimits}
                    onSave={saveAutoTaggingLimits}
                    gpus={gpus}
                    systemInfo={systemInfo}
                  />
                </div>
              )}
            </div>

            {/* Smart Defaults Info */}
            <div className="smart-defaults-info">
              <div className="info-icon">
                <ZapIcon />
              </div>
              <div className="info-content">
                <strong>Using Smart Defaults</strong>
                <p>
                  Values set to 0 or "Auto" let the system automatically choose
                  optimal settings based on your hardware. Adjust only if you
                  need to limit resource usage.
                </p>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

// ============================================================================
// Transcoding Settings Component
// ============================================================================

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
  const [hasChanges, setHasChanges] = useState(false);

  useEffect(() => {
    setLocalLimits(limits);
    setHasChanges(false);
  }, [limits]);

  const updateLimit = <K extends keyof TranscodingResourceLimits>(
    key: K,
    value: TranscodingResourceLimits[K],
  ) => {
    setLocalLimits((prev) => ({ ...prev, [key]: value }));
    setHasChanges(true);
  };

  const handleSave = () => {
    onSave(localLimits);
    setHasChanges(false);
  };

  return (
    <div className="settings-form">
      {/* GPU Toggle */}
      <div className="form-row toggle-row">
        <div className="form-label">
          <span>GPU Acceleration</span>
          <span className="form-hint">Use hardware encoding when available</span>
        </div>
        <label className="modern-toggle small">
          <input
            type="checkbox"
            checked={localLimits.useGpu}
            onChange={(e) => updateLimit('useGpu', e.target.checked)}
          />
          <span className="toggle-track">
            <span className="toggle-thumb" />
          </span>
        </label>
      </div>

      {/* GPU Device Selection */}
      {localLimits.useGpu && gpus.length > 0 && (
        <div className="form-row">
          <div className="form-label">
            <span>GPU Device</span>
            <span className="form-hint">Select which GPU to use</span>
          </div>
          <div className="form-input">
            <Select<number>
              value={localLimits.gpuDevice}
              options={[
                { value: -1, label: 'Auto-select best GPU' },
                ...gpus.map((gpu, idx) => ({
                  value: idx,
                  label: `${gpu.name} (${Math.round(gpu.memory_total_mb / 1024)}GB)`,
                })),
              ]}
              onChange={(value) => updateLimit('gpuDevice', value)}
              fullWidth
            />
          </div>
        </div>
      )}

      {/* CPU Threads */}
      <div className="form-row">
        <div className="form-label">
          <span>CPU Threads</span>
          <span className="form-hint">
            0 = auto ({systemInfo?.cpu_cores || 'all'} available)
          </span>
        </div>
        <div className="form-input compact">
          <input
            type="number"
            min="0"
            max={systemInfo?.cpu_cores || 32}
            value={localLimits.threads}
            onChange={(e) =>
              updateLimit('threads', parseInt(e.target.value) || 0)
            }
          />
          <span className="input-suffix">
            {localLimits.threads === 0 ? 'Auto' : 'threads'}
          </span>
        </div>
      </div>

      {/* Encoding Preset */}
      <div className="form-row">
        <div className="form-label">
          <span>Quality Preset</span>
          <span className="form-hint">
            Faster = larger files, slower = smaller files
          </span>
        </div>
        <div className="form-input">
          <Select<TranscodingResourceLimits['preset']>
            value={localLimits.preset}
            options={[
              { value: 'ultrafast', label: 'Ultrafast (largest files)' },
              { value: 'superfast', label: 'Superfast' },
              { value: 'veryfast', label: 'Very Fast' },
              { value: 'faster', label: 'Faster' },
              { value: 'fast', label: 'Fast (recommended)' },
              { value: 'medium', label: 'Medium' },
              { value: 'slow', label: 'Slow' },
              { value: 'slower', label: 'Slower' },
              { value: 'veryslow', label: 'Very Slow (smallest files)' },
            ]}
            onChange={(value) => updateLimit('preset', value)}
            fullWidth
          />
        </div>
      </div>

      {/* Memory Limit */}
      <div className="form-row">
        <div className="form-label">
          <span>Memory Limit</span>
          <span className="form-hint">0 = unlimited</span>
        </div>
        <div className="form-input compact">
          <input
            type="number"
            min="0"
            max="32768"
            step="512"
            value={localLimits.memoryLimitMB}
            onChange={(e) =>
              updateLimit('memoryLimitMB', parseInt(e.target.value) || 0)
            }
          />
          <span className="input-suffix">
            {localLimits.memoryLimitMB === 0 ? 'Unlimited' : 'MB'}
          </span>
        </div>
      </div>

      {/* Concurrent Jobs */}
      <div className="form-row">
        <div className="form-label">
          <span>Concurrent Jobs</span>
          <span className="form-hint">
            How many videos to process simultaneously
          </span>
        </div>
        <div className="form-input compact">
          <input
            type="number"
            min="1"
            max="8"
            value={localLimits.maxConcurrentJobs}
            onChange={(e) =>
              updateLimit('maxConcurrentJobs', parseInt(e.target.value) || 1)
            }
          />
          <span className="input-suffix">jobs</span>
        </div>
      </div>

      {/* Save Button */}
      <div className="form-actions">
        <button
          className={`save-button ${hasChanges ? 'has-changes' : ''}`}
          onClick={handleSave}
          disabled={!hasChanges}
        >
          <span>{hasChanges ? 'Save Changes' : 'Saved'}</span>
          {hasChanges && (
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <polyline points="20 6 9 17 4 12" />
            </svg>
          )}
        </button>
      </div>
    </div>
  );
}

// ============================================================================
// Auto-Tagging Settings Component
// ============================================================================

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
  const [hasChanges, setHasChanges] = useState(false);

  useEffect(() => {
    setLocalLimits(limits);
    setHasChanges(false);
  }, [limits]);

  const updateLimit = <K extends keyof AutoTaggingResourceLimits>(
    key: K,
    value: AutoTaggingResourceLimits[K],
  ) => {
    setLocalLimits((prev) => ({ ...prev, [key]: value }));
    setHasChanges(true);
  };

  const handleSave = () => {
    onSave(localLimits);
    setHasChanges(false);
  };

  return (
    <div className="settings-form">
      {/* GPU Memory */}
      <div className="form-row">
        <div className="form-label">
          <span>GPU Memory Limit</span>
          <span className="form-hint">
            0 = unlimited
            {gpus.length > 0 &&
              ` (${Math.round(gpus[0].memory_total_mb / 1024)}GB available)`}
          </span>
        </div>
        <div className="form-input compact">
          <input
            type="number"
            min="0"
            max={
              gpus.length > 0
                ? Math.max(...gpus.map((g) => g.memory_total_mb / 1024))
                : 100
            }
            step="0.5"
            value={localLimits.gpuMemoryLimitGB}
            onChange={(e) =>
              updateLimit('gpuMemoryLimitGB', parseFloat(e.target.value) || 0)
            }
          />
          <span className="input-suffix">
            {localLimits.gpuMemoryLimitGB === 0 ? 'Unlimited' : 'GB'}
          </span>
        </div>
      </div>

      {/* CPU Cores */}
      <div className="form-row">
        <div className="form-label">
          <span>CPU Cores Limit</span>
          <span className="form-hint">
            0 = unlimited ({systemInfo?.cpu_cores || 'all'} available)
          </span>
        </div>
        <div className="form-input compact">
          <input
            type="number"
            min="0"
            max={systemInfo?.cpu_cores || 32}
            value={localLimits.cpuCoresLimit}
            onChange={(e) =>
              updateLimit('cpuCoresLimit', parseInt(e.target.value) || 0)
            }
          />
          <span className="input-suffix">
            {localLimits.cpuCoresLimit === 0 ? 'Unlimited' : 'cores'}
          </span>
        </div>
      </div>

      {/* System Memory */}
      <div className="form-row">
        <div className="form-label">
          <span>System Memory Limit</span>
          <span className="form-hint">0 = unlimited</span>
        </div>
        <div className="form-input compact">
          <input
            type="number"
            min="0"
            max="128"
            value={localLimits.systemMemoryLimitGB}
            onChange={(e) =>
              updateLimit('systemMemoryLimitGB', parseFloat(e.target.value) || 0)
            }
          />
          <span className="input-suffix">
            {localLimits.systemMemoryLimitGB === 0 ? 'Unlimited' : 'GB'}
          </span>
        </div>
      </div>

      {/* GPU Utilization */}
      <div className="form-row">
        <div className="form-label">
          <span>GPU Utilization Limit</span>
          <span className="form-hint">0 = unlimited, recommended: 50-70%</span>
        </div>
        <div className="form-input slider-input">
          <div className="slider-value">
            {localLimits.gpuUtilizationPercent === 0
              ? 'Unlimited'
              : `${localLimits.gpuUtilizationPercent}%`}
          </div>
          <input
            type="range"
            min="0"
            max="100"
            step="5"
            value={localLimits.gpuUtilizationPercent}
            onChange={(e) =>
              updateLimit('gpuUtilizationPercent', parseInt(e.target.value))
            }
            className="styled-slider"
          />
        </div>
      </div>

      {/* Number of GPUs */}
      {gpus.length > 1 && (
        <div className="form-row">
          <div className="form-label">
            <span>Number of GPUs</span>
            <span className="form-hint">
              0 = use all ({gpus.length} available)
            </span>
          </div>
          <div className="form-input compact">
            <input
              type="number"
              min="0"
              max={gpus.length}
              value={localLimits.numGpu}
              onChange={(e) =>
                updateLimit('numGpu', parseInt(e.target.value) || 0)
              }
            />
            <span className="input-suffix">
              {localLimits.numGpu === 0 ? 'All' : 'GPUs'}
            </span>
          </div>
        </div>
      )}

      {/* Save Button */}
      <div className="form-actions">
        <button
          className={`save-button ${hasChanges ? 'has-changes' : ''}`}
          onClick={handleSave}
          disabled={!hasChanges}
        >
          <span>{hasChanges ? 'Save Changes' : 'Saved'}</span>
          {hasChanges && (
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <polyline points="20 6 9 17 4 12" />
            </svg>
          )}
        </button>
      </div>
    </div>
  );
}

export default AIAdvancedSettings;
