/**
 * useOperationTracking Hook
 *
 * Standardized hook for tracking file operations across all components.
 * Provides consistent event listening and polling behavior.
 */

import { useEffect } from 'react';
import { OPERATION_EVENTS, POLLING_INTERVALS } from '../utils/operationEvents';

export interface UseOperationTrackingOptions {
  /** Callback to execute when an operation starts */
  onOperationStarted: () => void;
  /** Polling interval in milliseconds (default: NORMAL = 1000ms) */
  pollingInterval?: number;
  /** Whether tracking is enabled (default: true) */
  enabled?: boolean;
  /** Whether to refresh immediately when event fires (default: true) */
  immediateRefresh?: boolean;
  /** Optional delayed refreshes in milliseconds for eventual consistency (default: [200, 500]) */
  delayedRefreshes?: number[];
  /** Optional: filter to specific event types (default: all events) */
  eventFilter?: string[];
}

/**
 * Hook for tracking file operations with standardized event listening and polling
 *
 * @example
 * ```tsx
 * useOperationTracking({
 *   onOperationStarted: loadOperations,
 *   pollingInterval: POLLING_INTERVALS.NORMAL,
 *   enabled: isVisible,
 * });
 * ```
 */
export function useOperationTracking({
  onOperationStarted,
  pollingInterval = POLLING_INTERVALS.NORMAL,
  enabled = true,
  immediateRefresh = true,
  delayedRefreshes = [200, 500],
  eventFilter,
}: UseOperationTrackingOptions) {
  useEffect(() => {
    if (!enabled) return;

    const handleOperationStarted = () => {
      if (immediateRefresh) {
        onOperationStarted();
      }
      // Optional delayed refreshes for eventual consistency
      if (delayedRefreshes && delayedRefreshes.length > 0) {
        delayedRefreshes.forEach((delay) => {
          setTimeout(() => onOperationStarted(), delay);
        });
      }
    };

    // Determine which events to listen to
    const eventsToListen = eventFilter || OPERATION_EVENTS;

    // Register all event listeners
    eventsToListen.forEach((event) => {
      window.addEventListener(event, handleOperationStarted);
    });

    // Set up polling
    const interval = setInterval(onOperationStarted, pollingInterval);

    return () => {
      clearInterval(interval);
      // Clean up all event listeners
      eventsToListen.forEach((event) => {
        window.removeEventListener(event, handleOperationStarted);
      });
    };
  }, [
    enabled,
    pollingInterval,
    onOperationStarted,
    immediateRefresh,
    delayedRefreshes,
    eventFilter,
  ]);
}
