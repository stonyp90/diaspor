/**
 * OllamaManager Component
 *
 * Provides a friendly UI for managing Ollama models:
 * - Pull/download models from Ollama registry
 * - List available models
 * - Show currently running/serving models
 * - Start/stop serving models
 */
import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './OllamaManager.css';

interface OllamaModel {
  name: string;
  size: number;
  modified_at: string;
  digest: string;
}

interface RunningModel {
  name: string;
  model: string;
  size: number;
  expires_at?: string;
}

interface PullProgress {
  status: string;
  digest?: string;
  total?: number;
  completed?: number;
}

interface SystemInfo {
  os_name: string;
  os_version: string;
  kernel_version: string;
  hostname: string;
  cpu_brand: string;
  cpu_cores: number;
}

interface GpuInfo {
  id: number;
  name: string;
  vendor: string;
  memory_total_mb: number;
}

interface ResourceLimits {
  gpuMemoryLimitGB: number; // Max GPU memory in GB (0 = unlimited)
  cpuCoresLimit: number; // Max CPU cores (0 = unlimited)
  systemMemoryLimitGB: number; // Max system RAM in GB (0 = unlimited)
  gpuUtilizationPercent: number; // Max GPU utilization % (0-100, 0 = unlimited)
  numGpu: number; // Number of GPUs to use (0 = all available)
}

type Platform = 'macos' | 'windows' | 'linux' | 'unknown';

const STORAGE_KEY_RESOURCE_LIMITS = 'ollama_resource_limits';

const DEFAULT_RESOURCE_LIMITS: ResourceLimits = {
  gpuMemoryLimitGB: 0, // Unlimited by default
  cpuCoresLimit: 0, // Unlimited by default
  systemMemoryLimitGB: 0, // Unlimited by default
  gpuUtilizationPercent: 0, // Unlimited by default
  numGpu: 0, // Use all GPUs by default
};

