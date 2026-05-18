/**
 * Standard Operation Event Constants
 *
 * Centralized list of all operation events used across the application.
 * This ensures consistency and makes it easy to add new operation types.
 */

// Standard operation event names
export const OPERATION_EVENTS = [
  'upload-started',
  'download-started',
  'delete-started',
  'move-started',
  'copy-started',
  'paste-started',
  'rename-started',
  'mkdir-started',
  'rmdir-started',
  'tier-change-started',
  'transcribe-started',
  'transcode-started',
  'auto-tag-started',
] as const;

export type OperationEventType = (typeof OPERATION_EVENTS)[number];

// Standard polling intervals
export const POLLING_INTERVALS = {
  FAST: 500, // For individual progress components
  NORMAL: 1000, // For main panels
  SLOW: 2000, // For history/widgets
} as const;
