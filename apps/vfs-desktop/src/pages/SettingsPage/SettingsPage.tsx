/**
 * SettingsPage - Full-page settings view
 * Displays theme customization and app settings
 */
import { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  useTheme,
  themeColors,
  ThemeColorKey,
} from '../../contexts/ThemeContext';
import { resetOnboardingTour } from '../../components/OnboardingTour';
import { AISetup } from '../../components/AISetup';
import { Select } from '../../components/Select';
import './SettingsPage.css';

// Storage keys for settings
const METRICS_INTERVAL_KEY = 'diaspor-metrics-polling-interval';
const DEFAULT_POLLING_INTERVAL = 2000; // 2 seconds

// Get stored polling interval
function getStoredPollingInterval(): number {
  try {
    const stored = localStorage.getItem(METRICS_INTERVAL_KEY);
    if (stored) {
      const interval = parseInt(stored, 10);
      if (interval >= 500 && interval <= 30000) {
        return interval;
      }
    }
  } catch {
    // Ignore
  }
  return DEFAULT_POLLING_INTERVAL;
}

// Save polling interval
function savePollingInterval(interval: number): void {
  try {
    localStorage.setItem(METRICS_INTERVAL_KEY, interval.toString());
    // Emit event for metrics page to pick up
    window.dispatchEvent(
      new CustomEvent('metrics-interval-changed', { detail: interval }),
    );
  } catch {
    // Ignore
  }
}

// Export for other components to use
export { getStoredPollingInterval, METRICS_INTERVAL_KEY };

interface SettingsPageProps {
  onClose?: () => void;
  /** Initial tab to show when opening settings */
  initialTab?: 'settings' | 'theme' | 'ai' | 'logging';
}

const colorDisplayNames: Record<ThemeColorKey, string> = {
  cyan: 'Cyan',
  purple: 'Purple',
  neonCyan: 'Neon Cyan',
  neonMagenta: 'Neon Magenta',
  electricPurple: 'Electric Purple',
  neonGreen: 'Neon Green',
  sunsetOrange: 'Sunset Orange',
  electricBlue: 'Electric Blue',
  cyberYellow: 'Cyber Yellow',
  neonRed: 'Neon Red',
};

