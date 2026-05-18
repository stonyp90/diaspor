/**
 * Docker & Ollama Setup Component
 *
 * Modern, cutting-edge installation experience with:
 * - Auto-detection and status polling
 * - One-click installation where possible
 * - Real-time status updates
 * - Beautiful animations and feedback
 * - Accessibility-first design
 * - Smart error handling and recovery
 */
import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useToast } from '../Toast';
import './DockerOllamaSetup.css';

type Platform = 'macos' | 'linux' | 'windows' | 'unknown';

interface InstallationStatus {
  docker: {
    installed: boolean;
    running: boolean;
    version?: string;
  };
  ollama: {
    installed: boolean;
    running: boolean;
    version?: string;
  };
}

interface DependencyInfo {
  name: string;
  description: string;
  icon: string;
  status: 'installed-running' | 'installed-stopped' | 'not-installed';
  installCommand: string;
  instructions: {
    title: string;
    steps: string[];
  };
  canAutoInstall: boolean;
  autoInstallAction?: () => Promise<void>;
}

export function DockerOllamaSetup() {
  const [platform, setPlatform] = useState<Platform>('unknown');
  const [status, setStatus] = useState<InstallationStatus | null>(null);
  const [copiedCommand, setCopiedCommand] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [autoRefreshing, setAutoRefreshing] = useState(true);
  const [installing, setInstalling] = useState<{
    docker?: boolean;
    ollama?: boolean;
  }>({});
  const toast = useToast();

  // Auto-refresh status every 3 seconds when auto-refresh is enabled
  useEffect(() => {
    if (!autoRefreshing) return;

    const interval = setInterval(() => {
      checkInstallationStatus(false); // Silent refresh
    }, 3000);

    return () => clearInterval(interval);
  }, [autoRefreshing]);

  useEffect(() => {
    detectPlatform();
    checkInstallationStatus();
  }, []);

  const detectPlatform = async () => {
    try {
      const detected = await invoke<Platform>('detect_platform');
      setPlatform(detected);
    } catch {
      // Fallback detection
      const userAgent = navigator.userAgent.toLowerCase();
      if (userAgent.includes('mac')) {
        setPlatform('macos');
      } else if (userAgent.includes('win')) {
        setPlatform('windows');
      } else {
        setPlatform('linux');
      }
    }
  };

  const checkInstallationStatus = async (showLoading = true) => {
    if (showLoading) setLoading(true);
    try {
      const [dockerInstalled, dockerRunning, ollamaInstalled, ollamaRunning] =
        await Promise.all([
          invoke<boolean>('check_docker_installed').catch(() => false),
          invoke<boolean>('check_docker_running').catch(() => false),
          invoke<boolean>('check_ollama_installed').catch(() => false),
          invoke<boolean>('check_ollama_running').catch(() => false),
        ]);

      setStatus({
        docker: {
          installed: dockerInstalled,
          running: dockerRunning,
        },
        ollama: {
          installed: ollamaInstalled,
          running: ollamaRunning,
        },
      });
    } catch (error) {
      console.error('Failed to check installation status:', error);
      setStatus({
        docker: { installed: false, running: false },
        ollama: { installed: false, running: false },
      });
    } finally {
      if (showLoading) setLoading(false);
    }
  };

  const copyToClipboard = async (text: string, commandId: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedCommand(commandId);
      toast.showToast({
        type: 'success',
        message: 'Command copied to clipboard',
        duration: 2000,
      });
      setTimeout(() => setCopiedCommand(null), 2000);
    } catch (error) {
      console.error('Failed to copy to clipboard:', error);
      toast.showToast({
        type: 'error',
        message: 'Failed to copy to clipboard',
        duration: 3000,
      });
    }
  };

  const handleAutoInstall = async (dependency: 'docker' | 'ollama') => {
    setInstalling((prev) => ({ ...prev, [dependency]: true }));

    try {
      const installFn =
        dependency === 'ollama' ? 'install_ollama' : 'install_docker';
      const result = await invoke<{
        success: boolean;
        message: string;
        requires_restart: boolean;
      }>(installFn);

      if (result.success) {
        toast.showToast({
          type: 'success',
          message: result.message,
          duration: 4000,
        });

        // Wait a bit then refresh status
        setTimeout(() => {
          checkInstallationStatus();
        }, 2000);
      } else {
        toast.showToast({
          type: 'warning',
          message: result.message,
          duration: 5000,
        });
      }
    } catch (error) {
      console.error(`Failed to install ${dependency}:`, error);
      toast.showToast({
        type: 'error',
        message: `Failed to install ${dependency}. Please install manually.`,
        duration: 5000,
      });
    } finally {
      setInstalling((prev) => ({ ...prev, [dependency]: false }));
    }
  };

  const getDockerInfo = (): DependencyInfo => {
    const dockerStatus = status?.docker;
    let statusType:
      | 'installed-running'
      | 'installed-stopped'
      | 'not-installed' = 'not-installed';

    if (dockerStatus?.installed && dockerStatus?.running) {
      statusType = 'installed-running';
    } else if (dockerStatus?.installed) {
      statusType = 'installed-stopped';
    }

    const installCommand = (() => {
      switch (platform) {
        case 'macos':
          return 'brew install --cask docker';
        case 'linux':
          return 'curl -fsSL https://get.docker.com -o get-docker.sh && sh get-docker.sh';
        case 'windows':
          return 'winget install Docker.DockerDesktop';
        default:
          return 'See installation instructions below';
      }
    })();

    const instructions = (() => {
      switch (platform) {
        case 'macos':
          return {
            title: 'macOS Installation',
            steps: [
              'Open Terminal (Cmd+Space, type "Terminal")',
              `Run: ${installCommand}`,
              'Or download Docker Desktop from docker.com/products/docker-desktop',
              'Launch Docker Desktop from Applications',
              'Wait for Docker to start (whale icon in menu bar)',
              'Docker will start automatically on login',
            ],
          };
        case 'linux':
          return {
            title: 'Linux Installation',
            steps: [
              'Open Terminal',
              `Run: ${installCommand}`,
              'Add your user to docker group: sudo usermod -aG docker $USER',
              'Log out and back in (or run: newgrp docker)',
              'Start Docker: sudo systemctl start docker',
              'Enable auto-start: sudo systemctl enable docker',
            ],
          };
        case 'windows':
          return {
            title: 'Windows Installation',
            steps: [
              'Download Docker Desktop from docker.com/products/docker-desktop',
              'Run the installer (DockerDesktopInstaller.exe)',
              'Restart your computer if prompted',
              'Launch Docker Desktop from Start menu',
              'Wait for Docker to start (whale icon in system tray)',
              'Docker will start automatically on boot',
            ],
          };
        default:
          return {
            title: 'Installation',
            steps: ['Visit docker.com/products/docker-desktop'],
          };
      }
    })();

    return {
      name: 'Docker',
      description: 'Container platform required for AI model management',
      icon: '🐳',
      status: statusType,
      installCommand,
      instructions,
      canAutoInstall:
        platform === 'macos' || platform === 'linux' || platform === 'windows',
      autoInstallAction: () => handleAutoInstall('docker'),
    };
  };

  const getOllamaInfo = (): DependencyInfo => {
    const ollamaStatus = status?.ollama;
    let statusType:
      | 'installed-running'
      | 'installed-stopped'
      | 'not-installed' = 'not-installed';

    if (ollamaStatus?.installed && ollamaStatus?.running) {
      statusType = 'installed-running';
    } else if (ollamaStatus?.installed) {
      statusType = 'installed-stopped';
    }

    const installCommand = (() => {
      switch (platform) {
        case 'macos':
          return 'brew install ollama';
        case 'linux':
          return 'curl -fsSL https://ollama.ai/install.sh | sh';
        case 'windows':
          return 'winget install Ollama.Ollama';
        default:
          return 'See installation instructions below';
      }
    })();

    const instructions = (() => {
      switch (platform) {
        case 'macos':
          return {
            title: 'macOS Installation',
            steps: [
              'Open Terminal (Cmd+Space, type "Terminal")',
              `Run: ${installCommand}`,
              'Start Ollama: ollama serve',
              'Ollama will start automatically on login',
              'Verify: ollama list (should show available models)',
            ],
          };
        case 'linux':
          return {
            title: 'Linux Installation',
            steps: [
              'Open Terminal',
              `Run: ${installCommand}`,
              'Start Ollama: ollama serve',
              'Or enable service: sudo systemctl enable ollama && sudo systemctl start ollama',
              'Verify: ollama list',
            ],
          };
        case 'windows':
          return {
            title: 'Windows Installation',
            steps: [
              'Download from ollama.ai/download',
              'Run the installer (OllamaSetup.exe)',
              'Ollama will start automatically',
              'Or start manually: ollama serve',
              'Verify: ollama list',
            ],
          };
        default:
          return {
            title: 'Installation',
            steps: ['Visit ollama.ai/download'],
          };
      }
    })();

    return {
      name: 'Ollama',
      description: 'Local AI model runtime for transcription and video tagging',
      icon: '🤖',
      status: statusType,
      installCommand,
      instructions,
      canAutoInstall:
        platform === 'macos' || platform === 'linux' || platform === 'windows',
      autoInstallAction: () => handleAutoInstall('ollama'),
    };
  };

  if (loading && !status) {
    return (
      <div className="docker-ollama-setup">
        <div className="loading-container">
          <div className="spinner" />
          <p>Detecting installation status...</p>
        </div>
      </div>
    );
  }

  const dockerInfo = getDockerInfo();
  const ollamaInfo = getOllamaInfo();

  return (
    <div className="docker-ollama-setup">
      <div className="setup-header">
        <h2>AI Dependencies Setup</h2>
        <p className="description">
          Docker and Ollama enable AI-powered features like transcription and
          video tagging. Install them using the commands below or use one-click
          installation.
        </p>

        <div className="header-actions">
          <label className="auto-refresh-toggle">
            <input
              type="checkbox"
              checked={autoRefreshing}
              onChange={(e) => setAutoRefreshing(e.target.checked)}
            />
            <span>Auto-refresh status</span>
          </label>
          <button
            className="refresh-button"
            onClick={() => checkInstallationStatus()}
            disabled={loading}
            aria-label="Refresh installation status"
          >
            {loading ? '🔄 Checking...' : '🔄 Refresh'}
          </button>
        </div>
      </div>

      <DependencyCard
        info={dockerInfo}
        copiedCommand={copiedCommand}
        onCopy={copyToClipboard}
        installing={installing.docker}
      />

      <DependencyCard
        info={ollamaInfo}
        copiedCommand={copiedCommand}
        onCopy={copyToClipboard}
        installing={installing.ollama}
      />

      {/* Success State */}
      {status?.docker.installed &&
        status?.docker.running &&
        status?.ollama.installed &&
        status?.ollama.running && (
          <div className="success-banner">
            <div className="success-icon">✅</div>
            <div className="success-content">
              <h3>All dependencies installed and running!</h3>
              <p>
                You're ready to use AI transcription and video tagging features.
              </p>
            </div>
          </div>
        )}
    </div>
  );
}