export const OllamaManager: React.FC = () => {
  const [isOllamaInstalled, setIsOllamaInstalled] = useState<boolean | null>(
    null,
  );
  const [isOllamaRunning, setIsOllamaRunning] = useState<boolean>(false);
  const [models, setModels] = useState<OllamaModel[]>([]);
  const [runningModels, setRunningModels] = useState<RunningModel[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [platform, setPlatform] = useState<Platform>('unknown');
  const [copiedCommand, setCopiedCommand] = useState(false);
  const [resourceLimits, setResourceLimits] = useState<ResourceLimits>(
    DEFAULT_RESOURCE_LIMITS,
  );
  const [gpus, setGpus] = useState<GpuInfo[]>([]);
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [showResourceSettings, setShowResourceSettings] = useState(false);

  // Pull model state
  const [pullModelName, setPullModelName] = useState('');
  const [isPulling, setIsPulling] = useState(false);
  const [pullProgress, setPullProgress] = useState<PullProgress | null>(null);

  // Install all dependencies state
  const [isInstallingAll, setIsInstallingAll] = useState(false);
  const [installProgress, setInstallProgress] = useState<{
    current: string;
    completed: string[];
    remaining: string[];
  } | null>(null);

  // Required models for full functionality
  const requiredModels = [
    {
      name: 'whisper',
      description:
        'OpenAI Whisper - Audio transcription (required for transcription feature)',
    },
  ];

  // Popular models suggestions
  const popularModels = [
    { name: 'llama3.2', description: 'Meta Llama 3.2 - Fast & capable' },
    { name: 'mistral', description: 'Mistral 7B - Balanced performance' },
    { name: 'codellama', description: 'Code Llama - Code generation' },
    { name: 'whisper', description: 'OpenAI Whisper - Audio transcription' },
    { name: 'phi3', description: 'Microsoft Phi-3 - Compact & efficient' },
    { name: 'gemma2', description: 'Google Gemma 2 - Quality reasoning' },
  ];

  // Check if Ollama is installed and running
  const checkOllamaStatus = useCallback(async () => {
    try {
      if (typeof invoke === 'undefined') {
        setError('Tauri API not available');
        return;
      }
      // Try to list models - if it works, Ollama is running
      const result = await invoke<{ models: OllamaModel[] }>('ollama_list');
      setIsOllamaInstalled(true);
      setIsOllamaRunning(true);
      setModels(result.models || []);
      setError(null);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      if (
        errorMsg.includes('not found') ||
        errorMsg.includes('not installed') ||
        errorMsg.includes('Cannot read properties')
      ) {
        setIsOllamaInstalled(false);
        setIsOllamaRunning(false);
        setError(null); // Don't show error for missing Ollama
      } else if (
        errorMsg.includes('connection refused') ||
        errorMsg.includes('not running')
      ) {
        setIsOllamaInstalled(true);
        setIsOllamaRunning(false);
        setError(null);
      } else {
        setError(errorMsg);
      }
    }
  }, []);

  // Get running models
  const getRunningModels = useCallback(async () => {
    if (!isOllamaRunning) return;
    try {
      const result = await invoke<{ models: RunningModel[] }>('ollama_ps');
      setRunningModels(result.models || []);
    } catch (err) {
      console.error('Failed to get running models:', err);
    }
  }, [isOllamaRunning]);

  // Check which required models are missing
  const getMissingRequiredModels = useCallback((): string[] => {
    const installedModelNames = models.map((m) => m.name.toLowerCase());
    return requiredModels
      .filter((req) => !installedModelNames.includes(req.name.toLowerCase()))
      .map((req) => req.name);
  }, [models]);

  // Install all required dependencies
  const installAllRequired = async () => {
    const missing = getMissingRequiredModels();

    if (missing.length === 0) {
      setError('All required models are already installed!');
      return;
    }

    setIsInstallingAll(true);
    setError(null);
    setInstallProgress({
      current: missing[0],
      completed: [],
      remaining: missing.slice(1),
    });

    try {
      for (let i = 0; i < missing.length; i++) {
        const modelName = missing[i];

        setInstallProgress({
          current: modelName,
          completed: missing.slice(0, i),
          remaining: missing.slice(i + 1),
        });

        setPullProgress({ status: `Installing ${modelName}...` });

        try {
          await invoke('ollama_pull', { model: modelName });

          setInstallProgress((prev) => ({
            current: prev?.remaining[0] || '',
            completed: [...(prev?.completed || []), modelName],
            remaining: prev?.remaining.slice(1) || [],
          }));
        } catch (err) {
          const errorMsg = err instanceof Error ? err.message : String(err);
          setError(`Failed to install ${modelName}: ${errorMsg}`);
          setIsInstallingAll(false);
          setInstallProgress(null);
          setPullProgress(null);
          await checkOllamaStatus();
          return;
        }
      }

      // All installed successfully
      setPullProgress({ status: 'All dependencies installed successfully!' });
      await checkOllamaStatus();

      setTimeout(() => {
        setIsInstallingAll(false);
        setInstallProgress(null);
        setPullProgress(null);
      }, 2000);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(`Failed to install dependencies: ${errorMsg}`);
      setIsInstallingAll(false);
      setInstallProgress(null);
      setPullProgress(null);
    }
  };

  // Pull a model
  const pullModel = async (modelName: string) => {
    if (!modelName.trim()) return;
    setIsPulling(true);
    setPullProgress({ status: 'Starting download...' });
    setError(null);

    try {
      await invoke('ollama_pull', { model: modelName.trim() });
      setPullProgress({ status: 'Completed!' });
      setPullModelName('');
      // Refresh models list
      await checkOllamaStatus();
      setTimeout(() => {
        setIsPulling(false);
        setPullProgress(null);
      }, 2000);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(`Failed to pull model: ${errorMsg}`);
      setIsPulling(false);
      setPullProgress(null);
    }
  };

  // Delete a model
  const deleteModel = async (modelName: string) => {
    if (
      !confirm(
        `Are you sure you want to delete "${modelName}"? This cannot be undone.`,
      )
    ) {
      return;
    }
    setLoading(true);
    try {
      await invoke('ollama_delete', { model: modelName });
      await checkOllamaStatus();
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(`Failed to delete model: ${errorMsg}`);
    }
    setLoading(false);
  };

  // Start serving a model with resource limits
  const serveModel = async (modelName: string) => {
    setLoading(true);
    try {
      await invoke('ollama_run', {
        model: modelName,
        resourceLimits: resourceLimits,
      });
      await getRunningModels();
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(`Failed to start model: ${errorMsg}`);
    }
    setLoading(false);
  };

  // Stop serving a model
  const stopModel = async (modelName: string) => {
    setLoading(true);
    try {
      await invoke('ollama_stop', { model: modelName });
      await getRunningModels();
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(`Failed to stop model: ${errorMsg}`);
    }
    setLoading(false);
  };

  // Start Ollama service
  const startOllama = async () => {
    setLoading(true);
    try {
      await invoke('ollama_serve');
      // Wait a bit for the service to start
      setTimeout(async () => {
        await checkOllamaStatus();
        setLoading(false);
      }, 2000);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(`Failed to start Ollama: ${errorMsg}`);
      setLoading(false);
    }
  };

  // Load resource limits from localStorage
  useEffect(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY_RESOURCE_LIMITS);
      if (saved) {
        const parsed = JSON.parse(saved) as ResourceLimits;
        setResourceLimits({ ...DEFAULT_RESOURCE_LIMITS, ...parsed });
      }
    } catch (err) {
      console.error('Failed to load resource limits:', err);
    }
  }, []);

  // Save resource limits to localStorage
  const saveResourceLimits = (limits: ResourceLimits) => {
    setResourceLimits(limits);
    try {
      localStorage.setItem(STORAGE_KEY_RESOURCE_LIMITS, JSON.stringify(limits));
    } catch (err) {
      console.error('Failed to save resource limits:', err);
    }
  };

  // Load GPU info and system info
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
    if (isOllamaRunning) {
      loadSystemInfo();
    }
  }, [isOllamaRunning]);

  // Detect platform from system info
  const detectPlatform = useCallback(async () => {
    try {
      const info = await invoke<SystemInfo>('get_system_info');
      setSystemInfo(info);
      const osName = info.os_name.toLowerCase();
      if (osName.includes('macos') || osName.includes('darwin')) {
        setPlatform('macos');
      } else if (osName.includes('windows')) {
        setPlatform('windows');
      } else if (osName.includes('linux')) {
        setPlatform('linux');
      } else {
        setPlatform('unknown');
      }
    } catch (err) {
      console.error('Failed to detect platform:', err);
      setPlatform('unknown');
    }
  }, []);

  // Format file size
  const formatSize = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  };

  // Get OS-specific installation instructions
  const getInstallInstructions = (): {
    title: string;
    steps: string[];
    downloadUrl: string;
    command?: string;
  } => {
    switch (platform) {
      case 'macos':
        return {
          title: 'Install Ollama on macOS',
          steps: [
            'Download the macOS installer from ollama.ai',
            'Open the downloaded .dmg file',
            'Drag Ollama to your Applications folder',
            'Open Ollama from Applications (it will start automatically)',
            'Alternatively, install via Homebrew:',
          ],
          downloadUrl: 'https://ollama.ai/download/mac',
          command: 'brew install ollama',
        };
      case 'windows':
        return {
          title: 'Install Ollama on Windows',
          steps: [
            'Download the Windows installer from ollama.ai',
            'Run the installer (.exe file)',
            'Follow the installation wizard',
            'Ollama will start automatically after installation',
            'Alternatively, install via winget:',
          ],
          downloadUrl: 'https://ollama.ai/download/windows',
          command: 'winget install Ollama.Ollama',
        };
      case 'linux':
        return {
          title: 'Install Ollama on Linux',
          steps: [
            'Run the installation script in your terminal:',
            'The script will detect your Linux distribution automatically',
            'After installation, start the Ollama service:',
            'Verify Ollama is running:',
          ],
          downloadUrl: 'https://ollama.ai/download/linux',
          command: 'curl -fsSL https://ollama.ai/install.sh | sh',
        };
      default:
        return {
          title: 'Install Ollama',
          steps: [
            'Visit ollama.ai to download the installer for your operating system',
            'Follow the installation instructions for your platform',
            'After installation, Ollama will start automatically',
          ],
          downloadUrl: 'https://ollama.ai',
        };
    }
  };

  // Initial load
  useEffect(() => {
    detectPlatform();
    checkOllamaStatus();
  }, [detectPlatform, checkOllamaStatus]);

  // Poll for running models
  useEffect(() => {
    if (isOllamaRunning) {
      getRunningModels();
      const interval = setInterval(getRunningModels, 5000);
      return () => clearInterval(interval);
    }
  }, [isOllamaRunning, getRunningModels]);

  // Not installed state
  if (isOllamaInstalled === false) {
    const instructions = getInstallInstructions();
    const platformName =
      platform === 'macos'
        ? 'macOS'
        : platform === 'windows'
          ? 'Windows'
          : platform === 'linux'
            ? 'Linux'
            : 'your system';

    return (
      <div className="ollama-manager">
        <div className="ollama-not-installed">
          <div className="ollama-icon">
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <circle cx="12" cy="12" r="10" />
              <path d="M12 6v6l4 2" />
            </svg>
          </div>
          <h3>Ollama Not Installed</h3>
          <p className="install-description">
            Ollama is required to run local AI models for transcription and
            other features. Follow the instructions below to install Ollama on{' '}
            {platformName}.
          </p>

          <div className="install-instructions">
            <h4>{instructions.title}</h4>
            <ol className="install-steps">
              {instructions.steps.map((step, index) => (
                <li key={index}>{step}</li>
              ))}
            </ol>

            {instructions.command && (
              <div className="install-command">
                <div className="command-label">Terminal Command:</div>
                <code className="command-code">{instructions.command}</code>
                <button
                  className={`command-copy-btn ${copiedCommand ? 'copied' : ''}`}
                  onClick={async () => {
                    if (instructions.command) {
                      await navigator.clipboard.writeText(instructions.command);
                      setCopiedCommand(true);
                      setTimeout(() => setCopiedCommand(false), 2000);
                    }
                  }}
                  title={copiedCommand ? 'Copied!' : 'Copy command'}
                >
                  {copiedCommand ? (
                    <svg
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                    >
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  ) : (
                    <svg
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                    >
                      <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                    </svg>
                  )}
                </button>
              </div>
            )}

            {platform === 'linux' && (
              <div className="install-note">
                <strong>Note:</strong> After installation, you may need to start
                the Ollama service:
                <code className="command-code">
                  sudo systemctl start ollama
                </code>
                <br />
                To enable auto-start on boot:
                <code className="command-code">
                  sudo systemctl enable ollama
                </code>
              </div>
            )}
          </div>

          <div className="install-actions">
            <a
              href={instructions.downloadUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="ollama-install-link"
            >
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
              Download for {platformName}
            </a>
            <button className="ollama-refresh-btn" onClick={checkOllamaStatus}>
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" />
                <path d="M21 3v5h-5" />
                <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" />
                <path d="M3 21v-5h5" />
              </svg>
              Refresh Status
            </button>
          </div>
        </div>
      </div>
    );
  }

  // Not running state
  if (!isOllamaRunning && isOllamaInstalled) {
    return (
      <div className="ollama-manager">
        <div className="ollama-not-running">
          <div className="ollama-icon warning">
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
              <line x1="12" y1="9" x2="12" y2="13" />
              <line x1="12" y1="17" x2="12.01" y2="17" />
            </svg>
          </div>
          <h3>Ollama Not Running</h3>
          <p>Ollama is installed but not currently running.</p>
          <button
            className="ollama-start-btn"
            onClick={startOllama}
            disabled={loading}
          >
            {loading ? 'Starting...' : 'Start Ollama'}
          </button>
          <button className="ollama-refresh-btn" onClick={checkOllamaStatus}>
            Refresh Status
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="ollama-manager">
      {/* Header */}
      <div className="ollama-header">
        <div className="ollama-status">
          <span className="status-dot online" />
          <span>Ollama Running</span>
        </div>
        <button
          className="ollama-refresh-btn small"
          onClick={checkOllamaStatus}
          disabled={loading}
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" />
            <path d="M21 3v5h-5" />
            <path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" />
            <path d="M3 21v-5h5" />
          </svg>
        </button>
      </div>

      {/* Error display */}
      {error && (
        <div className="ollama-error">
          <span>{error}</span>
          <button onClick={() => setError(null)}>×</button>
        </div>
      )}

      {/* Install All Required Dependencies */}
      {getMissingRequiredModels().length > 0 && (
        <div className="ollama-section install-all-section">
          <div className="install-all-header">
            <div>
              <h4>Required Dependencies</h4>
              <p className="install-all-description">
                Install all required models for full functionality (
                {getMissingRequiredModels().length} missing)
              </p>
            </div>
            <button
              className="ollama-install-all-btn"
              onClick={installAllRequired}
              disabled={isInstallingAll || isPulling}
            >
              {isInstallingAll ? 'Installing...' : 'Install All Required'}
            </button>
          </div>

          {installProgress && (
            <div className="install-all-progress">
              <div className="progress-info">
                <span className="progress-current">
                  Installing: <strong>{installProgress.current}</strong>
                </span>
                {installProgress.completed.length > 0 && (
                  <span className="progress-completed">
                    Completed: {installProgress.completed.join(', ')}
                  </span>
                )}
                {installProgress.remaining.length > 0 && (
                  <span className="progress-remaining">
                    Remaining: {installProgress.remaining.length}
                  </span>
                )}
              </div>
              {pullProgress && (
                <div className="progress-bar">
                  <div
                    className="progress-fill"
                    style={{
                      width:
                        pullProgress.completed && pullProgress.total
                          ? `${(pullProgress.completed / pullProgress.total) * 100}%`
                          : '50%',
                    }}
                  />
                </div>
              )}
            </div>
          )}

          <div className="required-models-list">
            {requiredModels.map((model) => {
              const isInstalled = models.some(
                (m) => m.name.toLowerCase() === model.name.toLowerCase(),
              );
              return (
                <div
                  key={model.name}
                  className={`required-model-item ${isInstalled ? 'installed' : 'missing'}`}
                >
                  <span className="model-status-icon">
                    {isInstalled ? '✓' : '○'}
                  </span>
                  <span className="model-name">{model.name}</span>
                  <span className="model-description">{model.description}</span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Pull Model Section */}
      <div className="ollama-section">
        <h4>Download Model</h4>
        <div className="ollama-pull-form">
          <input
            type="text"
            value={pullModelName}
            onChange={(e) => setPullModelName(e.target.value)}
            placeholder="Model name (e.g., llama3.2, mistral)"
            disabled={isPulling}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !isPulling) {
                pullModel(pullModelName);
              }
            }}
          />
          <button
            onClick={() => pullModel(pullModelName)}
            disabled={isPulling || !pullModelName.trim()}
          >
            {isPulling ? 'Downloading...' : 'Download'}
          </button>
        </div>

        {/* Pull progress */}
        {pullProgress && (
          <div className="ollama-pull-progress">
            <span className="progress-status">{pullProgress.status}</span>
            {pullProgress.total && pullProgress.completed && (
              <div className="progress-bar">
                <div
                  className="progress-fill"
                  style={{
                    width: `${(pullProgress.completed / pullProgress.total) * 100}%`,
                  }}
                />
              </div>
            )}
          </div>
        )}

        {/* Popular models */}
        <div className="ollama-popular">
          <span className="popular-label">Popular:</span>
          <div className="popular-list">
            {popularModels.map((model) => (
              <button
                key={model.name}
                className="popular-model"
                onClick={() => setPullModelName(model.name)}
                title={model.description}
                disabled={isPulling}
              >
                {model.name}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Running Models Section */}
      {runningModels.length > 0 && (
        <div className="ollama-section">
          <h4>
            Running Models{' '}
            <span className="model-count">({runningModels.length})</span>
          </h4>
          <div className="ollama-model-list">
            {runningModels.map((model) => (
              <div key={model.name} className="ollama-model-item running">
                <div className="model-info">
                  <span className="model-name">
                    {model.model || model.name}
                  </span>
                  <span className="model-status">
                    <span className="status-dot online" /> Running
                  </span>
                </div>
                <div className="model-actions">
                  <button
                    className="model-action-btn stop"
                    onClick={() => stopModel(model.model || model.name)}
                    disabled={loading}
                    title="Stop serving this model"
                  >
                    Stop
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Resource Limits Section */}
      <div className="ollama-section">
        <div className="ollama-section-header">
          <h4>Resource Limits</h4>
          <button
            className="ollama-toggle-btn"
            onClick={() => setShowResourceSettings(!showResourceSettings)}
            title={showResourceSettings ? 'Hide settings' : 'Show settings'}
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              style={{
                transform: showResourceSettings
                  ? 'rotate(180deg)'
                  : 'rotate(0deg)',
                transition: 'transform 0.2s ease',
              }}
            >
              <path d="M6 9l6 6 6-6" />
            </svg>
          </button>
        </div>
        <p className="section-description">
          Limit GPU and CPU usage to ensure other applications have resources
          available.
        </p>

        {showResourceSettings && (
          <div className="ollama-resource-settings">
            {/* GPU Memory Limit */}
            <div className="resource-setting">
              <label htmlFor="gpu-memory-limit">
                <span className="setting-name">GPU Memory Limit</span>
                <span className="setting-desc">
                  Maximum GPU memory Ollama can use (GB). 0 = unlimited.
                  {gpus.length > 0 &&
                    ` Available: ${gpus.map((g) => `${(g.memory_total_mb / 1024).toFixed(1)}GB`).join(', ')}`}
                </span>
              </label>
              <div className="setting-input-group">
                <input
                  type="number"
                  id="gpu-memory-limit"
                  min="0"
                  max={
                    gpus.length > 0
                      ? Math.max(...gpus.map((g) => g.memory_total_mb / 1024))
                      : 100
                  }
                  step="0.5"
                  value={resourceLimits.gpuMemoryLimitGB || 0}
                  onChange={(e) =>
                    saveResourceLimits({
                      ...resourceLimits,
                      gpuMemoryLimitGB: parseFloat(e.target.value) || 0,
                    })
                  }
                />
                <span className="input-unit">GB</span>
              </div>
            </div>

            {/* CPU Cores Limit */}
            <div className="resource-setting">
              <label htmlFor="cpu-cores-limit">
                <span className="setting-name">CPU Cores Limit</span>
                <span className="setting-desc">
                  Maximum CPU cores Ollama can use. 0 = unlimited.
                  {systemInfo && ` Available: ${systemInfo.cpu_cores} cores`}
                </span>
              </label>
              <div className="setting-input-group">
                <input
                  type="number"
                  id="cpu-cores-limit"
                  min="0"
                  max={systemInfo?.cpu_cores || 32}
                  step="1"
                  value={resourceLimits.cpuCoresLimit || 0}
                  onChange={(e) =>
                    saveResourceLimits({
                      ...resourceLimits,
                      cpuCoresLimit: parseInt(e.target.value) || 0,
                    })
                  }
                />
                <span className="input-unit">cores</span>
              </div>
            </div>

            {/* System Memory Limit */}
            <div className="resource-setting">
              <label htmlFor="system-memory-limit">
                <span className="setting-name">System Memory Limit</span>
                <span className="setting-desc">
                  Maximum system RAM Ollama can use (GB). 0 = unlimited.
                </span>
              </label>
              <div className="setting-input-group">
                <input
                  type="number"
                  id="system-memory-limit"
                  min="0"
                  max="128"
                  step="1"
                  value={resourceLimits.systemMemoryLimitGB || 0}
                  onChange={(e) =>
                    saveResourceLimits({
                      ...resourceLimits,
                      systemMemoryLimitGB: parseFloat(e.target.value) || 0,
                    })
                  }
                />
                <span className="input-unit">GB</span>
              </div>
            </div>

            {/* GPU Utilization Limit */}
            <div className="resource-setting">
              <label htmlFor="gpu-utilization-limit">
                <span className="setting-name">GPU Utilization Limit</span>
                <span className="setting-desc">
                  Maximum GPU utilization percentage (0-100). 0 = unlimited.
                  Recommended: 50-70% to leave resources for other apps.
                </span>
              </label>
              <div className="setting-input-group">
                <input
                  type="number"
                  id="gpu-utilization-limit"
                  min="0"
                  max="100"
                  step="5"
                  value={resourceLimits.gpuUtilizationPercent || 0}
                  onChange={(e) =>
                    saveResourceLimits({
                      ...resourceLimits,
                      gpuUtilizationPercent: parseInt(e.target.value) || 0,
                    })
                  }
                />
                <span className="input-unit">%</span>
              </div>
              {resourceLimits.gpuUtilizationPercent > 0 && (
                <div className="slider-container">
                  <input
                    type="range"
                    min="0"
                    max="100"
                    step="5"
                    value={resourceLimits.gpuUtilizationPercent}
                    onChange={(e) =>
                      saveResourceLimits({
                        ...resourceLimits,
                        gpuUtilizationPercent: parseInt(e.target.value),
                      })
                    }
                    className="resource-slider"
                  />
                </div>
              )}
            </div>

            {/* Number of GPUs */}
            {gpus.length > 1 && (
              <div className="resource-setting">
                <label htmlFor="num-gpu">
                  <span className="setting-name">Number of GPUs</span>
                  <span className="setting-desc">
                    How many GPUs to use for Ollama. 0 = use all available GPUs.
                    Available: {gpus.length} GPU{gpus.length > 1 ? 's' : ''}
                  </span>
                </label>
                <div className="setting-input-group">
                  <input
                    type="number"
                    id="num-gpu"
                    min="0"
                    max={gpus.length}
                    step="1"
                    value={resourceLimits.numGpu || 0}
                    onChange={(e) =>
                      saveResourceLimits({
                        ...resourceLimits,
                        numGpu: parseInt(e.target.value) || 0,
                      })
                    }
                  />
                  <span className="input-unit">
                    GPU{gpus.length > 1 ? 's' : ''}
                  </span>
                </div>
              </div>
            )}

            {/* Resource Summary */}
            <div className="resource-summary">
              <div className="summary-item">
                <span className="summary-label">GPU Memory:</span>
                <span className="summary-value">
                  {resourceLimits.gpuMemoryLimitGB > 0
                    ? `${resourceLimits.gpuMemoryLimitGB} GB`
                    : 'Unlimited'}
                </span>
              </div>
              <div className="summary-item">
                <span className="summary-label">CPU Cores:</span>
                <span className="summary-value">
                  {resourceLimits.cpuCoresLimit > 0
                    ? `${resourceLimits.cpuCoresLimit} cores`
                    : 'Unlimited'}
                </span>
              </div>
              <div className="summary-item">
                <span className="summary-label">System RAM:</span>
                <span className="summary-value">
                  {resourceLimits.systemMemoryLimitGB > 0
                    ? `${resourceLimits.systemMemoryLimitGB} GB`
                    : 'Unlimited'}
                </span>
              </div>
              <div className="summary-item">
                <span className="summary-label">GPU Usage:</span>
                <span className="summary-value">
                  {resourceLimits.gpuUtilizationPercent > 0
                    ? `${resourceLimits.gpuUtilizationPercent}%`
                    : 'Unlimited'}
                </span>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Installed Models Section */}
      <div className="ollama-section">
        <h4>
          Installed Models{' '}
          <span className="model-count">({models.length})</span>
        </h4>
        {models.length === 0 ? (
          <div className="ollama-empty">
            <p>No models installed yet.</p>
            <p className="hint">Download a model above to get started.</p>
          </div>
        ) : (
          <div className="ollama-model-list">
            {models.map((model) => {
              const isRunning = runningModels.some(
                (r) => r.model === model.name || r.name === model.name,
              );
              return (
                <div
                  key={model.name}
                  className={`ollama-model-item ${isRunning ? 'running' : ''}`}
                >
                  <div className="model-info">
                    <span className="model-name">{model.name}</span>
                    <span className="model-size">{formatSize(model.size)}</span>
                  </div>
                  <div className="model-actions">
                    {!isRunning ? (
                      <button
                        className="model-action-btn serve"
                        onClick={() => serveModel(model.name)}
                        disabled={loading}
                        title="Start serving this model"
                      >
                        Serve
                      </button>
                    ) : (
                      <button
                        className="model-action-btn stop"
                        onClick={() => stopModel(model.name)}
                        disabled={loading}
                        title="Stop serving this model"
                      >
                        Stop
                      </button>
                    )}
                    <button
                      className="model-action-btn delete"
                      onClick={() => deleteModel(model.name)}
                      disabled={loading || isRunning}
                      title={
                        isRunning
                          ? 'Stop the model before deleting'
                          : 'Delete this model'
                      }
                    >
                      Delete
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};

export default OllamaManager;