export function SettingsPage({
  onClose,
  initialTab = 'settings',
}: SettingsPageProps) {
  const { mode, toggleMode, colorKey, setColorKey } = useTheme();
  const [activeTab, setActiveTab] = useState<
    'settings' | 'theme' | 'ai' | 'logging' | 'audit'
  >(initialTab);
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const [showAuditPage, setShowAuditPage] = useState(false);

  // Advanced settings state
  const [pollingInterval, setPollingInterval] = useState(
    getStoredPollingInterval,
  );

  const handlePollingIntervalChange = useCallback((value: number) => {
    setPollingInterval(value);
    savePollingInterval(value);
  }, []);

  const openDevTools = useCallback(async () => {
    try {
      await invoke('open_devtools');
      // Show instructions as well
      alert(
        'DevTools can be opened via:\n' +
          '• Right-click → Inspect Element\n' +
          '• Cmd+Option+I (Mac) or Ctrl+Shift+I (Windows/Linux)\n\n' +
          'Check the console for more information.',
      );
    } catch (error) {
      console.error('Failed to open dev tools:', error);
      alert(
        'DevTools can be opened via:\n' +
          '• Right-click → Inspect Element\n' +
          '• Cmd+Option+I (Mac) or Ctrl+Shift+I (Windows/Linux)',
      );
    }
  }, []);

  // Close on Escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && onClose) {
        e.preventDefault();
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  return (
    <div className="settings-page">
      <div className="settings-container">
        <div className="settings-header">
          <h1>Settings</h1>
          {onClose && (
            <button className="close-btn" onClick={onClose}>
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path d="M18 6L6 18M6 6l12 12" />
              </svg>
            </button>
          )}
        </div>

        <div className="settings-tabs">
          <button
            className={`settings-tab ${activeTab === 'settings' ? 'active' : ''}`}
            onClick={() => setActiveTab('settings')}
          >
            General
          </button>
          <button
            className={`settings-tab ${activeTab === 'theme' ? 'active' : ''}`}
            onClick={() => setActiveTab('theme')}
          >
            Theme
          </button>
          <button
            className={`settings-tab ${activeTab === 'ai' ? 'active' : ''}`}
            onClick={() => setActiveTab('ai')}
          >
            AI
          </button>
          <button
            className={`settings-tab ${activeTab === 'logging' ? 'active' : ''}`}
            onClick={() => setActiveTab('logging')}
          >
            Logging
          </button>
        </div>

        <div className="settings-content">
          {activeTab === 'settings' && (
            <>
              {/* Onboarding Tour */}
              <div className="settings-section">
                <h2>Onboarding</h2>
                <div className="tour-buttons">
                  <button
                    className="tour-btn"
                    onClick={async () => {
                      try {
                        // Navigate to files tab first, then start tour after delay
                        if (onClose) {
                          onClose();
                        }
                        // Give time for the Files page to render before starting tour
                        await new Promise((resolve) =>
                          setTimeout(resolve, 500),
                        );
                        resetOnboardingTour();
                      } catch (error) {
                        console.error(
                          '[SettingsPage] Failed to start tour:',
                          error,
                        );
                      }
                    }}
                  >
                    <svg
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                    >
                      <circle cx="12" cy="12" r="10" />
                      <path d="M12 6v6l4 2" />
                    </svg>
                    <span>Start Feature Tour</span>
                  </button>
                  <button
                    className="tour-btn secondary"
                    onClick={async () => {
                      try {
                        // Reset tour state first
                        resetOnboardingTour();
                        // Navigate to files tab to show the tour
                        if (onClose) {
                          await new Promise((resolve) =>
                            setTimeout(resolve, 200),
                          );
                          onClose();
                          // Start tour after navigation completes
                          await new Promise((resolve) =>
                            setTimeout(resolve, 500),
                          );
                          resetOnboardingTour();
                        }
                      } catch (error) {
                        console.error(
                          '[SettingsPage] Failed to reset tour:',
                          error,
                        );
                      }
                    }}
                    title="Reset and start onboarding tour now"
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
                    <span>Reset</span>
                  </button>
                </div>
                <p className="tour-description">
                  Take a quick tour to learn about Diaspor features and keyboard
                  shortcuts. Use "Reset" if you want to see it again on next app
                  start.
                </p>
              </div>

              {/* Advanced Settings */}
              <div className="settings-section">
                <h2>Advanced Settings</h2>

                {/* Metrics Polling Interval */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label htmlFor="polling-interval">
                      Metrics Polling Interval
                    </label>
                    <p className="setting-description">
                      How often to update system metrics (CPU, GPU, memory).
                      Lower values = more responsive but uses more resources.
                    </p>
                  </div>
                  <div className="setting-control">
                    <select
                      id="polling-interval"
                      value={pollingInterval}
                      onChange={(e) =>
                        handlePollingIntervalChange(
                          parseInt(e.target.value, 10),
                        )
                      }
                      className="setting-select"
                    >
                      <option value={500}>0.5 seconds (High)</option>
                      <option value={1000}>1 second</option>
                      <option value={2000}>2 seconds (Default)</option>
                      <option value={5000}>5 seconds</option>
                      <option value={10000}>10 seconds</option>
                      <option value={30000}>30 seconds (Low)</option>
                    </select>
                  </div>
                </div>

                {/* Dev Tools */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label>Developer Tools</label>
                    <p className="setting-description">
                      Open browser developer tools for debugging. Useful for
                      troubleshooting issues.
                    </p>
                  </div>
                  <div className="setting-control">
                    <button
                      className="tour-btn secondary"
                      onClick={openDevTools}
                      title="Open Developer Tools"
                    >
                      <svg
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="2"
                      >
                        <path d="M16 18l6-6-6-6M8 6l-6 6 6 6" />
                      </svg>
                      <span>Open Dev Tools</span>
                    </button>
                  </div>
                </div>
              </div>
            </>
          )}

          {activeTab === 'theme' && (
            <>
              {/* Theme Mode */}
              <div className="settings-section">
                <h2>Theme Mode</h2>
                <div className="mode-toggle">
                  <button
                    className={`mode-btn ${mode === 'dark' ? 'active' : ''}`}
                    onClick={() => mode !== 'dark' && toggleMode()}
                  >
                    <svg
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                    >
                      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
                    </svg>
                    <span>Dark</span>
                  </button>
                  <button
                    className={`mode-btn ${mode === 'light' ? 'active' : ''}`}
                    onClick={() => mode !== 'light' && toggleMode()}
                  >
                    <svg
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2"
                    >
                      <circle cx="12" cy="12" r="5" />
                      <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
                    </svg>
                    <span>Light</span>
                  </button>
                </div>
              </div>

              {/* Accent Colors */}
              <div className="settings-section">
                <h2>Accent Color</h2>
                <div className="color-grid">
                  {(Object.keys(themeColors) as ThemeColorKey[]).map((key) => (
                    <button
                      key={key}
                      className={`color-swatch ${colorKey === key ? 'active' : ''}`}
                      onClick={() => setColorKey(key)}
                      style={
                        {
                          '--swatch-color': themeColors[key].primary,
                          '--swatch-secondary': themeColors[key].secondary,
                        } as React.CSSProperties
                      }
                      title={colorDisplayNames[key]}
                    >
                      <div className="swatch-inner" />
                      {colorKey === key && (
                        <svg
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="2.5"
                          className="check-icon"
                        >
                          <path d="M20 6L9 17l-5-5" />
                        </svg>
                      )}
                    </button>
                  ))}
                </div>
                <p className="selected-color">{colorDisplayNames[colorKey]}</p>
              </div>

              {/* Preview */}
              <div className="settings-section">
                <h2>Preview</h2>
                <div className="theme-preview">
                  <div className="preview-sidebar">
                    <div className="preview-item active" />
                    <div className="preview-item" />
                    <div className="preview-item" />
                  </div>
                  <div className="preview-content">
                    <div className="preview-toolbar" />
                    <div className="preview-grid">
                      <div className="preview-file" />
                      <div className="preview-file selected" />
                      <div className="preview-file" />
                      <div className="preview-file" />
                    </div>
                  </div>
                </div>
              </div>
            </>
          )}

          {activeTab === 'ai' && (
            <>
              {/* AI Setup with Advanced Settings */}
              <div className="settings-section">
                <AISetup />
              </div>
            </>
          )}

          {activeTab === 'logging' && (
            <>
              <LoggingSettingsSection />
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// Logging Settings Component
function LoggingSettingsSection() {
  const [logPath, setLogPath] = useState<string>('');
  const [logLevel, setLogLevel] = useState<string>('INFO');
  const [maxFileSize, setMaxFileSize] = useState<number>(10);
  const [maxRotatedFiles, setMaxRotatedFiles] = useState<number>(5);
  const [enableFileLogging, setEnableFileLogging] = useState<boolean>(true);
  const [loading, setLoading] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    loadLoggingSettings();
  }, []);

  const loadLoggingSettings = async () => {
    try {
      const settings = await invoke<{
        log_path: string | null;
        log_level: string | null;
        max_file_size: number | null;
        max_rotated_files: number | null;
        enable_file_logging: boolean | null;
      }>('get_logging_settings');

      if (settings.log_path) {
        setLogPath(settings.log_path);
      }
      if (settings.log_level) {
        setLogLevel(settings.log_level);
      }
      if (settings.max_file_size) {
        setMaxFileSize(settings.max_file_size / (1024 * 1024)); // Convert to MB
      }
      if (settings.max_rotated_files) {
        setMaxRotatedFiles(settings.max_rotated_files);
      }
      if (settings.enable_file_logging !== null) {
        setEnableFileLogging(settings.enable_file_logging);
      }
    } catch (err) {
      console.error('Failed to load logging settings:', err);
    }
  };

  const handleSave = async () => {
    setLoading(true);
    setSaved(false);
    try {
      await invoke('update_logging_settings', {
        logPath: logPath || null,
        logLevel: logLevel || null,
        maxFileSize: maxFileSize ? maxFileSize * 1024 * 1024 : null, // Convert to bytes
        maxRotatedFiles: maxRotatedFiles || null,
        enableFileLogging: enableFileLogging,
      });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (err) {
      console.error('Failed to save logging settings:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleBrowseLogPath = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select Log Directory',
      });
      if (selected && typeof selected === 'string') {
        setLogPath(selected);
      } else if (Array.isArray(selected) && selected.length > 0) {
        setLogPath(selected[0]);
      }
    } catch (err) {
      console.error('Failed to browse for log path:', err);
    }
  };

  return (
    <div className="settings-section">
      <h2>Logging</h2>
      <p className="section-description">
        Configure application logging settings. Logs help troubleshoot issues
        and track operations.
      </p>

      <div className="settings-form">
        <div className="form-group">
          <label>
            <input
              type="checkbox"
              checked={enableFileLogging}
              onChange={(e) => setEnableFileLogging(e.target.checked)}
            />
            <span>Enable File Logging</span>
          </label>
          <p className="form-hint">
            When enabled, logs are written to disk for troubleshooting.
          </p>
        </div>

        {enableFileLogging && (
          <>
            <div className="form-group">
              <label htmlFor="log-path">Log Directory Path</label>
              <div className="input-with-button">
                <input
                  id="log-path"
                  type="text"
                  value={logPath}
                  onChange={(e) => setLogPath(e.target.value)}
                  placeholder="Default: ~/.local/share/diaspor/logs"
                />
                <button
                  type="button"
                  onClick={handleBrowseLogPath}
                  className="browse-btn"
                >
                  Browse
                </button>
              </div>
              <p className="form-hint">
                Leave empty to use default location. Changes require app
                restart.
              </p>
            </div>

            <div className="form-group">
              <Select
                id="log-level"
                label="Log Level"
                value={logLevel}
                onChange={(value) => setLogLevel(value)}
                options={[
                  { value: 'TRACE', label: 'TRACE' },
                  { value: 'DEBUG', label: 'DEBUG' },
                  { value: 'INFO', label: 'INFO' },
                  { value: 'WARN', label: 'WARN' },
                  { value: 'ERROR', label: 'ERROR' },
                ]}
                fullWidth
              />
              <p className="form-hint">
                Minimum log level to record. Higher levels include lower ones.
              </p>
            </div>

            <div className="form-group">
              <label htmlFor="max-file-size">
                Max File Size (MB): {maxFileSize}
              </label>
              <input
                id="max-file-size"
                type="range"
                min="1"
                max="100"
                value={maxFileSize}
                onChange={(e) => setMaxFileSize(Number(e.target.value))}
              />
              <p className="form-hint">
                Log files are rotated when they reach this size.
              </p>
            </div>

            <div className="form-group">
              <label htmlFor="max-rotated-files">
                Max Rotated Files: {maxRotatedFiles}
              </label>
              <input
                id="max-rotated-files"
                type="range"
                min="1"
                max="10"
                value={maxRotatedFiles}
                onChange={(e) => setMaxRotatedFiles(Number(e.target.value))}
              />
              <p className="form-hint">
                Number of rotated log files to keep before deleting oldest.
              </p>
            </div>
          </>
        )}

        <div className="form-actions">
          <button onClick={handleSave} disabled={loading} className="save-btn">
            {loading ? 'Saving...' : saved ? '✓ Saved' : 'Save Settings'}
          </button>
        </div>
      </div>
    </div>
  );
}
