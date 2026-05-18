/**
 * VideoPlayer Component
 * 
 * Modal video player with blob URL support for secure playback
 */

import React, { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DialogService } from '../../services/dialog';
import './VideoPlayer.css';

interface VideoPlayerProps {
  fileName: string;
  sourceId: string;
  filePath: string;
  sizeHuman?: string;
  onClose: () => void;
}

export const VideoPlayer: React.FC<VideoPlayerProps> = ({
  fileName,
  sourceId,
  filePath,
  sizeHuman,
  onClose,
}) => {
  const [loading, setLoading] = React.useState(true);
  const [blobUrl, setBlobUrl] = React.useState<string | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    let url: string | null = null;

    const loadVideo = async () => {
      try {
        // Read file bytes
        const fileData = await invoke<number[]>('vfs_read_file_bytes', {
          sourceId,
          path: filePath,
        });

        if (!mounted) return;

        // Convert to blob URL
        const blob = new Blob([new Uint8Array(fileData)], { type: 'video/mp4' });
        url = URL.createObjectURL(blob);
        setBlobUrl(url);
        setLoading(false);
      } catch (err) {
        if (!mounted) return;
        const errorMsg = err instanceof Error ? err.message : String(err);
        console.error('Failed to load video:', errorMsg);
        setError(errorMsg);
        setLoading(false);
        DialogService.error(`Failed to play video: ${errorMsg}`);
      }
    };

    loadVideo();

    return () => {
      mounted = false;
      if (url) {
        URL.revokeObjectURL(url);
      }
    };
  }, [sourceId, filePath]);

  const handleOverlayClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  };

  if (error) {
    return null;
  }

  return (
    <div className="video-player-overlay" onClick={handleOverlayClick}>
      {loading ? (
        <div className="video-player-loading">
          <div className="video-player-loading-icon">⏳</div>
          <div className="video-player-loading-title">Loading video...</div>
          <div className="video-player-loading-subtitle">Preparing video stream</div>
        </div>
      ) : (
        <div className="video-player-container">
          <div className="video-player-header">
            <h3 className="video-player-title">{fileName}</h3>
            <button
              className="video-player-close-btn"
              onClick={onClose}
              title="Close"
            >
              ✕
            </button>
          </div>

          {blobUrl && (
            <video
              className="video-player-video"
              src={blobUrl}
              controls
              autoPlay
            />
          )}

          <div className="video-player-info">
            ✓ Full playback controls • {sizeHuman || 'Unknown size'}
          </div>
        </div>
      )}
    </div>
  );
};

export default VideoPlayer;
