/**
 * AITagSettings Component
 *
 * Settings for AI-powered tag suggestions for videos
 * - Enable/disable AI tag suggestions
 * - Choose execution mode: background or on-demand
 * - Configure which models to use
 * - View pending tag approvals
 * - Check and guide installation of dependencies (Docker, Ollama, etc.)
 */
import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './AITagSettings.css';

interface AITagConfig {
  enabled: boolean;
  mode: 'background' | 'on-demand';
  model: string;
  autoApprove: boolean;
  onlyMountedStorage: boolean;
}

interface SystemInfo {
  os_name: string;
  os_version: string;
}

type DependencyStatus =
  | 'checking'
  | 'installed'
  | 'not-installed'
  | 'not-running';

interface DependencyCheck {
  name: string;
  status: DependencyStatus;
  instructions: {
    title: string;
    steps: string[];
    downloadUrl: string;
    command?: string;
  };
}

const DEFAULT_CONFIG: AITagConfig = {
  enabled: false,
  mode: 'on-demand',
  model: 'llama3.2',
  autoApprove: false,
  onlyMountedStorage: true,
};

const STORAGE_KEY = 'ai_tag_suggestions_config';

export const AITagSettings: React.FC = () => {
  const [config, setConfig] = useState<AITagConfig>(DEFAULT_CONFIG);
  const [pendingCount, setPendingCount] = useState(0);
  const [isOllamaAvailable, setIsOllamaAvailable] = useState<boolean | null>(
    null,
  );
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [platform, setPlatform] = useState<
    'macos' | 'windows' | 'linux' | 'unknown'
  >('unknown');
  const [dependencies, setDependencies] = useState<DependencyCheck[]>([]);

  // Detect platform
  const detectPlatform = useCallback(async () => {
    try {
      const info = await invoke<SystemInfo>('get_system_info');
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

  // Get installation instructions for a dependency
  const getDependencyInstructions = (
    name: string,
  ): {
    title: string;
    steps: string[];
    downloadUrl: string;
    command?: string;
  } => {
    switch (name) {
      case 'docker':
        switch (platform) {
          case 'macos':
            return {
              title: 'Install Docker Desktop on macOS',
              steps: [
                'Download Docker Desktop from docker.com',
                'Open the downloaded .dmg file',
                'Drag Docker to your Applications folder',
                'Open Docker from Applications and complete setup',
                'Alternatively, install via Homebrew:',
              ],
              downloadUrl: 'https://www.docker.com/products/docker-desktop/',
              command: 'brew install --cask docker',
            };
          case 'windows':
            return {
              title: 'Install Docker Desktop on Windows',
              steps: [
                'Download Docker Desktop from docker.com',
                'Run the installer (.exe file)',
                'Follow the installation wizard',
                'Restart your computer if prompted',
                'Launch Docker Desktop from Start menu',
                'Alternatively, install via winget:',
              ],
              downloadUrl: 'https://www.docker.com/products/docker-desktop/',
              command: 'winget install Docker.DockerDesktop',
            };
          case 'linux':
            return {
              title: 'Install Docker on Linux',
              steps: [
                'Run the installation script in your terminal:',
                'After installation, start the Docker service:',
                'Enable Docker to start on boot (optional):',
                'Verify Docker is running:',
                'Add your user to the docker group (optional, to run without sudo):',
              ],
              downloadUrl: 'https://docs.docker.com/engine/install/',
              command:
                'curl -fsSL https://get.docker.com -o get-docker.sh && sh get-docker.sh',
            };
          default:
            return {
              title: 'Install Docker',
              steps: [
                'Visit docker.com to download Docker Desktop for your operating system',
                'Follow the installation instructions for your platform',
                'Start Docker Desktop after installation',
              ],
              downloadUrl: 'https://www.docker.com/products/docker-desktop/',
            };
        }
      case 'ollama':
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
      case 'ffmpeg':
        switch (platform) {
          case 'macos':
            return {
              title: 'Install FFmpeg on macOS',
              steps: [
                'Install via Homebrew (recommended):',
                'Or download from ffmpeg.org',
              ],
              downloadUrl: 'https://ffmpeg.org/download.html',
              command: 'brew install ffmpeg',
            };
          case 'windows':
            return {
              title: 'Install FFmpeg on Windows',
              steps: [
                'Download FFmpeg from ffmpeg.org',
                'Extract to a folder (e.g., C:\\ffmpeg)',
                'Add FFmpeg to your system PATH',
                'Alternatively, install via winget:',
              ],
              downloadUrl: 'https://ffmpeg.org/download.html',
              command: 'winget install Gyan.FFmpeg',
            };
          case 'linux':
            return {
              title: 'Install FFmpeg on Linux',
              steps: [
                'Install via package manager:',
                'Ubuntu/Debian: sudo apt-get install ffmpeg',
                'Fedora/RHEL: sudo dnf install ffmpeg',
                'Arch Linux: sudo pacman -S ffmpeg',
              ],
              downloadUrl: 'https://ffmpeg.org/download.html',
              command: 'sudo apt-get install ffmpeg',
            };
          default:
            return {
              title: 'Install FFmpeg',
              steps: [
                'Visit ffmpeg.org to download for your operating system',
                'Follow installation instructions for your platform',
              ],
              downloadUrl: 'https://ffmpeg.org/download.html',
            };
        }
      default:
        return {
          title: `Install ${name}`,
          steps: [`Please install ${name} to continue.`],
          downloadUrl: '#',
        };
    }
  };

  // Check Docker availability
  const checkDocker = useCallback(async (): Promise<DependencyStatus> => {
    try {
      // Try backend commands first (if implemented)
      try {
        const isInstalled = await invoke<boolean>('check_docker_installed');
        if (!isInstalled) {
          return 'not-installed';
        }
        try {
          const isRunning = await invoke<boolean>('check_docker_running');
          return isRunning ? 'installed' : 'not-running';
        } catch {
          // If check_docker_running doesn't exist, assume running if installed
          return 'installed';
        }
      } catch (err) {
        // Backend commands not implemented yet, return checking state
        // In production, these commands should be implemented
        return 'not-installed';
      }
    } catch (err) {
      return 'not-installed';
    }
  }, []);

  // Check Ollama availability
  const checkOllama = useCallback(async (): Promise<DependencyStatus> => {
    try {
      // First check if Ollama is running
      const isRunning = await invoke<boolean>('check_ollama_running');
      if (!isRunning) {
        setIsOllamaAvailable(false);
        return 'not-running';
      }

      // Then try to list models
      const result = await invoke<{ models: Array<{ name: string }> }>(
        'ollama_list',
      );
      setIsOllamaAvailable(true);
      setAvailableModels(result.models.map((m) => m.name));
      return 'installed';
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      if (
        errorMsg.includes('not found') ||
        errorMsg.includes('not installed') ||
        errorMsg.includes('Failed to connect')
      ) {
        setIsOllamaAvailable(false);
        // Check if it's installed but not running
        try {
          const isInstalled = await invoke<boolean>('check_ollama_installed');
          return isInstalled ? 'not-running' : 'not-installed';
        } catch {
          return 'not-installed';
        }
      } else if (
        errorMsg.includes('connection refused') ||
        errorMsg.includes('not running')
      ) {
        setIsOllamaAvailable(false);
        return 'not-running';
      }
      setIsOllamaAvailable(false);
      return 'not-installed';
    }
  }, []);

  // Check FFmpeg availability
  const checkFFmpeg = useCallback(async (): Promise<DependencyStatus> => {
    try {
      const isAvailable = await invoke<boolean>('check_ffmpeg_installed');
      return isAvailable ? 'installed' : 'not-installed';
    } catch (err) {
      // Command might not exist, assume not installed
      return 'not-installed';
    }
  }, []);

  // Check all dependencies
  const checkAllDependencies = useCallback(async () => {
    setDependencies([
      {
        name: 'docker',
        status: 'checking',
        instructions: getDependencyInstructions('docker'),
      },
      {
        name: 'ollama',
        status: 'checking',
        instructions: getDependencyInstructions('ollama'),
      },
      {
        name: 'ffmpeg',
        status: 'checking',
        instructions: getDependencyInstructions('ffmpeg'),
      },
    ]);

    // Check Docker (optional, for some advanced features)
    const dockerStatus = await checkDocker();
    setDependencies((prev) =>
      prev.map((d) =>
        d.name === 'docker' ? { ...d, status: dockerStatus } : d,
      ),
    );

    // Check Ollama (can run standalone, doesn't require Docker)
    const ollamaStatus = await checkOllama();
    setDependencies((prev) =>
      prev.map((d) =>
        d.name === 'ollama' ? { ...d, status: ollamaStatus } : d,
      ),
    );

    // Check FFmpeg (optional, for video processing)
    const ffmpegStatus = await checkFFmpeg();
    setDependencies((prev) =>
      prev.map((d) =>
        d.name === 'ffmpeg' ? { ...d, status: ffmpegStatus } : d,
      ),
    );
  }, [platform, checkDocker, checkOllama, checkFFmpeg]);

  // Load config from localStorage
  useEffect(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved) as AITagConfig;
        setConfig({ ...DEFAULT_CONFIG, ...parsed });
      }
    } catch (err) {
      console.error('Failed to load AI tag config:', err);
    }
  }, []);

  // Detect platform and check dependencies
  useEffect(() => {
    detectPlatform();
  }, [detectPlatform]);

  useEffect(() => {
    if (platform !== 'unknown') {
      checkAllDependencies();
    }
  }, [platform, checkAllDependencies]);

  // Load pending tag suggestions count
  useEffect(() => {
    const loadPendingCount = async () => {
      try {
        const pending = await invoke<number>(
          'get_pending_tag_suggestions_count',
        );
        setPendingCount(pending);
      } catch (err) {
        // Command might not exist yet, ignore
        setPendingCount(0);
      }
    };
    if (config.enabled) {
      loadPendingCount();
      const interval = setInterval(loadPendingCount, 5000);
      return () => clearInterval(interval);
    }
  }, [config.enabled]);

  // Save config to localStorage
  const saveConfig = (newConfig: AITagConfig) => {
    setConfig(newConfig);
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(newConfig));
    } catch (err) {
      console.error('Failed to save AI tag config:', err);
    }
  };

  // Open tag approval dialog
  const openApprovalDialog = () => {
    window.dispatchEvent(
      new CustomEvent('open-tag-approval', { detail: { count: pendingCount } }),
    );
  };

  return (
    <div className="ai-tag-settings">
      {/* Header */}
      <div className="ai-tag-header">
        <div className="ai-tag-status">
          <span
            className={`status-indicator ${config.enabled ? 'enabled' : 'disabled'}`}
          />
          <span>
            {config.enabled ? 'AI Tag Suggestions Enabled' : 'Disabled'}
          </span>
        </div>
        {pendingCount > 0 && (
          <button className="pending-badge" onClick={openApprovalDialog}>
            {pendingCount} Pending
          </button>
        )}
      </div>

      {/* Dependencies Check */}
      {dependencies.length > 0 && (
        <div className="ai-tag-dependencies">
          <h4 className="dependencies-title">Required Dependencies</h4>
          {dependencies.map((dep, index) => {
            const isFirstMissing =
              dep.status !== 'installed' &&
              dependencies
                .slice(0, index)
                .every((d) => d.status === 'installed');
            const showInstructions =
              dep.status !== 'installed' && isFirstMissing;

            return (
              <div
                key={dep.name}
                className={`dependency-item ${dep.status === 'installed' ? 'installed' : dep.status === 'not-running' ? 'not-running' : 'missing'}`}
              >
                <div className="dependency-header">
                  <div className="dependency-status">
                    {dep.status === 'checking' && (
                      <div className="status-spinner" />
                    )}
                    {dep.status === 'installed' && (
                      <svg
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                      >
                        <polyline points="20 6 9 17 4 12" />
                      </svg>
                    )}
                    {(dep.status === 'not-installed' ||
                      dep.status === 'not-running') && (
                      <svg
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                      >
                        <circle cx="12" cy="12" r="10" />
                        <line x1="12" y1="8" x2="12" y2="12" />
                        <line x1="12" y1="16" x2="12.01" y2="16" />
                      </svg>
                    )}
                    <span className="dependency-name">
                      {dep.name === 'docker'
                        ? 'Docker'
                        : dep.name === 'ollama'
                          ? 'Ollama'
                          : dep.name === 'ffmpeg'
                            ? 'FFmpeg'
                            : dep.name}
                    </span>
                  </div>
                  <span className="dependency-badge">
                    {dep.status === 'installed'
                      ? 'Installed'
                      : dep.status === 'not-running'
                        ? 'Not Running'
                        : dep.status === 'checking'
                          ? 'Checking...'
                          : dep.name === 'ollama'
                            ? 'Required'
                            : 'Optional'}
                  </span>
                </div>

                {showInstructions && (
                  <div className="dependency-instructions">
                    <h5>{dep.instructions.title}</h5>
                    <ol className="install-steps">
                      {dep.instructions.steps.map((step, stepIndex) => (
                        <li key={stepIndex}>{step}</li>
                      ))}
                    </ol>
                    {dep.instructions.command && (
                      <div className="install-command">
                        <div className="command-label">Terminal Command:</div>
                        <code className="command-code">
                          {dep.instructions.command}
                        </code>
                        <button
                          className="command-copy-btn"
                          onClick={async () => {
                            await navigator.clipboard.writeText(
                              dep.instructions.command || '',
                            );
                          }}
                          title="Copy command"
                        >
                          <svg
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            strokeWidth="2"
                          >
                            <rect
                              x="9"
                              y="9"
                              width="13"
                              height="13"
                              rx="2"
                              ry="2"
                            />
                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                          </svg>
                        </button>
                      </div>
                    )}
                    <a
                      href={dep.instructions.downloadUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="dependency-download-btn"
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
                      Download{' '}
                      {dep.name === 'docker'
                        ? 'Docker'
                        : dep.name === 'ollama'
                          ? 'Ollama'
                          : dep.name === 'ffmpeg'
                            ? 'FFmpeg'
                            : dep.name}
                    </a>
                    {dep.status === 'not-running' && (
                      <button
                        className="dependency-start-btn"
                        onClick={async () => {
                          if (dep.name === 'docker') {
                            // Try to start Docker
                            try {
                              await invoke('start_docker');
                              setTimeout(checkAllDependencies, 2000);
                            } catch (err) {
                              console.error('Failed to start Docker:', err);
                            }
                          } else if (dep.name === 'ollama') {
                            // Try to start Ollama
                            try {
                              await invoke('ollama_serve');
                              setTimeout(checkAllDependencies, 2000);
                            } catch (err) {
                              console.error('Failed to start Ollama:', err);
                            }
                          }
                        }}
                      >
                        Start {dep.name === 'docker' ? 'Docker' : 'Ollama'}
                      </button>
                    )}
                    <button
                      className="dependency-refresh-btn"
                      onClick={checkAllDependencies}
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
                      Refresh Status
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Enable/Disable Toggle */}
      <div className="ai-tag-setting">
        <div className="setting-label">
          <label htmlFor="ai-tag-enabled">Enable AI Tag Suggestions</label>
          <span className="setting-description">
            Automatically suggest tags for videos using local AI models
          </span>
        </div>
        <div className="setting-control">
          <label className="toggle-switch">
            <input
              type="checkbox"
              id="ai-tag-enabled"
              checked={config.enabled}
              onChange={(e) =>
                saveConfig({ ...config, enabled: e.target.checked })
              }
              disabled={
                dependencies.some(
                  (d) =>
                    d.name !== 'ffmpeg' && // FFmpeg is optional
                    (d.status === 'not-installed' ||
                      d.status === 'not-running' ||
                      d.status === 'checking'),
                ) || isOllamaAvailable === false
              }
            />
            <span className="toggle-slider" />
          </label>
        </div>
      </div>

      {/* Execution Mode */}
      {config.enabled && (
        <>
          <div className="ai-tag-setting">
            <div className="setting-label">
              <label htmlFor="ai-tag-mode">Execution Mode</label>
              <span className="setting-description">
                Choose when to generate tag suggestions
              </span>
            </div>
            <div className="setting-control">
              <select
                id="ai-tag-mode"
                value={config.mode}
                onChange={(e) =>
                  saveConfig({
                    ...config,
                    mode: e.target.value as 'background' | 'on-demand',
                  })
                }
              >
                <option value="on-demand">On-Demand</option>
                <option value="background">Background Task</option>
              </select>
            </div>
          </div>

          {/* Model Selection */}
          {availableModels.length > 0 && (
            <div className="ai-tag-setting">
              <div className="setting-label">
                <label htmlFor="ai-tag-model">AI Model</label>
                <span className="setting-description">
                  Select the model to use for tag generation
                </span>
              </div>
              <div className="setting-control">
                <select
                  id="ai-tag-model"
                  value={config.model}
                  onChange={(e) =>
                    saveConfig({ ...config, model: e.target.value })
                  }
                >
                  {availableModels.map((model) => (
                    <option key={model} value={model}>
                      {model}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          )}

          {/* Storage Filter */}
          <div className="ai-tag-setting">
            <div className="setting-label">
              <label htmlFor="ai-tag-storage">
                Only Process Mounted Storage
              </label>
              <span className="setting-description">
                Only suggest tags for videos on local or network-mounted storage
                (not cloud storage)
              </span>
            </div>
            <div className="setting-control">
              <label className="toggle-switch">
                <input
                  type="checkbox"
                  id="ai-tag-storage"
                  checked={config.onlyMountedStorage}
                  onChange={(e) =>
                    saveConfig({
                      ...config,
                      onlyMountedStorage: e.target.checked,
                    })
                  }
                />
                <span className="toggle-slider" />
              </label>
            </div>
          </div>

          {/* Auto-Approve */}
          <div className="ai-tag-setting">
            <div className="setting-label">
              <label htmlFor="ai-tag-auto-approve">Auto-Approve Tags</label>
              <span className="setting-description">
                Automatically approve suggested tags without manual review (not
                recommended)
              </span>
            </div>
            <div className="setting-control">
              <label className="toggle-switch">
                <input
                  type="checkbox"
                  id="ai-tag-auto-approve"
                  checked={config.autoApprove}
                  onChange={(e) =>
                    saveConfig({ ...config, autoApprove: e.target.checked })
                  }
                />
                <span className="toggle-slider" />
              </label>
            </div>
          </div>

          {/* Info Box */}
          <div className="ai-tag-info">
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <circle cx="12" cy="12" r="10" />
              <line x1="12" y1="16" x2="12" y2="12" />
              <line x1="12" y1="8" x2="12.01" y2="8" />
            </svg>
            <div>
              <strong>How it works:</strong>
              <ul>
                <li>
                  AI analyzes video files and suggests relevant tags based on
                  content
                </li>
                <li>
                  Suggested tags require approval before being applied (unless
                  auto-approve is enabled)
                </li>
                <li>
                  Only videos from mounted storage (local/network) are processed
                </li>
                <li>
                  Background mode processes videos automatically as they're
                  added
                </li>
              </ul>
            </div>
          </div>
        </>
      )}
    </div>
  );
};

export default AITagSettings;
