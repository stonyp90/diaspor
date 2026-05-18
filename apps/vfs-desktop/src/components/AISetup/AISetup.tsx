/**
 * AISetup - User-Friendly AI Setup Experience
 *
 * Designed for non-technical users with:
 * - One-click setup with visual progress
 * - Feature-focused cards (what can I do?)
 * - Push-and-play model management
 * - Beautiful, intuitive design
 */

import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useToast } from '../Toast';
import { Select, SelectOption } from '../Select/Select';
import { AIAdvancedSettings } from '../AIAdvancedSettings';
import './AISetup.css';

interface SetupStatus {
  docker: 'checking' | 'installed' | 'not-installed';
  ollama: 'checking' | 'installed' | 'not-installed';
  whisperCpp: 'checking' | 'installed' | 'not-installed';
  transcriptionModel: 'checking' | 'installed' | 'not-installed';
  taggingModel: 'checking' | 'installed' | 'not-installed';
  transcriptionModelRunning: boolean;
  taggingModelRunning: boolean;
}

const REQUIRED_MODELS = {
  transcription: 'whisper',
  tagging: 'llava',
};

interface FeatureConfig {
  transcription: {
    enabled: boolean;
    language: string;
    autoTranscribe: boolean;
  };
  autoTagging: {
    enabled: boolean;
    autoTag: boolean;
    confidenceThreshold: number;
  };
}

const DEFAULT_CONFIG: FeatureConfig = {
  transcription: {
    enabled: false,
    language: 'auto',
    autoTranscribe: false,
  },
  autoTagging: {
    enabled: false,
    autoTag: false,
    confidenceThreshold: 0.7,
  },
};

const LANGUAGE_OPTIONS: SelectOption[] = [
  { value: 'auto', label: 'Auto-detect' },
  { value: 'en', label: 'English' },
  { value: 'es', label: 'Spanish' },
  { value: 'fr', label: 'French' },
  { value: 'de', label: 'German' },
  { value: 'it', label: 'Italian' },
  { value: 'pt', label: 'Portuguese' },
  { value: 'ru', label: 'Russian' },
  { value: 'ja', label: 'Japanese' },
  { value: 'zh', label: 'Chinese' },
  { value: 'ko', label: 'Korean' },
];

// ============================================================================
// Icon Components
// ============================================================================

const MicrophoneIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
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

const TagIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z" />
    <line x1="7" y1="7" x2="7.01" y2="7" />
  </svg>
);

const SparklesIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M12 3l1.912 5.813a2 2 0 0 0 1.275 1.275L21 12l-5.813 1.912a2 2 0 0 0-1.275 1.275L12 21l-1.912-5.813a2 2 0 0 0-1.275-1.275L3 12l5.813-1.912a2 2 0 0 0 1.275-1.275L12 3z" />
  </svg>
);

const ZapIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
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
    strokeWidth="1.5"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
  </svg>
);

const CheckIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <polyline points="20 6 9 17 4 12" />
  </svg>
);

const PlayIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <polygon points="5 3 19 12 5 21 5 3" />
  </svg>
);

const StopIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <rect x="6" y="6" width="12" height="12" rx="2" />
  </svg>
);

const DownloadIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
    <polyline points="7 10 12 15 17 10" />
    <line x1="12" y1="15" x2="12" y2="3" />
  </svg>
);

