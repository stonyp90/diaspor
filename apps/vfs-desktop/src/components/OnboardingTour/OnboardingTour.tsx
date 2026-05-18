/**
 * OnboardingTour Component
 * Simple step-by-step guided tour using react-joyride
 * Optimized for Tauri + React StrictMode environment
 */
import { useState, useEffect, useCallback, useRef } from 'react';
import Joyride, { Step, CallBackProps, STATUS, ACTIONS } from 'react-joyride';
import './OnboardingTour.css';

interface OnboardingTourProps {
  autoStart?: boolean;
  onComplete?: () => void;
  onSkip?: () => void;
}

const STORAGE_KEY = 'ursly-onboarding-completed';

// Tour steps - simple and declarative
const TOUR_STEPS: Step[] = [
  {
    target: 'body',
    placement: 'center',
    disableBeacon: true,
    content: (
      <div className="tour-step-content">
        <h3>Welcome to Diaspor</h3>
        <p className="tour-subtitle">
          Virtual File System for Multi-Cloud Storage
        </p>
        <ul>
          <li>
            Single interface for AWS S3, Azure, GCS, Oracle, NFS, SMB & SFTP
          </li>
          <li>AI-powered auto-tagging and audio/video transcription</li>
          <li>Real-time CPU, GPU, memory & network monitoring with alerts</li>
          <li>Secure cross-storage file transfers with operation tracking</li>
        </ul>
        <div className="tour-step-tip">
          <strong>Note:</strong> Restart this tour anytime from Settings →
          General.
        </div>
      </div>
    ),
  },
  {
    target: '.action-pill.search',
    placement: 'top',
    disableBeacon: true,
    content: (
      <div className="tour-step-content">
        <h3>Spotlight Search</h3>
        <p>
          Search files across all mounted storage providers instantly. Use{' '}
          <kbd>Cmd+K</kbd> (Mac) or <kbd>Ctrl+K</kbd> (Windows/Linux).
        </p>
        <div className="tour-step-tip">
          <strong>Smart Results:</strong> Search includes file metadata,
          AI-generated tags, and transcription text.
        </div>
      </div>
    ),
  },
  {
    target: '.add-storage-btn',
    placement: 'right',
    disableBeacon: true,
    content: (
      <div className="tour-step-content">
        <h3>Mount Storage</h3>
        <p>
          Add cloud storage (<strong>S3</strong>, <strong>Azure Blob</strong>,{' '}
          <strong>GCS</strong>, <strong>Oracle</strong>) or network drives (
          <strong>NFS</strong>, <strong>SMB</strong>, <strong>SFTP</strong>).
          All providers appear as local folders.
        </p>
        <div className="tour-step-tip">
          <strong>Security:</strong> All credentials are AES-256 encrypted and
          stored locally.
        </div>
      </div>
    ),
  },
  {
    target: '.finder-toolbar',
    placement: 'bottom',
    disableBeacon: true,
    content: (
      <div className="tour-step-content">
        <h3>File Browser & Navigation</h3>
        <p>
          Navigate folders, switch between grid and list views, toggle hidden
          files, and upload content. Drag files between providers for
          cross-storage transfers.
        </p>
        <div className="tour-step-tip">
          <strong>AI Features:</strong> Right-click any file to generate AI tags
          or transcribe audio/video content.
        </div>
      </div>
    ),
  },
  {
    target: '.header-tab[data-tab="metrics"]',
    placement: 'bottom',
    disableBeacon: true,
    content: (
      <div className="tour-step-content">
        <h3>System Metrics</h3>
        <p>
          Monitor CPU, GPU, RAM, disk I/O, and network bandwidth in real-time.
          View historical trends and identify performance bottlenecks.
        </p>
        <div className="tour-step-tip">
          <strong>Alerts:</strong> Set custom thresholds to get notified when
          any metric exceeds limits.
        </div>
      </div>
    ),
  },
  {
    target: '.header-tab[data-tab="settings"]',
    placement: 'bottom',
    disableBeacon: true,
    content: (
      <div className="tour-step-content">
        <h3>Settings & AI</h3>
        <p>
          Configure themes, set up AI providers for auto-tagging and
          transcription, and manage metric alert thresholds.
        </p>
        <div className="tour-step-tip">
          <strong>Privacy-First:</strong> Use Ollama for 100% local AI
          processing — no data leaves your machine.
        </div>
      </div>
    ),
  },
];

// Global ref for external control (reset from settings)
let tourControl: { start: () => void } | null = null;

