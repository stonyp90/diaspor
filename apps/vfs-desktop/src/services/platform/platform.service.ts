/**
 * Platform Service
 *
 * Provides cross-platform information from the Tauri backend.
 * This service ensures all OS-level detection comes from Rust,
 * avoiding browser-based navigator.platform checks.
 */

import { invoke } from '@tauri-apps/api/core';

export interface PlatformInfo {
  platform: 'macos' | 'windows' | 'linux' | 'unknown';
  isMac: boolean;
  isWindows: boolean;
  isLinux: boolean;
  pathSeparator: string;
  modifierKey: string;
  altKey: string;
  shiftKey: string;
  deleteKey: string;
  theme: string;
}

// Cached platform info
let cachedPlatformInfo: PlatformInfo | null = null;
let platformInfoPromise: Promise<PlatformInfo> | null = null;

// Default fallback for browser-only mode or before Tauri is ready
const DEFAULT_PLATFORM_INFO: PlatformInfo = {
  platform: 'unknown',
  isMac: false,
  isWindows: false,
  isLinux: false,
  pathSeparator: '/',
  modifierKey: 'Ctrl',
  altKey: 'Alt',
  shiftKey: 'Shift',
  deleteKey: 'Backspace',
  theme: 'system',
};

/**
 * Check if Tauri is available
 */
function isTauriAvailable(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * Get platform info from Tauri backend
 * Caches the result for subsequent calls
 */
export async function getPlatformInfo(): Promise<PlatformInfo> {
  // Return cached value if available
  if (cachedPlatformInfo) {
    return cachedPlatformInfo;
  }

  // Return existing promise if fetch is in progress
  if (platformInfoPromise) {
    return platformInfoPromise;
  }

  // If not in Tauri, use browser-based detection as fallback
  if (!isTauriAvailable()) {
    const isMac =
      typeof navigator !== 'undefined' &&
      /Mac|iPod|iPhone|iPad/.test(navigator.platform);
    const isWindows =
      typeof navigator !== 'undefined' && /Win/.test(navigator.platform);
    const isLinux =
      typeof navigator !== 'undefined' && /Linux/.test(navigator.platform);

    cachedPlatformInfo = {
      platform: isMac
        ? 'macos'
        : isWindows
          ? 'windows'
          : isLinux
            ? 'linux'
            : 'unknown',
      isMac,
      isWindows,
      isLinux,
      pathSeparator: isWindows ? '\\' : '/',
      modifierKey: isMac ? '⌘' : 'Ctrl',
      altKey: isMac ? '⌥' : 'Alt',
      shiftKey: isMac ? '⇧' : 'Shift',
      deleteKey: isMac ? '⌫' : 'Backspace',
      theme: 'system',
    };
    return cachedPlatformInfo;
  }

  // Fetch from Tauri backend
  platformInfoPromise = invoke<PlatformInfo>('vfs_get_os_preferences')
    .then((info) => {
      cachedPlatformInfo = info;
      platformInfoPromise = null;
      return info;
    })
    .catch((error) => {
      console.error('Failed to get platform info from Tauri:', error);
      platformInfoPromise = null;
      // Fallback to default
      cachedPlatformInfo = DEFAULT_PLATFORM_INFO;
      return DEFAULT_PLATFORM_INFO;
    });

  return platformInfoPromise;
}

/**
 * Get platform info synchronously (returns cached value or default)
 * Use this for immediate UI rendering, but call getPlatformInfo() on init
 */
export function getPlatformInfoSync(): PlatformInfo {
  if (cachedPlatformInfo) {
    return cachedPlatformInfo;
  }

  // Browser-based fallback for initial render
  if (typeof navigator !== 'undefined') {
    const isMac = /Mac|iPod|iPhone|iPad/.test(navigator.platform);
    const isWindows = /Win/.test(navigator.platform);
    const isLinux = /Linux/.test(navigator.platform);

    return {
      platform: isMac
        ? 'macos'
        : isWindows
          ? 'windows'
          : isLinux
            ? 'linux'
            : 'unknown',
      isMac,
      isWindows,
      isLinux,
      pathSeparator: isWindows ? '\\' : '/',
      modifierKey: isMac ? '⌘' : 'Ctrl',
      altKey: isMac ? '⌥' : 'Alt',
      shiftKey: isMac ? '⇧' : 'Shift',
      deleteKey: isMac ? '⌫' : 'Backspace',
      theme: 'system',
    };
  }

  return DEFAULT_PLATFORM_INFO;
}

/**
 * Initialize platform info on app startup
 * Call this early in the app lifecycle
 */
export async function initPlatformInfo(): Promise<void> {
  await getPlatformInfo();
}

/**
 * Format a keyboard shortcut for display
 * Uses platform-appropriate symbols
 */
export function formatShortcutKey(
  key: string,
  modifiers: ('meta' | 'ctrl' | 'alt' | 'shift')[] = [],
): string {
  const platform = getPlatformInfoSync();
  const parts: string[] = [];

  if (modifiers.includes('meta')) {
    parts.push(platform.modifierKey);
  }
  if (modifiers.includes('ctrl') && !modifiers.includes('meta')) {
    parts.push('Ctrl');
  }
  if (modifiers.includes('alt')) {
    parts.push(platform.altKey);
  }
  if (modifiers.includes('shift')) {
    parts.push(platform.shiftKey);
  }

  // Format special keys
  const keyDisplay = formatKey(key, platform);
  parts.push(keyDisplay);

  return parts.join(platform.isMac ? '' : '+');
}

/**
 * Format individual key for display
 */
function formatKey(key: string, platform: PlatformInfo): string {
  const keyMap: Record<string, string> = {
    ArrowUp: '↑',
    ArrowDown: '↓',
    ArrowLeft: '←',
    ArrowRight: '→',
    Enter: '↵',
    Backspace: platform.deleteKey,
    Delete: platform.isMac ? '⌦' : 'Del',
    Escape: 'Esc',
    Tab: '⇥',
    ' ': 'Space',
  };

  return keyMap[key] || key.toUpperCase();
}

/**
 * Check if a keyboard event matches a shortcut
 * Handles cross-platform modifier key differences
 */
export function matchesShortcut(
  event: KeyboardEvent,
  key: string,
  modifiers: ('meta' | 'ctrl' | 'alt' | 'shift')[] = [],
): boolean {
  const platform = getPlatformInfoSync();

  // On macOS, meta is Cmd. On Windows/Linux, we treat meta as Ctrl
  const metaMatch = modifiers.includes('meta')
    ? platform.isMac
      ? event.metaKey
      : event.ctrlKey
    : true;

  const altMatch = modifiers.includes('alt') ? event.altKey : !event.altKey;
  const shiftMatch = modifiers.includes('shift')
    ? event.shiftKey
    : !event.shiftKey;

  // Check key (case-insensitive)
  const keyMatch =
    event.key.toLowerCase() === key.toLowerCase() || event.key === key;

  return metaMatch && altMatch && shiftMatch && keyMatch;
}

export default {
  getPlatformInfo,
  getPlatformInfoSync,
  initPlatformInfo,
  formatShortcutKey,
  matchesShortcut,
};