const ChevronDownIcon: React.FC<{ className?: string }> = ({ className }) => (
  <svg
    className={className}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <polyline points="6 9 12 15 18 9" />
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

export const AISetup: React.FC = () => {
  const { showToast } = useToast();
  const [status, setStatus] = useState<SetupStatus>({
    docker: 'checking',
    ollama: 'checking',
    whisperCpp: 'checking',
    transcriptionModel: 'checking',
    taggingModel: 'checking',
    transcriptionModelRunning: false,
    taggingModelRunning: false,
  });
  const [isInstalling, setIsInstalling] = useState(false);
  const [installStep, setInstallStep] = useState<string>('');
  const [installProgress, setInstallProgress] = useState<number>(0);
  const [error, setError] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<{
    model: string;
    progress?: number;
    status?: string;
  } | null>(null);
  const [servingModel, setServingModel] = useState<string | null>(null);
  const [config, setConfig] = useState<FeatureConfig>(DEFAULT_CONFIG);
  const [transcriptionStarted, setTranscriptionStarted] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showManualInstall, setShowManualInstall] = useState(false);
  const [manualInstallInstructions, setManualInstallInstructions] = useState<{
    platform: string;
    method: string;
    command: string;
    alternative: string | null;
    url: string | null;
  } | null>(null);

  // Load saved config and transcription status
  useEffect(() => {
    try {
      const saved = localStorage.getItem('ai_feature_config');
      if (saved) {
        setConfig({ ...DEFAULT_CONFIG, ...JSON.parse(saved) });
      }
      const transcriptionStatus = localStorage.getItem(
        'transcription_model_started',
      );
      if (transcriptionStatus === 'true') {
        setTranscriptionStarted(true);
      }
    } catch (err) {
      console.error('Failed to load AI config:', err);
    }
  }, []);

  // Save config
  const saveConfig = useCallback((newConfig: FeatureConfig) => {
    setConfig(newConfig);
    localStorage.setItem('ai_feature_config', JSON.stringify(newConfig));
  }, []);

  // Check all dependencies
  const checkStatus = useCallback(async () => {
    try {
      // Check Docker
      try {
        const dockerInstalled = await invoke<boolean>('check_docker_installed');
        const dockerRunning = await invoke<boolean>(
          'check_docker_running',
        ).catch(() => false);
        setStatus((prev) => ({
          ...prev,
          docker:
            dockerInstalled && dockerRunning ? 'installed' : 'not-installed',
        }));
      } catch {
        setStatus((prev) => ({ ...prev, docker: 'not-installed' }));
      }

      // Check FFmpeg for transcription
      try {
        const ffmpegInstalled = await invoke<boolean>('check_ffmpeg_installed');
        const transcriptionAvailable = await invoke<boolean>(
          'vfs_is_transcription_available',
        ).catch(() => ffmpegInstalled);

        // Check whisper-cpp for high-quality transcription
        const whisperCppInstalled = await invoke<boolean>(
          'check_whisper_cpp_installed',
        ).catch(() => false);

        const wasStarted =
          transcriptionStarted ||
          localStorage.getItem('transcription_model_started') === 'true';

        setStatus((prev) => ({
          ...prev,
          whisperCpp: whisperCppInstalled ? 'installed' : 'not-installed',
          transcriptionModel:
            ffmpegInstalled && transcriptionAvailable
              ? 'installed'
              : 'not-installed',
          transcriptionModelRunning:
            ffmpegInstalled && transcriptionAvailable && wasStarted,
        }));
      } catch {
        setStatus((prev) => ({
          ...prev,
          whisperCpp: 'not-installed',
          transcriptionModel: 'not-installed',
          transcriptionModelRunning: false,
        }));
      }

      // Check Ollama via HTTP API for tagging
      try {
        const response = await fetch('http://localhost:11434/api/tags');
        if (response.ok) {
          const data = (await response.json()) as {
            models?: Array<{ name: string }>;
          };
          setStatus((prev) => ({ ...prev, ollama: 'installed' }));

          const models = data.models || [];
          const modelNames = models.map((m) => m.name.toLowerCase());
          const hasTagging = modelNames.some(
            (n) =>
              n === 'llava' ||
              n === 'llava:latest' ||
              n.includes('llava:') ||
              n.includes('llava'),
          );

          setStatus((prev) => ({
            ...prev,
            taggingModel: hasTagging ? 'installed' : 'not-installed',
          }));

          // Check running tagging models
          try {
            const runningResponse = await invoke<{
              models?: Array<{ name: string; model?: string }>;
            }>('ollama_ps');
            const runningModels = runningResponse.models || [];
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

            setStatus((prev) => ({
              ...prev,
              taggingModelRunning: taggingRunning,
            }));
          } catch {
            setStatus((prev) => ({
              ...prev,
              taggingModelRunning: false,
            }));
          }
        } else {
          setStatus((prev) => ({ ...prev, ollama: 'not-installed' }));
        }
      } catch {
        setStatus((prev) => ({ ...prev, ollama: 'not-installed' }));
      }
    } catch (err) {
      console.error('Failed to check AI setup status:', err);
    }
  }, [transcriptionStarted]);

  // Install everything with one click
  const installAll = async () => {
    setIsInstalling(true);
    setError(null);
    setInstallStep('Preparing installation...');
    setInstallProgress(5);

    try {
      setInstallStep('Installing FFmpeg, Ollama, and AI models...');
      setInstallProgress(15);

      try {
        const unifiedResultPromise = invoke<{
          success: boolean;
          message: string;
          requires_restart: boolean;
        }>('install_all_ai_dependencies');

        const timeoutPromise = new Promise<never>((_, reject) =>
          setTimeout(
            () =>
              reject(
                new Error(
                  'Installation timed out. Please check your network connection.',
                ),
              ),
            15 * 60 * 1000,
          ),
        );

        const unifiedResult = await Promise.race([
          unifiedResultPromise,
          timeoutPromise,
        ]);

        if (unifiedResult.success) {
          setInstallProgress(100);
          setInstallStep('Installation complete!');
          showToast({
            type: 'success',
            message: 'AI features are ready to use!',
          });
          setTimeout(() => {
            checkStatus();
            setIsInstalling(false);
            setInstallStep('');
            setInstallProgress(0);
          }, 1500);
          return;
        }
      } catch (unifiedErr) {
        console.log('Unified install attempt:', unifiedErr);
      }

      // Fallback installation steps
      if (status.transcriptionModel === 'not-installed') {
        setInstallStep('Installing FFmpeg...');
        setInstallProgress(25);
        try {
          const result = await invoke<{ success: boolean; message: string }>(
            'install_ffmpeg',
          );
          if (!result.success) {
            // If automatic installation failed, show manual instructions
            setShowManualInstall(true);
            throw new Error(result.message || 'FFmpeg installation failed');
          }
          await checkStatus();
        } catch (err) {
          const errorMsg = err instanceof Error ? err.message : String(err);
          // Don't throw immediately - show manual install option
          if (!errorMsg.includes('manually')) {
            setShowManualInstall(true);
          }
          throw new Error(`FFmpeg: ${errorMsg}`);
        }
      }

      if (status.ollama === 'not-installed') {
        setInstallStep('Installing Ollama...');
        setInstallProgress(50);
        try {
          const result = await invoke<{ success: boolean; message: string }>(
            'install_ollama',
          );
          if (!result.success) {
            throw new Error(result.message || 'Ollama installation failed');
          }
          await new Promise((resolve) => setTimeout(resolve, 3000));
          let retries = 10;
          while (retries > 0) {
            try {
              const isRunning = await invoke<boolean>('check_ollama_running');
              if (isRunning) break;
            } catch {
              // Will retry
            }
            await new Promise((resolve) => setTimeout(resolve, 2000));
            retries--;
          }
          await checkStatus();
        } catch (err) {
          const errorMsg = err instanceof Error ? err.message : String(err);
          throw new Error(`Ollama: ${errorMsg}`);
        }
      }

      if (status.taggingModel === 'not-installed') {
        setInstallStep('Downloading AI model...');
        setInstallProgress(75);
        setDownloadProgress({ model: REQUIRED_MODELS.tagging });
        try {
          await invoke('ollama_pull', { model: REQUIRED_MODELS.tagging });
          setDownloadProgress(null);
          await checkStatus();
        } catch (err) {
          setDownloadProgress(null);
          const errorMsg = err instanceof Error ? err.message : String(err);
          throw new Error(`Failed to download AI model: ${errorMsg}`);
        }
      }

      setInstallProgress(100);
      setInstallStep('All done!');
      await checkStatus();
      showToast({
        type: 'success',
        message: 'AI features are ready to use!',
      });
      setTimeout(() => {
        setIsInstalling(false);
        setInstallStep('');
        setInstallProgress(0);
        setError(null);
      }, 1500);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      console.error('Installation failed:', err);
      setError(errorMsg);
      setIsInstalling(false);
      setInstallStep('');
      setInstallProgress(0);
    }
  };

  // Start transcription model
  const startTranscriptionModel = async () => {
    setServingModel('transcription');
    setError(null);
    try {
      const ffmpegInstalled = await invoke<boolean>('check_ffmpeg_installed');
      if (!ffmpegInstalled) {
        throw new Error('FFmpeg is not installed.');
      }

      const transcriptionAvailable = await invoke<boolean>(
        'vfs_is_transcription_available',
      );
      if (!transcriptionAvailable) {
        throw new Error('Transcription is not available.');
      }

      localStorage.setItem('transcription_model_started', 'true');
      setTranscriptionStarted(true);
      await checkStatus();
      showToast({
        type: 'success',
        message: 'Transcription ready!',
      });
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(`Transcription: ${errorMsg}`);
    } finally {
      setServingModel(null);
    }
  };

  // Stop transcription model
  const stopTranscriptionModel = async () => {
    setServingModel('transcription');
    try {
      localStorage.setItem('transcription_model_started', 'false');
      setTranscriptionStarted(false);
      setStatus((prev) => ({
        ...prev,
        transcriptionModelRunning: false,
      }));
      showToast({
        type: 'success',
        message: 'Transcription stopped',
      });
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(`Stop transcription: ${errorMsg}`);
    } finally {
      setServingModel(null);
    }
  };

  // Serve tagging model
  const serveModel = async (modelName: string) => {
    setServingModel('tagging');
    setError(null);
    try {
      try {
        const response = await fetch('http://localhost:11434/api/tags');
        if (response.ok) {
          const data = (await response.json()) as {
            models?: Array<{ name: string }>;
          };
          const models = data.models || [];
          const modelNames = models.map((m) => m.name.toLowerCase());
          const modelExists = modelNames.some((n) =>
            n.includes(modelName.toLowerCase()),
          );

          if (!modelExists) {
            throw new Error(
              `Model '${modelName}' not found. Please install it first.`,
            );
          }
        }
      } catch (checkErr) {
        const checkErrorMsg =
          checkErr instanceof Error ? checkErr.message : String(checkErr);
        throw new Error(`${checkErrorMsg}. Make sure Ollama is running.`);
      }

      await invoke('ollama_run', {
        model: modelName,
        resourceLimits: null,
      });
      await new Promise((resolve) => setTimeout(resolve, 2000));
      await checkStatus();
      showToast({
        type: 'success',
        message: 'Smart Tagging ready!',
      });
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(`Tagging: ${errorMsg}`);
    } finally {
      setServingModel(null);
    }
  };

  // Stop model
  const stopModel = async (modelType: 'transcription' | 'tagging') => {
    if (modelType === 'transcription') {
      await stopTranscriptionModel();
      return;
    }
    setServingModel(modelType);
    try {
      await invoke('ollama_stop', { model: 'llava' });
      await new Promise((resolve) => setTimeout(resolve, 1000));
      await checkStatus();
      showToast({
        type: 'success',
        message: 'Model stopped',
      });
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setError(`Stop model: ${errorMsg}`);
    } finally {
      setServingModel(null);
    }
  };

  // Listen for Ollama pull progress events
  useEffect(() => {
    const unlistenProgress = listen<{
      model: string;
      status?: string;
      progress?: number;
    }>('ollama-pull-progress', (event) => {
      const payload = event.payload;
      setDownloadProgress({
        model: payload.model,
        progress: payload.progress,
        status: payload.status,
      });

      if (payload.progress !== undefined) {
        setInstallStep(
          `Downloading AI model... ${payload.progress.toFixed(0)}%`,
        );
        setInstallProgress(75 + (payload.progress / 100) * 20);
      } else if (payload.status) {
        setInstallStep(`Downloading AI model... ${payload.status}`);
      }
    });

    const unlistenComplete = listen<{ model: string }>(
      'ollama-pull-complete',
      () => {
        setDownloadProgress(null);
        setInstallStep('Model downloaded!');
        setInstallProgress(95);
      },
    );

    return () => {
      unlistenProgress.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
    };
  }, []);

  // Load manual installation instructions
  useEffect(() => {
    const loadInstructions = async () => {
      try {
        const instructions = await invoke<{
          platform: string;
          method: string;
          command: string;
          alternative: string | null;
          url: string | null;
        }>('get_ffmpeg_install_instructions');
        setManualInstallInstructions(instructions);
      } catch (err) {
        console.error('Failed to load FFmpeg install instructions:', err);
      }
    };
    loadInstructions();
  }, []);

  // Initial check
  useEffect(() => {
    checkStatus();
    const interval = setInterval(checkStatus, 8000);
    return () => clearInterval(interval);
  }, [checkStatus]);

  // Handle feature toggle with auto-start
  const handleFeatureToggle = useCallback(
    async (feature: 'transcription' | 'tagging', enabled: boolean) => {
      if (feature === 'transcription') {
        const newConfig = {
          ...config,
          transcription: { ...config.transcription, enabled },
        };
        saveConfig(newConfig);

        if (enabled && !status.transcriptionModelRunning) {
          setTimeout(() => startTranscriptionModel(), 150);
        } else if (!enabled && status.transcriptionModelRunning) {
          stopModel('transcription');
        }
      } else {
        const newConfig = {
          ...config,
          autoTagging: { ...config.autoTagging, enabled },
        };
        saveConfig(newConfig);

        if (
          enabled &&
          !status.taggingModelRunning &&
          status.taggingModel === 'installed'
        ) {
          setTimeout(() => serveModel(REQUIRED_MODELS.tagging), 150);
        } else if (!enabled && status.taggingModelRunning) {
          stopModel('tagging');
        }
      }
    },
    [
      config,
      saveConfig,
      status,
      startTranscriptionModel,
      serveModel,
      stopModel,
    ],
  );

  // whisper-cpp is optional but recommended for transcription
  const allInstalled =
    status.transcriptionModel === 'installed' &&
    status.ollama === 'installed' &&
    status.taggingModel === 'installed';

  const allReady = allInstalled && !isInstalling;

  const isChecking =
    status.docker === 'checking' ||
    status.ollama === 'checking' ||
    status.whisperCpp === 'checking' ||
    status.transcriptionModel === 'checking' ||
    status.taggingModel === 'checking';

  return (
    <div className="ai-setup-container">
      {/* Hero Section */}
      <div className="ai-hero">
        <div className="hero-icon">
          <SparklesIcon />
        </div>
        <div className="hero-content">
          <h2>AI-Powered Features</h2>
          <p>
            Transform your media with intelligent transcription and automatic
            tagging
          </p>
        </div>
      </div>

      {/* Error Display */}
      {error && (
        <div className="ai-error-banner">
          <div className="error-icon">
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
          </div>
          <div className="error-content">
            <span className="error-title">Something went wrong</span>
            <span className="error-message">{error}</span>
          </div>
          <button className="error-dismiss" onClick={() => setError(null)}>
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      )}

      {/* Main Content */}
      {allReady ? (
        <>
          {/* Feature Cards */}
          <div className="feature-cards">
            {/* Speech-to-Text Feature */}
            <div
              className={`feature-card ${config.transcription.enabled ? 'active' : ''}`}
            >
              <div className="feature-card-header">
                <div className="feature-icon speech">
                  <MicrophoneIcon />
                </div>
                <div className="feature-details">
                  <h3>Speech-to-Text</h3>
                  <p>Transcribe audio and video files automatically</p>
                </div>
                <label className="feature-toggle">
                  <input
                    type="checkbox"
                    checked={config.transcription.enabled}
                    onChange={(e) =>
                      handleFeatureToggle('transcription', e.target.checked)
                    }
                  />
                  <span className="toggle-track">
                    <span className="toggle-thumb" />
                  </span>
                </label>
              </div>

              {config.transcription.enabled && (
                <div className="feature-card-content">
                  <div className="feature-status">
                    {status.transcriptionModelRunning ? (
                      <div className="status-ready">
                        <CheckIcon />
                        <span>Ready • Click on media files to transcribe</span>
                      </div>
                    ) : (
                      <div className="status-loading">
                        <span className="loading-spinner" />
                        <span>Starting...</span>
                      </div>
                    )}
                  </div>

                  <div className="feature-options">
                    <div className="option-row">
                      <span className="option-label">Language</span>
                      <div className="option-input">
                        <Select
                          value={config.transcription.language}
                          options={LANGUAGE_OPTIONS}
                          onChange={(value) =>
                            saveConfig({
                              ...config,
                              transcription: {
                                ...config.transcription,
                                language: value,
                              },
                            })
                          }
                        />
                      </div>
                    </div>

                    <label className="option-toggle">
                      <span className="option-label">
                        Auto-transcribe on open
                      </span>
                      <input
                        type="checkbox"
                        checked={config.transcription.autoTranscribe}
                        onChange={(e) =>
                          saveConfig({
                            ...config,
                            transcription: {
                              ...config.transcription,
                              autoTranscribe: e.target.checked,
                            },
                          })
                        }
                      />
                      <span className="mini-toggle" />
                    </label>
                  </div>

                  {status.transcriptionModelRunning && (
                    <button
                      className="control-btn stop"
                      onClick={() => stopModel('transcription')}
                      disabled={servingModel === 'transcription'}
                    >
                      <StopIcon />
                      <span>
                        {servingModel === 'transcription'
                          ? 'Stopping...'
                          : 'Stop'}
                      </span>
                    </button>
                  )}
                </div>
              )}
            </div>

            {/* Smart Tagging Feature */}
            <div
              className={`feature-card ${config.autoTagging.enabled ? 'active' : ''}`}
            >
              <div className="feature-card-header">
                <div className="feature-icon tagging">
                  <TagIcon />
                </div>
                <div className="feature-details">
                  <h3>Smart Tagging</h3>
                  <p>AI-powered tag suggestions for images and videos</p>
                </div>
                <label className="feature-toggle">
                  <input
                    type="checkbox"
                    checked={config.autoTagging.enabled}
                    onChange={(e) =>
                      handleFeatureToggle('tagging', e.target.checked)
                    }
                  />
                  <span className="toggle-track">
                    <span className="toggle-thumb" />
                  </span>
                </label>
              </div>

              {config.autoTagging.enabled && (
                <div className="feature-card-content">
                  <div className="feature-status">
                    {status.taggingModelRunning ? (
                      <div className="status-ready">
                        <CheckIcon />
                        <span>Ready • Right-click files to generate tags</span>
                      </div>
                    ) : (
                      <div className="status-loading">
                        <span className="loading-spinner" />
                        <span>Starting AI model...</span>
                      </div>
                    )}
                  </div>

                  <div className="feature-options">
                    <label className="option-toggle">
                      <span className="option-label">
                        Auto-tag when viewing
                      </span>
                      <input
                        type="checkbox"
                        checked={config.autoTagging.autoTag}
                        onChange={(e) =>
                          saveConfig({
                            ...config,
                            autoTagging: {
                              ...config.autoTagging,
                              autoTag: e.target.checked,
                            },
                          })
                        }
                      />
                      <span className="mini-toggle" />
                    </label>
                  </div>

                  {status.taggingModelRunning && (
                    <button
                      className="control-btn stop"
                      onClick={() => stopModel('tagging')}
                      disabled={servingModel === 'tagging'}
                    >
                      <StopIcon />
                      <span>
                        {servingModel === 'tagging' ? 'Stopping...' : 'Stop'}
                      </span>
                    </button>
                  )}
                </div>
              )}
            </div>
          </div>

          {/* Advanced Settings Toggle */}
          <button
            className={`advanced-toggle ${showAdvanced ? 'open' : ''}`}
            onClick={() => setShowAdvanced(!showAdvanced)}
          >
            <SettingsIcon />
            <span>Advanced Settings</span>
            <ChevronDownIcon className="chevron" />
          </button>

          {/* Advanced Settings Panel */}
          {showAdvanced && (
            <div className="advanced-panel">
              <AIAdvancedSettings />
            </div>
          )}
        </>
      ) : (
        <>
          {/* Setup Required State */}
          <div className="setup-prompt">
            <div className="setup-visual">
              <div className="setup-icon-stack">
                <div className="setup-icon-bg" />
                <DownloadIcon className="setup-icon" />
              </div>
            </div>
            <div className="setup-content">
              <h3>Get Started with AI</h3>
              <p>
                Install the required components to enable transcription and
                smart tagging. This is a one-time setup that takes a few
                minutes.
              </p>
              <div className="setup-features">
                <div className="setup-feature">
                  <MicrophoneIcon />
                  <span>Speech-to-Text</span>
                </div>
                <div className="setup-feature">
                  <TagIcon />
                  <span>Smart Tagging</span>
                </div>
              </div>
            </div>
          </div>

          {/* Install Action */}
          <div className="install-section">
            {isInstalling ? (
              <div className="install-progress">
                <div className="progress-header">
                  <span className="progress-step">{installStep}</span>
                  <span className="progress-percent">
                    {Math.round(installProgress)}%
                  </span>
                </div>
                <div className="progress-bar">
                  <div
                    className="progress-fill"
                    style={{ width: `${installProgress}%` }}
                  />
                </div>
              </div>
            ) : (
              <div className="install-actions">
                <button
                  className="install-btn primary"
                  onClick={installAll}
                  disabled={isChecking}
                >
                  <ZapIcon />
                  <span>Install AI Features</span>
                </button>
                <button
                  className="install-btn secondary"
                  onClick={checkStatus}
                  disabled={isChecking}
                >
                  <RefreshIcon className={isChecking ? 'spinning' : ''} />
                  <span>Check Status</span>
                </button>
              </div>
            )}
          </div>

          {/* Current Status */}
          <div className="status-section">
            <h4>Component Status</h4>
            <div className="status-items">
              <StatusItem
                name="FFmpeg"
                description="Audio/video processing"
                status={status.transcriptionModel}
              />
              <StatusItem
                name="Whisper"
                description="Speech-to-text engine"
                status={status.whisperCpp}
              />
              <StatusItem
                name="Ollama"
                description="AI engine"
                status={status.ollama}
              />
              <StatusItem
                name="LLaVA Model"
                description="Smart tagging"
                status={status.taggingModel}
              />
            </div>
          </div>

          {/* Manual Installation Section */}
          {(showManualInstall ||
            status.transcriptionModel === 'not-installed') &&
            manualInstallInstructions && (
              <div className="manual-install-section">
                <button
                  className="manual-install-toggle"
                  onClick={() => setShowManualInstall(!showManualInstall)}
                >
                  <ChevronDownIcon
                    className={`chevron ${showManualInstall ? 'open' : ''}`}
                  />
                  <span>Manual Installation Instructions</span>
                </button>
                {showManualInstall && (
                  <div className="manual-install-content">
                    <div className="manual-install-platform">
                      <strong>Platform:</strong>{' '}
                      {manualInstallInstructions.platform}
                    </div>
                    <div className="manual-install-method">
                      <strong>Method:</strong>{' '}
                      {manualInstallInstructions.method}
                    </div>
                    <div className="manual-install-command">
                      <strong>Command:</strong>
                      <div className="command-box">
                        <code>{manualInstallInstructions.command}</code>
                        <button
                          className="copy-command-btn"
                          onClick={() => {
                            navigator.clipboard.writeText(
                              manualInstallInstructions.command,
                            );
                            showToast({
                              type: 'success',
                              message: 'Command copied to clipboard',
                            });
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
                    </div>
                    {manualInstallInstructions.alternative && (
                      <div className="manual-install-alternative">
                        <strong>Alternative:</strong>{' '}
                        {manualInstallInstructions.alternative}
                      </div>
                    )}
                    {manualInstallInstructions.url && (
                      <div className="manual-install-url">
                        <strong>Download:</strong>{' '}
                        <a
                          href={manualInstallInstructions.url}
                          target="_blank"
                          rel="noopener noreferrer"
                        >
                          {manualInstallInstructions.url}
                        </a>
                      </div>
                    )}
                    <div className="manual-install-note">
                      <strong>Note:</strong> After installing FFmpeg manually,
                      click "Check Status" to verify installation.
                    </div>
                  </div>
                )}
              </div>
            )}
        </>
      )}
    </div>
  );
};

// ============================================================================
// Status Item Component
// ============================================================================

interface StatusItemProps {
  name: string;
  description: string;
  status: 'checking' | 'installed' | 'not-installed';
}

function StatusItem({ name, description, status }: StatusItemProps) {
  return (
    <div className={`status-item ${status}`}>
      <div className="status-indicator">
        {status === 'checking' && <span className="status-spinner" />}
        {status === 'installed' && <CheckIcon />}
        {status === 'not-installed' && (
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="12" cy="12" r="10" />
          </svg>
        )}
      </div>
      <div className="status-info">
        <span className="status-name">{name}</span>
        <span className="status-desc">{description}</span>
      </div>
      <span className="status-badge">
        {status === 'installed'
          ? 'Ready'
          : status === 'checking'
            ? 'Checking'
            : 'Not installed'}
      </span>
    </div>
  );
}

export default AISetup;
