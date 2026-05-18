/**
 * Shared Operation Types
 *
 * Centralized type definitions for operations used across all components.
 * This ensures consistency and makes it easy to add new operation types.
 */

export type OperationType =
  | 'Upload'
  | 'Download'
  | 'Delete'
  | 'Move'
  | 'Copy'
  | 'Paste'
  | 'Rename'
  | 'CreateDir'
  | 'RemoveDir'
  | 'TierChange'
  | 'Transcribe'
  | 'Transcode'
  | 'AutoTag';

export type OperationStatus =
  | 'Pending'
  | 'InProgress'
  | 'Completed'
  | 'Failed'
  | 'Canceled'
  | 'Paused';
