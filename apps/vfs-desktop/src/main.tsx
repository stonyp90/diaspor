import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { ErrorBoundary } from './components/ErrorBoundary/ErrorBoundary';
import './styles/theme-variables.css';
import './styles/index.css';
import './styles/finder.css';

// Keys to preserve across app restarts
const PRESERVED_KEYS = [
  'ursly-onboarding-completed',
  'ursly-theme',
  'ursly-sidebar-width',
  'ursly-column-widths',
];

// Clear storage for a clean start, preserving important user preferences
async function clearAllStorage() {
  try {
    // Save preserved values
    const preserved: Record<string, string | null> = {};
    for (const key of PRESERVED_KEYS) {
      preserved[key] = localStorage.getItem(key);
    }

    // Clear frontend storage
    localStorage.clear();
    sessionStorage.clear();

    // Restore preserved values
    for (const key of PRESERVED_KEYS) {
      const value = preserved[key];
      if (value !== null) {
        localStorage.setItem(key, value);
      }
    }

    console.log('[App] Cleared storage (preserved user preferences)');

    // Clear backend operations (if Tauri is available)
    if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('vfs_clear_all_operations');
        console.log('[App] Cleared all backend operations');
      } catch (err) {
        console.warn(
          '[App] Failed to clear backend operations (may not be initialized yet):',
          err,
        );
      }
    }
  } catch (err) {
    console.warn('[App] Failed to clear storage:', err);
  }
}

// Execute cleanup on startup
clearAllStorage();

const rootElement = document.getElementById('root');
if (!rootElement) {
  throw new Error('Root element not found');
}

// Global error handler for unhandled errors
window.addEventListener('error', (event) => {
  console.error('Global error:', event.error);
});

window.addEventListener('unhandledrejection', (event) => {
  console.error('Unhandled promise rejection:', event.reason);
});

ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
