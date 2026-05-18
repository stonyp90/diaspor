/**
 * TranscriptionModal - Modal for starting transcription with options
 * 
 * Allows users to:
 * - Select language for transcription
 * - Choose output location/path
 * - Start transcription process
 */

import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FileMetadata } from '../../types/storage';
import './TranscriptionModal.css';

interface TranscriptionModalProps {
  file: FileMetadata;
  sourceId?: string;
  onClose: () => void;
  onStart?: (operationId: string) => void;
}

const SUPPORTED_LANGUAGES = [
  { code: 'auto', name: 'Auto-detect' },
  { code: 'en', name: 'English' },
  { code: 'es', name: 'Spanish' },
  { code: 'fr', name: 'French' },
  { code: 'de', name: 'German' },
  { code: 'it', name: 'Italian' },
  { code: 'pt', name: 'Portuguese' },
  { code: 'ru', name: 'Russian' },
  { code: 'ja', name: 'Japanese' },
  { code: 'ko', name: 'Korean' },
  { code: 'zh', name: 'Chinese' },
  { code: 'ar', name: 'Arabic' },
  { code: 'hi', name: 'Hindi' },
  { code: 'nl', name: 'Dutch' },
  { code: 'pl', name: 'Polish' },
  { code: 'tr', name: 'Turkish' },
  { code: 'sv', name: 'Swedish' },
  { code: 'da', name: 'Danish' },
  { code: 'no', name: 'Norwegian' },
  { code: 'fi', name: 'Finnish' },
];

const OUTPUT_FORMATS = [
  { value: 'srt', label: 'SRT (SubRip)' },
  { value: 'vtt', label: 'VTT (WebVTT)' },
  { value: 'txt', label: 'TXT (Plain Text)' },
];

export const TranscriptionModal: React.FC<TranscriptionModalProps> = ({
  file,
  sourceId,
  onClose,
  onStart,
}) => {
  const [language, setLanguage] = useState('auto');
  const [outputFormat, setOutputFormat] = useState('srt');
  const [outputPath, setOutputPath] = useState('');
  const [isStarting, setIsStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isAvailable, setIsAvailable] = useState<boolean | null>(null);

  useEffect(() => {
    // Check if transcription is available
    const checkAvailability = async () => {
      try {
        const available = await invoke<boolean>('vfs_is_transcription_available');
        setIsAvailable(available);
      } catch (err) {
        console.error('Failed to check transcription availability:', err);
        setIsAvailable(false);
      }
    };

    checkAvailability();

    // Set default output path based on input file
    const defaultPath = file.path.replace(/\.[^/.]+$/, '') + '.srt';
    setOutputPath(defaultPath);
  }, [file.path]);

  const handleStart = async () => {
    if (!sourceId) {
      setError('Source ID is required');
      return;
    }

    setIsStarting(true);
    setError(null);

    try {
      // Start transcription
      const result = await invoke<{
        operation_id: string;
        segments: unknown[];
      }>('vfs_start_transcription', {
        sourceId,
        path: file.path,
        language: language === 'auto' ? null : language,
        outputPath: outputPath || null,
      });

      // If output path is specified, save transcription after it completes
      // Note: This is a simplified approach. In production, you might want to
      // wait for transcription to complete before saving, or handle it via events
      if (outputPath && outputFormat) {
        // The save will happen later when transcription completes
        // For now, we just start the transcription
      }

      if (onStart) {
        onStart(result.operation_id);
      }

      onClose();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(`Failed to start transcription: ${errorMessage}`);
      setIsStarting(false);
    }
  };

  const handleSavePath = async () => {
    // In a real implementation, you might want to use Tauri's dialog API
    // to let users select a save location
    // For now, we'll just use the default path
  };

  if (isAvailable === false) {
    return (
      <div className="transcription-modal-overlay" onClick={onClose}>
        <div
          className="transcription-modal"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="transcription-modal-header">
            <h2>Transcription Not Available</h2>
            <button className="transcription-modal-close" onClick={onClose}>
              ×
            </button>
          </div>
          <div className="transcription-modal-content">
            <p>
              Transcription requires FFmpeg to be installed. Please install
              FFmpeg and try again.
            </p>
            <div className="transcription-modal-actions">
              <button onClick={onClose}>Close</button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="transcription-modal-overlay" onClick={onClose}>
      <div
        className="transcription-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="transcription-modal-header">
          <h2>Transcribe {file.name}</h2>
          <button className="transcription-modal-close" onClick={onClose}>
            ×
          </button>
        </div>

        <div className="transcription-modal-content">
          {error && (
            <div className="transcription-modal-error">{error}</div>
          )}

          <div className="transcription-modal-field">
            <label htmlFor="language">Language</label>
            <select
              id="language"
              value={language}
              onChange={(e) => setLanguage(e.target.value)}
              disabled={isStarting}
            >
              {SUPPORTED_LANGUAGES.map((lang) => (
                <option key={lang.code} value={lang.code}>
                  {lang.name}
                </option>
              ))}
            </select>
            <span className="field-hint">
              Select the language spoken in the audio/video
            </span>
          </div>

          <div className="transcription-modal-field">
            <label htmlFor="output-format">Output Format</label>
            <select
              id="output-format"
              value={outputFormat}
              onChange={(e) => setOutputFormat(e.target.value)}
              disabled={isStarting}
            >
              {OUTPUT_FORMATS.map((format) => (
                <option key={format.value} value={format.value}>
                  {format.label}
                </option>
              ))}
            </select>
            <span className="field-hint">
              Choose the subtitle/transcription file format
            </span>
          </div>

          <div className="transcription-modal-field">
            <label htmlFor="output-path">Output Path</label>
            <div className="output-path-input-group">
              <input
                id="output-path"
                type="text"
                value={outputPath}
                onChange={(e) => setOutputPath(e.target.value)}
                placeholder="Path where transcription will be saved"
                disabled={isStarting}
              />
              <button
                type="button"
                onClick={handleSavePath}
                disabled={isStarting}
                title="Choose save location"
              >
                📁
              </button>
            </div>
            <span className="field-hint">
              Location where the transcription file will be saved
            </span>
          </div>

          <div className="transcription-modal-info">
            <p>
              <strong>Note:</strong> Transcription will run in the background.
              You can monitor progress in the Operations panel.
            </p>
          </div>
        </div>

        <div className="transcription-modal-actions">
          <button
            className="transcription-modal-cancel"
            onClick={onClose}
            disabled={isStarting}
          >
            Cancel
          </button>
          <button
            className="transcription-modal-start"
            onClick={handleStart}
            disabled={isStarting || !sourceId}
          >
            {isStarting ? 'Starting...' : 'Start Transcription'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default TranscriptionModal;