interface DependencyCardProps {
  info: DependencyInfo;
  copiedCommand: string | null;
  onCopy: (text: string, id: string) => void;
  installing?: boolean;
}

function DependencyCard({
  info,
  copiedCommand,
  onCopy,
  installing,
}: DependencyCardProps) {
  const commandId = `${info.name.toLowerCase()}-install`;
  const isCopied = copiedCommand === commandId;

  const getStatusBadge = () => {
    switch (info.status) {
      case 'installed-running':
        return (
          <div className="status-badge success">
            <span className="status-dot running" />
            <span>Installed & Running</span>
          </div>
        );
      case 'installed-stopped':
        return (
          <div className="status-badge warning">
            <span className="status-dot stopped" />
            <span>Installed (Not Running)</span>
          </div>
        );
      default:
        return (
          <div className="status-badge error">
            <span className="status-dot missing" />
            <span>Not Installed</span>
          </div>
        );
    }
  };

  return (
    <div className={`dependency-card ${info.status}`}>
      <div className="card-header">
        <div className="card-title-group">
          <span className="dependency-icon">{info.icon}</span>
          <div>
            <h3>{info.name}</h3>
            <p className="dependency-description">{info.description}</p>
          </div>
        </div>
        {getStatusBadge()}
      </div>

      {info.status === 'not-installed' && (
        <>
          <div className="command-box">
            <div className="command-label">
              <span>One-liner install command:</span>
              {info.canAutoInstall && (
                <button
                  className="auto-install-button"
                  onClick={info.autoInstallAction}
                  disabled={installing}
                  aria-label={`Auto-install ${info.name}`}
                >
                  {installing ? (
                    <>
                      <span className="spinner-small" />
                      Installing...
                    </>
                  ) : (
                    <>⚡ Auto-install</>
                  )}
                </button>
              )}
            </div>
            <div className="command-container">
              <code
                className="command"
                onClick={() => onCopy(info.installCommand, commandId)}
              >
                {info.installCommand}
              </code>
              <button
                className={`copy-button ${isCopied ? 'copied' : ''}`}
                onClick={() => onCopy(info.installCommand, commandId)}
                title="Copy to clipboard"
                aria-label="Copy command to clipboard"
              >
                {isCopied ? (
                  <>
                    <span className="check-icon">✓</span>
                    Copied
                  </>
                ) : (
                  <>
                    <span className="copy-icon">📋</span>
                    Copy
                  </>
                )}
              </button>
            </div>
          </div>

          <div className="instructions">
            <h4>{info.instructions.title}</h4>
            <ol>
              {info.instructions.steps.map((step, idx) => (
                <li key={idx}>{step}</li>
              ))}
            </ol>
          </div>
        </>
      )}

      {info.status === 'installed-stopped' && (
        <div className="warning-box">
          <div className="warning-icon">⚠️</div>
          <div className="warning-content">
            <strong>{info.name} is installed but not running.</strong>
            {info.name === 'Docker' ? (
              <p>Please start Docker Desktop from your Applications folder.</p>
            ) : (
              <p>
                Start it with: <code>ollama serve</code>
              </p>
            )}
          </div>
        </div>
      )}

      {info.status === 'installed-running' && (
        <div className="success-box">
          <div className="success-icon-small">✓</div>
          <div className="success-content-small">
            <strong>{info.name} is running!</strong>
            <p>All systems operational.</p>
          </div>
        </div>
      )}
    </div>
  );
}