export function OnboardingTour({
  autoStart = false,
  onComplete,
  onSkip,
}: OnboardingTourProps) {
  const [run, setRun] = useState(false);
  const [stepIndex, setStepIndex] = useState(0);
  const isMountedRef = useRef(false);
  const hasStartedRef = useRef(false);

  // Track mount state for StrictMode
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  // Expose control for external reset
  useEffect(() => {
    tourControl = {
      start: () => {
        localStorage.removeItem(STORAGE_KEY);
        setStepIndex(0);
        setRun(true);
      },
    };
    return () => {
      tourControl = null;
    };
  }, []);

  // Auto-start on first visit (with delay for Tauri webview)
  useEffect(() => {
    if (!autoStart || hasStartedRef.current) return;

    const shouldStart = localStorage.getItem(STORAGE_KEY) !== 'true';
    if (!shouldStart) return;

    // Longer delay for Tauri webview initialization
    const timer = setTimeout(() => {
      if (isMountedRef.current && !hasStartedRef.current) {
        hasStartedRef.current = true;
        setRun(true);
      }
    }, 800);

    return () => clearTimeout(timer);
  }, [autoStart]);

  // Listen for manual start event
  useEffect(() => {
    const handleStart = () => {
      localStorage.removeItem(STORAGE_KEY);
      setStepIndex(0);
      hasStartedRef.current = true;
      setRun(true);
    };
    window.addEventListener('start-onboarding-tour', handleStart);
    return () =>
      window.removeEventListener('start-onboarding-tour', handleStart);
  }, []);

  // Joyride callback handler
  const handleCallback = useCallback(
    (data: CallBackProps) => {
      const { status, action, type, index } = data;

      // Tour finished or skipped
      if (status === STATUS.FINISHED || status === STATUS.SKIPPED) {
        setRun(false);
        localStorage.setItem(STORAGE_KEY, 'true');
        if (status === STATUS.FINISHED) {
          onComplete?.();
        } else {
          onSkip?.();
        }
        return;
      }

      // Handle step navigation
      if (type === 'step:after') {
        if (action === ACTIONS.NEXT) {
          setStepIndex(index + 1);
        } else if (action === ACTIONS.PREV) {
          setStepIndex(Math.max(0, index - 1));
        }
      }

      // Handle close button
      if (action === ACTIONS.CLOSE) {
        setRun(false);
        onSkip?.();
      }
    },
    [onComplete, onSkip],
  );

  return (
    <Joyride
      steps={TOUR_STEPS}
      run={run}
      stepIndex={stepIndex}
      continuous
      showProgress
      showSkipButton
      hideCloseButton={false}
      disableOverlayClose
      disableScrolling
      spotlightClicks={false}
      callback={handleCallback}
      floaterProps={{
        disableAnimation: true,
        styles: {
          floater: {
            filter: 'none',
          },
          arrow: {
            length: 12,
            spread: 16,
          },
        },
      }}
      styles={{
        options: {
          zIndex: 10000,
        },
        overlay: {
          backgroundColor: 'rgba(0, 0, 0, 0.75)',
        },
        tooltip: {
          borderRadius: 16,
          padding: 24,
          maxWidth: 420,
        },
        tooltipContainer: {
          textAlign: 'left',
        },
        tooltipContent: {
          padding: 0,
        },
        buttonNext: {
          borderRadius: 10,
          padding: '12px 24px',
          fontSize: 14,
          fontWeight: 600,
          border: 'none',
        },
        buttonBack: {
          marginRight: 10,
          padding: '10px 16px',
          borderRadius: 8,
          fontSize: 14,
          fontWeight: 500,
        },
        buttonSkip: {
          padding: '10px 16px',
          fontSize: 14,
        },
        buttonClose: {
          padding: 10,
          top: 12,
          right: 12,
          width: 28,
          height: 28,
          borderRadius: 8,
        },
        spotlight: {
          borderRadius: 12,
        },
      }}
      locale={{
        back: 'Previous',
        close: 'Dismiss',
        last: 'Complete Setup',
        next: 'Continue',
        skip: 'Skip',
      }}
    />
  );
}

// Helper to reset tour from settings
export const resetOnboardingTour = () => {
  if (tourControl) {
    tourControl.start();
  } else {
    window.dispatchEvent(new CustomEvent('start-onboarding-tour'));
  }
};

export const hasCompletedOnboarding = (): boolean => {
  return localStorage.getItem(STORAGE_KEY) === 'true';
};

export default OnboardingTour;
