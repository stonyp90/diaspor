/**
 * TranscriptionProgressPanel Component
 *
 * Panel that displays all active transcription operations
 */
import React, { useEffect, useState, startTransition } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TranscriptionProgress } from './TranscriptionProgress';
import '../OperationsPanel/OperationsPanel.css';

interface TranscriptionState {
  operation_id: string;
  source_id: string;
  source_path: string;
  file_size?: number;
  bytes_processed: number;
  status: string;
}

export const TranscriptionProgressPanel: React.FC = () => {
  const [transcriptions, setTranscriptions] = useState<TranscriptionState[]>(
    [],
  );
  // Track visible transcriptions (including completed ones until manually closed)
  const [visibleTranscriptions, setVisibleTranscriptions] = useState<
    Set<string>
  >(new Set());
  // Track dismissed transcriptions to prevent them from being re-added
  const [, setDismissedTranscriptions] = useState<Set<string>>(new Set());

  const loadTranscriptions = React.useCallback(async () => {
    try {
      const transcriptionList = await invoke<TranscriptionState[]>(
        'vfs_list_transcriptions',
      );

      // Use functional updates to access current state without including in dependencies
      setDismissedTranscriptions((dismissed) => {
        setVisibleTranscriptions((visible) => {
          // Filter out dismissed transcriptions
          const filteredTranscriptionList = transcriptionList.filter(
            (t) => !dismissed.has(t.operation_id),
          );

          // Add any new transcriptions to visible set (auto-show when transcription starts)
          // But exclude transcriptions that have been dismissed
          const newTranscriptions = filteredTranscriptionList
            .filter(
              (t) =>
                !visible.has(t.operation_id) && !dismissed.has(t.operation_id),
            )
            .map((t) => t.operation_id);

          const nextVisible =
            newTranscriptions.length > 0
              ? new Set([...visible, ...newTranscriptions])
              : visible;

          // Keep completed/failed transcriptions in visible set - user must manually close them
          // Only remove transcriptions that are no longer in the list (canceled/deleted)
          const currentTranscriptionIds = new Set(
            filteredTranscriptionList.map((t) => t.operation_id),
          );
          const finalVisible = new Set(nextVisible);
          // Remove transcriptions that no longer exist
          for (const id of nextVisible) {
            if (!currentTranscriptionIds.has(id)) {
              finalVisible.delete(id);
            }
          }

          // Use React's startTransition to batch state updates
          startTransition(() => {
            setTranscriptions(filteredTranscriptionList);
          });

          return finalVisible;
        });
        return dismissed; // Return unchanged dismissed state
      });
    } catch (err) {
      console.error('Failed to load transcriptions:', err);
    }
  }, []); // Empty deps - using functional updates

  useEffect(() => {
    // Load initial transcriptions
    loadTranscriptions();

    // Poll for updates more frequently for better progress updates
    const interval = setInterval(loadTranscriptions, 500);
    return () => clearInterval(interval);
  }, [loadTranscriptions]);

  const handleTranscriptionComplete = (operationId: string) => {
    // Don't remove from visible - keep showing completed transcriptions until user closes
    // Just reload to update the status
    console.log('[TranscriptionProgress] Transcription complete:', operationId);
    setTimeout(loadTranscriptions, 500);
  };

  const handleTranscriptionCancel = async (operationId: string) => {
    try {
      // Remove from visible when cancelled
      setVisibleTranscriptions((prev) => {
        const next = new Set(prev);
        next.delete(operationId);
        return next;
      });
      // Mark as dismissed
      setDismissedTranscriptions((prev) => {
        const next = new Set(prev);
        next.add(operationId);
        return next;
      });
    } catch (err) {
      console.error('Failed to cancel transcription:', err);
    }
  };

  const handleCloseTranscription = (operationId: string) => {
    // Remove from visible
    setVisibleTranscriptions((prev) => {
      const next = new Set(prev);
      next.delete(operationId);
      return next;
    });
    // Mark as dismissed
    setDismissedTranscriptions((prev) => {
      const next = new Set(prev);
      next.add(operationId);
      return next;
    });
  };

  // Get visible transcriptions (filter by visible set)
  const visibleTranscriptionList = transcriptions.filter((t) =>
    visibleTranscriptions.has(t.operation_id),
  );

  if (visibleTranscriptionList.length === 0) {
    return null;
  }

  return (
    <div className="upload-progress-panel">
      <div className="upload-progress-panel-header">
        <h3>Transcriptions</h3>
      </div>
      <div className="upload-progress-panel-body">
        {visibleTranscriptionList.map((transcription) => {
          const fileName =
            transcription.source_path.split('/').pop() || 'Unknown file';
          return (
            <TranscriptionProgress
              key={transcription.operation_id}
              operationId={transcription.operation_id}
              fileName={fileName}
              onComplete={() =>
                handleTranscriptionComplete(transcription.operation_id)
              }
              onCancel={() =>
                handleTranscriptionCancel(transcription.operation_id)
              }
              onClose={() =>
                handleCloseTranscription(transcription.operation_id)
              }
            />
          );
        })}
      </div>
    </div>
  );
};
