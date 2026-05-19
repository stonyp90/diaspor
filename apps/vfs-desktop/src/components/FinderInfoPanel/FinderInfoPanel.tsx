/**
 * FinderInfoPanel Component
 *
 * Info panel that displays file metadata and actions.
 */

import React from 'react';
import type { FileMetadata } from '../../types/storage';
import {
  getFileIcon,
  formatDate,
  formatSize,
} from '../../pages/FinderPage/utils';
import './FinderInfoPanel.css';

export interface FinderInfoPanelProps {
  selectedFile: FileMetadata | null;
  files: FileMetadata[];
  isMountedStorage: () => boolean;
  onWarm: (file: FileMetadata) => Promise<void>;
  onTranscode: (file: FileMetadata) => Promise<void>;
  onTranscribe: (file: FileMetadata) => Promise<void>;
}

export function FinderInfoPanel({
  selectedFile,
  files,
  isMountedStorage,
  onWarm,
  onTranscode,
  onTranscribe,
}: FinderInfoPanelProps) {
  if (!selectedFile) {
    return (
      <aside className="finder-info">
        <div className="info-empty">
          <div className="info-empty-icon">
            <svg
              width="48"
              height="48"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1"
            >
              <circle cx="12" cy="12" r="10" />
              <path d="M12 16v-4" />
              <path d="M12 8h.01" />
            </svg>
          </div>
          <p>No selection</p>
          <p className="info-hint">{files.length} items</p>
        </div>
      </aside>
    );
  }

  return (
    <aside className="finder-info">
      <div className="info-preview">
        {selectedFile.thumbnail ? (
          <img src={selectedFile.thumbnail} alt="" />
        ) : (
          <span className="info-icon">{getFileIcon(selectedFile, 64)}</span>
        )}
      </div>
      <h3 className="info-name">{selectedFile.name}</h3>
      <div className="info-meta">
        {/* Basic Info */}
        <div className="meta-section">
          <div className="meta-section-title">General</div>
          <div className="meta-row">
            <span className="meta-label">Size</span>
            <span className="meta-value">{formatSize(selectedFile.size)}</span>
          </div>
          <div className="meta-row">
            <span className="meta-label">Modified</span>
            <span className="meta-value">
              {formatDate(selectedFile.lastModified) !== '-'
                ? new Date(selectedFile.lastModified).toLocaleString()
                : '-'}
            </span>
          </div>
          {selectedFile.createdAt && (
            <div className="meta-row">
              <span className="meta-label">Created</span>
              <span className="meta-value">
                {formatDate(selectedFile.createdAt) !== '-'
                  ? new Date(selectedFile.createdAt).toLocaleString()
                  : '-'}
              </span>
            </div>
          )}
          {selectedFile.container && (
            <div className="meta-row">
              <span className="meta-label">Container</span>
              <span className="meta-value">
                {selectedFile.container.toUpperCase()}
              </span>
            </div>
          )}
        </div>

        {/* Video Info */}
        {(selectedFile.videoCodec || selectedFile.width) && (
          <div className="meta-section">
            <div className="meta-section-title">Video</div>
            {selectedFile.width && selectedFile.height && (
              <div className="meta-row">
                <span className="meta-label">Resolution</span>
                <span className="meta-value">
                  {selectedFile.width} x {selectedFile.height}
                </span>
              </div>
            )}
            {selectedFile.videoCodec && (
              <div className="meta-row">
                <span className="meta-label">Codec</span>
                <span className="meta-value">
                  {selectedFile.videoCodec.toUpperCase()}
                </span>
              </div>
            )}
            {selectedFile.frameRate && (
              <div className="meta-row">
                <span className="meta-label">Frame Rate</span>
                <span className="meta-value">{selectedFile.frameRate} fps</span>
              </div>
            )}
            {selectedFile.videoBitrate && (
              <div className="meta-row">
                <span className="meta-label">Bitrate</span>
                <span className="meta-value">
                  {selectedFile.videoBitrate} kbps
                </span>
              </div>
            )}
            {selectedFile.colorSpace && (
              <div className="meta-row">
                <span className="meta-label">Color</span>
                <span className="meta-value">{selectedFile.colorSpace}</span>
              </div>
            )}
            {selectedFile.hdrFormat && (
              <div className="meta-row">
                <span className="meta-label">HDR</span>
                <span className="meta-value highlight">
                  {selectedFile.hdrFormat.toUpperCase()}
                </span>
              </div>
            )}
          </div>
        )}

        {/* Audio Info */}
        {(selectedFile.audioCodec || selectedFile.audioChannels) && (
          <div className="meta-section">
            <div className="meta-section-title">Audio</div>
            {selectedFile.audioCodec && (
              <div className="meta-row">
                <span className="meta-label">Codec</span>
                <span className="meta-value">
                  {selectedFile.audioCodec.toUpperCase()}
                </span>
              </div>
            )}
            {selectedFile.audioChannels && (
              <div className="meta-row">
                <span className="meta-label">Channels</span>
                <span className="meta-value">
                  {selectedFile.audioChannels === 1
                    ? 'Mono'
                    : selectedFile.audioChannels === 2
                      ? 'Stereo'
                      : selectedFile.audioChannels === 6
                        ? '5.1 Surround'
                        : selectedFile.audioChannels === 8
                          ? '7.1 Surround'
                          : `${selectedFile.audioChannels} ch`}
                </span>
              </div>
            )}
            {selectedFile.audioSampleRate && (
              <div className="meta-row">
                <span className="meta-label">Sample Rate</span>
                <span className="meta-value">
                  {selectedFile.audioSampleRate / 1000} kHz
                </span>
              </div>
            )}
            {selectedFile.audioBitrate && (
              <div className="meta-row">
                <span className="meta-label">Bitrate</span>
                <span className="meta-value">
                  {selectedFile.audioBitrate} kbps
                </span>
              </div>
            )}
          </div>
        )}

        {/* Duration */}
        {selectedFile.duration && (
          <div className="meta-section">
            <div className="meta-section-title">Duration</div>
            <div className="meta-row">
              <span className="meta-label">Length</span>
              <span className="meta-value highlight">
                {Math.floor(selectedFile.duration / 3600) > 0
                  ? `${Math.floor(selectedFile.duration / 3600)}h ${Math.floor((selectedFile.duration % 3600) / 60)}m ${Math.floor(selectedFile.duration % 60)}s`
                  : `${Math.floor(selectedFile.duration / 60)}m ${Math.floor(selectedFile.duration % 60)}s`}
              </span>
            </div>
          </div>
        )}

        {/* Tags */}
        {selectedFile.tags && selectedFile.tags.length > 0 && (
          <div className="meta-section">
            <div className="meta-section-title">Tags</div>
            <div className="meta-tags">
              {selectedFile.tags.map((tag, i) => {
                const tagObj =
                  typeof tag === 'string'
                    ? { name: tag, color: '#6b7280' }
                    : tag;
                return (
                  <span
                    key={i}
                    className="meta-tag"
                    style={
                      {
                        '--tag-color': tagObj.color || '#6b7280',
                      } as React.CSSProperties
                    }
                  >
                    {tagObj.name}
                  </span>
                );
              })}
            </div>
          </div>
        )}
      </div>
      <div className="info-actions">
        {/* Warm to Hot - only for cloud/remote storage */}
        {!isMountedStorage() &&
          selectedFile.canWarm &&
          !selectedFile.isWarmed && (
            <button
              className="action-btn warm"
              onClick={() => onWarm(selectedFile)}
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="currentColor"
              >
                <path d="M12 23a7.5 7.5 0 0 1-5.138-12.963C8.204 8.774 11.5 6.5 11 1.5c0 0 6.5 3.5 6.5 9a5.5 5.5 0 0 1-3 4.9v.1a5 5 0 0 0 5 5c0 3.866-3.134 2.5-7.5 2.5z" />
              </svg>
              Warm to Hot
            </button>
          )}
        {/* Transcode - only for cloud/remote storage */}
        {!isMountedStorage() && selectedFile.canTranscode && (
          <button
            className="action-btn secondary"
            onClick={() => onTranscode(selectedFile)}
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <polygon points="5 3 19 12 5 21 5 3" />
            </svg>
            Transcode
          </button>
        )}
        {/* Transcribe - for video/audio files */}
        {(selectedFile.mimeType?.startsWith('video/') ||
          selectedFile.mimeType?.startsWith('audio/')) &&
          !selectedFile.isDirectory && (
            <button
              className="action-btn secondary"
              onClick={() => onTranscribe(selectedFile)}
            >
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
                <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
                <line x1="12" y1="18" x2="12" y2="22" />
                <line x1="8" y1="22" x2="16" y2="22" />
              </svg>
              Transcribe
            </button>
          )}
      </div>
    </aside>
  );
}
