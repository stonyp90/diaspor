/**
 * FinderContextMenu Component
 *
 * Context menu for file and folder operations in the Finder view.
 */

import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DialogService } from '../../services/dialog';
import { getPlatformInfoSync } from '../../services/platform';
import type { FileMetadata, StorageSource } from '../../types/storage';
import {
  isObjectStorage,
  supportsFilesystemOperations,
} from '../../pages/FinderPage/utils';
import { VideoPlayer } from '../VideoPlayer';
import './FinderContextMenu.css';

export interface FinderContextMenuProps {
  visible: boolean;
  x: number;
  y: number;
  targetFile?: FileMetadata;
  selectedSource: StorageSource | null;
  selectedFiles: Set<string>;
  showOpenWith: boolean;
  appsLoading: boolean;
  availableApps: Array<{ name: string; path: string }>;
  isMountedStorage: () => boolean;
  onClose: () => void;
  onSetShowOpenWith: (show: boolean) => void;
  onSetTierDialogPaths: (paths: string[]) => void;
  onSetShowTierDialog: (show: boolean) => void;
  onSetInfoModal: (modal: { visible: boolean; file: FileMetadata }) => void;
  onNavigateTo: (path: string) => void;
  onHandleOpenFile: (file: FileMetadata) => Promise<void>;
  onHandleOpenFileWith: (file: FileMetadata, appPath: string) => Promise<void>;
  onHandleDownloadFile: (file: FileMetadata) => Promise<void>;
  onHandleDelete: () => Promise<void>;
  onHandleCopy: () => Promise<void>;
  onHandleCut: () => Promise<void>;
  onHandlePaste: (targetPath?: string) => Promise<void>;
  onHandleRename: (file: FileMetadata) => void;
  onHandleNewFolder: (targetPath?: string) => Promise<void>;
  onHandleTranscribe: (file: FileMetadata) => Promise<void>;
  onHandleAutoTag?: (file: FileMetadata) => Promise<void>;
  onHandleUpload: () => Promise<void>;
  onLoadAppsForFile: (file: FileMetadata) => Promise<void>;
  onLoadFilesList?: (
    sourceId: string,
    path: string,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    providedSource?: any,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    providedSources?: any[],
  ) => Promise<void>;
  currentPath?: string;
  onOpenSettings?: () => void;
  aiModelsAvailable: boolean;
}

export function FinderContextMenu({
  visible,
  x,
  y,
  targetFile,
  selectedSource,
  selectedFiles,
  showOpenWith,
  appsLoading,
  availableApps,
  isMountedStorage,
  onClose,
  onSetShowOpenWith,
  onSetTierDialogPaths,
  onSetShowTierDialog,
  onSetInfoModal,
  onNavigateTo,
  onHandleOpenFile,
  onHandleOpenFileWith,
  onHandleDownloadFile,
  onHandleDelete,
  onHandleCopy,
  onHandleCut,
  onHandlePaste,
  onHandleRename,
  onHandleNewFolder,
  onHandleTranscribe,
  onHandleAutoTag,
  onHandleUpload,
  onLoadAppsForFile,
  onOpenSettings,
  aiModelsAvailable,
}: FinderContextMenuProps) {
  const [showVideoPlayer, setShowVideoPlayer] = useState(false);
  const [videoPlayerProps, setVideoPlayerProps] = useState<{
    fileName: string;
    sourceId: string;
    filePath: string;
    sizeHuman?: string;
  } | null>(null);

  if (!visible) return null;

  const isObjStorage = isObjectStorage(selectedSource);

  return (
    <div
      className="context-menu"
      style={{
        position: 'fixed',
        top: y,
        left: x,
        zIndex: 10000,
      }}
      onClick={(e) => {
        e.stopPropagation();
        console.log('[Context Menu] Menu clicked');
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
      }}
    >
      {/* Object storage - full feature set */}
      {isObjStorage && targetFile && (
        <>
          {/* Open action for folders */}
          {targetFile.isDirectory && (
            <button
              className="context-item"
              onClick={() => {
                if (targetFile) {
                  onNavigateTo(targetFile.path);
                }
                onClose();
              }}
            >
              <svg
                className="context-icon"
                viewBox="0 0 16 16"
                fill="currentColor"
              >
                <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h3.172a1.5 1.5 0 0 1 1.06.44l.708.706a.5.5 0 0 0 .354.147H13.5A1.5 1.5 0 0 1 15 4.793v7.707A1.5 1.5 0 0 1 13.5 14h-11A1.5 1.5 0 0 1 1 12.5v-9z" />
              </svg>
              Open
            </button>
          )}

          {/* Play - for video files */}
          {!targetFile.isDirectory && (() => {
            const ext = targetFile.name.split('.').pop()?.toLowerCase() || '';
            const isVideo = ['mp4', 'mov', 'avi', 'mkv', 'webm', 'm4v', 'mpeg', 'mpg'].includes(ext);
            return isVideo ? (
              <button
                className="context-item"
                onClick={() => {
                  if (selectedSource) {
                    setVideoPlayerProps({
                      fileName: targetFile.name,
                      sourceId: selectedSource.id,
                      filePath: targetFile.path,
                      sizeHuman: targetFile.size_human,
                    });
                    setShowVideoPlayer(true);
                  }
                  onClose();
                }}
              >
                <svg
                  className="context-icon"
                  viewBox="0 0 16 16"
                  fill="currentColor"
                >
                  <path d="M5 3l8 5-8 5V3z" />
                </svg>
                Play Video
              </button>
            ) : null;
          })()}


          {/* Download - for files */}
          {!targetFile.isDirectory && (
            <button
              className="context-item"
              onClick={() => {
                if (targetFile) {
                  onHandleDownloadFile(targetFile);
                }
                onClose();
              }}
            >
              <svg
                className="context-icon"
                viewBox="0 0 16 16"
                fill="currentColor"
              >
                <path d="M.5 9.9a.5.5 0 0 1 .5.5v2.5a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-2.5a.5.5 0 0 1 1 0v2.5a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2v-2.5a.5.5 0 0 1 .5-.5z" />
                <path d="M7.646 11.854a.5.5 0 0 0 .708 0l3-3a.5.5 0 0 0-.708-.708L8.5 10.293V1.5a.5.5 0 0 0-1 0v8.793L5.354 8.146a.5.5 0 1 0-.708.708l3 3z" />
              </svg>
              Download
            </button>
          )}

          {/* Asset Details */}
          <button
            className="context-item"
            onClick={() => {
              if (targetFile) {
                onSetInfoModal({ visible: true, file: targetFile });
              }
              onClose();
            }}
          >
            <svg
              className="context-icon"
              viewBox="0 0 16 16"
              fill="currentColor"
            >
              <path d="M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14zm0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16z" />
              <path d="m8.93 6.588-2.29.287-.082.38.45.083c.294.07.352.176.288.469l-.738 3.468c-.194.897.105 1.319.808 1.319.545 0 1.178-.252 1.465-.598l.088-.416c-.2.176-.492.246-.686.246-.275 0-.375-.193-.304-.533L8.93 6.588zM9 4.5a1 1 0 1 1-2 0 1 1 0 0 1 2 0z" />
            </svg>
            Asset Details
            <span className="context-shortcut">⌘I</span>
          </button>

          <div className="context-divider" />

          {/* Clipboard actions - Copy, Cut, Paste */}
          {(selectedFiles.size > 0 || targetFile) && (
            <>
              {/* Copy */}
              <button
                className="context-item"
                onClick={async (e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  await onHandleCopy();
                  onClose();
                }}
              >
                <svg
                  className="context-icon"
                  viewBox="0 0 16 16"
                  fill="currentColor"
                >
                  <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z" />
                  <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z" />
                </svg>
                Copy
                <span className="context-shortcut">⌘C</span>
              </button>

              {/* Cut */}
              <button
                className="context-item"
                onClick={async (e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  await onHandleCut();
                  onClose();
                }}
              >
                <svg
                  className="context-icon"
                  viewBox="0 0 16 16"
                  fill="currentColor"
                >
                  <path d="M9.5 0a.5.5 0 0 0-.5.5v3a.5.5 0 0 1-.5.5h-3a.5.5 0 0 0 0 1h3a.5.5 0 0 1 .5.5v3a.5.5 0 0 0 1 0v-3A1.5 1.5 0 0 0 9.5 4h-3A1.5 1.5 0 0 0 5 5.5v3a.5.5 0 0 0 1 0v-3a.5.5 0 0 1 .5-.5h3a.5.5 0 0 0 .5-.5v-3A.5.5 0 0 0 9.5 0z" />
                  <path d="M13.354.646a1.207 1.207 0 0 0-1.708 0l-10 10A1 1 0 0 0 2 12h3v2a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2h3a1 1 0 0 0 .708-1.646l-10-10zM9 14v-2H7v2h2zm1 0v-2h2l-2 2z" />
                </svg>
                Cut
                <span className="context-shortcut">⌘X</span>
              </button>

              {/* Paste */}
              <button
                className="context-item"
                onClick={async (e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  const targetPath =
                    targetFile &&
                    (targetFile.mimeType === 'folder' || targetFile.isDirectory)
                      ? targetFile.path
                      : undefined;
                  await onHandlePaste(targetPath);
                  onClose();
                }}
              >
                <svg
                  className="context-icon"
                  viewBox="0 0 16 16"
                  fill="currentColor"
                >
                  <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z" />
                  <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z" />
                </svg>
                Paste
                <span className="context-shortcut">⌘V</span>
              </button>

              {/* Rename - only for single files (folders can't be renamed on object storage) */}
              {!targetFile.isDirectory && selectedFiles.size <= 1 && (
                <button
                  className="context-item"
                  onClick={() => {
                    if (targetFile) onHandleRename(targetFile);
                    onClose();
                  }}
                >
                  <svg
                    className="context-icon"
                    viewBox="0 0 16 16"
                    fill="currentColor"
                  >
                    <path d="M12.146.146a.5.5 0 0 1 .708 0l3 3a.5.5 0 0 1 0 .708l-10 10a.5.5 0 0 1-.168.11l-5 2a.5.5 0 0 1-.65-.65l2-5a.5.5 0 0 1 .11-.168l10-10zM11.207 2.5 13.5 4.793 14.793 3.5 12.5 1.207 11.207 2.5zm1.586 3L10.5 3.207 4 9.707V10h.5a.5.5 0 0 1 .5.5v.5h.5a.5.5 0 0 1 .5.5v.5h.293l6.5-6.5zm-9.761 5.175-.106.106-1.528 3.821 3.821-1.528.106-.106A.5.5 0 0 1 5 12.5V12h-.5a.5.5 0 0 1-.5-.5V11h-.5a.5.5 0 0 1-.468-.325z" />
                  </svg>
                  Rename
                </button>
              )}

              <div className="context-divider" />
            </>
          )}

          {/* Move to Storage Tier */}
          {targetFile && (
            <button
              className="context-item"
              onClick={() => {
                const paths =
                  selectedFiles.size > 0
                    ? Array.from(selectedFiles)
                    : targetFile
                      ? [targetFile.path]
                      : [];
                onSetTierDialogPaths(paths);
                onSetShowTierDialog(true);
                onClose();
              }}
            >
              <svg
                className="context-icon storage-tier"
                viewBox="0 0 16 16"
                fill="currentColor"
              >
                <path d="M.5 3l.04.87a1.99 1.99 0 0 0-.342 1.311l.637 7A2 2 0 0 0 2.826 14H9.81a2 2 0 0 0 1.991-1.819l.637-7a1.99 1.99 0 0 0-.342-1.311L12.5 3H.5zm.217 1h11.566l-.166 2.894a.5.5 0 0 1-.421.45l-5.5.894a.5.5 0 0 1-.578-.45L1.717 4zM14 2H2a1 1 0 0 0-1 1v1h14V3a1 1 0 0 0-1-1zM2 1a2 2 0 0 0-2 2v1h16V3a2 2 0 0 0-2-2H2z" />
                <path d="M3 4.5a.5.5 0 0 1 .5-.5h6a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-6a.5.5 0 0 1-.5-.5v-1z" />
              </svg>
              Move to Storage Tier
            </button>
          )}

          <div className="context-divider" />

          {/* Delete */}
          <button
            className="context-item danger"
            onClick={() => {
              onHandleDelete();
              onClose();
            }}
          >
            <svg
              className="context-icon"
              viewBox="0 0 16 16"
              fill="currentColor"
            >
              <path d="M5.5 5.5A.5.5 0 0 1 6 6v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm2.5 0a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm3 .5a.5.5 0 0 0-1 0v6a.5.5 0 0 0 1 0V6z" />
              <path
                fillRule="evenodd"
                d="M14.5 3a1 1 0 0 1-1 1H13v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4h-.5a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1H6a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1h3.5a1 1 0 0 1 1 1v1zM4.118 4 4 4.059V13a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V4.059L11.882 4H4.118zM2.5 3V2h11v1h-11z"
              />
            </svg>
            Delete
            <span className="context-shortcut">⌘⌫</span>
          </button>
        </>
      )}

      {/* Full feature set for mount-based storage */}
      {!isObjStorage && (
        <>
          {/* Open action for file/folder */}
          {targetFile && (
            <>
              <button
                className="context-item"
                onClick={() => {
                  if (targetFile) {
                    const isFolder =
                      targetFile.isDirectory ||
                      targetFile.mimeType === 'folder' ||
                      targetFile.path.endsWith('/');
                    if (isFolder) {
                      onNavigateTo(targetFile.path);
                    } else {
                      onHandleOpenFile(targetFile);
                    }
                  }
                  onClose();
                }}
              >
                <svg
                  className="context-icon"
                  viewBox="0 0 16 16"
                  fill="currentColor"
                >
                  <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h3.172a1.5 1.5 0 0 1 1.06.44l.708.706a.5.5 0 0 0 .354.147H13.5A1.5 1.5 0 0 1 15 4.793v7.707A1.5 1.5 0 0 1 13.5 14h-11A1.5 1.5 0 0 1 1 12.5v-9z" />
                </svg>
                Open
              </button>

              {/* Open With submenu - only for files, not folders */}
              {targetFile &&
                !targetFile.isDirectory &&
                !(
                  targetFile.mimeType === 'folder' ||
                  targetFile.path.endsWith('/')
                ) && (
                  <div
                    className="context-item has-submenu"
                    onMouseEnter={() => {
                      if (targetFile) {
                        onLoadAppsForFile(targetFile);
                        onSetShowOpenWith(true);
                      }
                    }}
                    onMouseLeave={() => onSetShowOpenWith(false)}
                  >
                    <svg
                      className="context-icon"
                      viewBox="0 0 16 16"
                      fill="currentColor"
                    >
                      <path d="M6.5 1A1.5 1.5 0 0 0 5 2.5V3H1.5A1.5 1.5 0 0 0 0 4.5v8A1.5 1.5 0 0 0 1.5 14h13a1.5 1.5 0 0 0 1.5-1.5v-8A1.5 1.5 0 0 0 14.5 3H11v-.5A1.5 1.5 0 0 0 9.5 1h-3zm0 1h3a.5.5 0 0 1 .5.5V3H6v-.5a.5.5 0 0 1 .5-.5z" />
                    </svg>
                    Open With
                    <svg
                      className="context-arrow"
                      viewBox="0 0 16 16"
                      fill="currentColor"
                    >
                      <path d="M6 12.796V3.204L11.481 8 6 12.796z" />
                    </svg>
                    {/* Open With submenu */}
                    {showOpenWith && (
                      <div className="context-submenu">
                        {appsLoading ? (
                          <div className="context-item disabled">
                            Loading apps...
                          </div>
                        ) : availableApps.length > 0 ? (
                          availableApps.map((app, index) => (
                            <button
                              key={index}
                              className="context-item"
                              onClick={(e) => {
                                e.stopPropagation();
                                if (targetFile) {
                                  onHandleOpenFileWith(targetFile, app.path);
                                }
                                onClose();
                              }}
                            >
                              {app.name}
                            </button>
                          ))
                        ) : (
                          <div className="context-item disabled">
                            No apps found
                          </div>
                        )}
                        <div className="context-divider" />
                        <button
                          className="context-item"
                          onClick={async (e) => {
                            e.stopPropagation();
                            if (!targetFile) {
                              onClose();
                              return;
                            }

                            try {
                              const { open } =
                                await import('@tauri-apps/plugin-dialog');

                              onClose();

                              const platformForDialog = getPlatformInfoSync();
                              const selectedApp = await open({
                                title: 'Choose Application',
                                directory: false,
                                multiple: false,
                                filters: platformForDialog.isMac
                                  ? [
                                      {
                                        name: 'Applications',
                                        extensions: ['app'],
                                      },
                                    ]
                                  : platformForDialog.isWindows
                                    ? [
                                        {
                                          name: 'Executables',
                                          extensions: ['exe'],
                                        },
                                      ]
                                    : [],
                                defaultPath: platformForDialog.isMac
                                  ? '/Applications'
                                  : platformForDialog.isWindows
                                    ? 'C:\\Program Files'
                                    : '/usr/bin',
                              });

                              let appPath: string | null = null;

                              if (selectedApp === null) {
                                return;
                              } else if (typeof selectedApp === 'string') {
                                appPath = selectedApp;
                              } else if (Array.isArray(selectedApp)) {
                                const firstItem = selectedApp[0];
                                if (
                                  typeof firstItem === 'string' &&
                                  firstItem
                                ) {
                                  appPath = firstItem;
                                }
                              }

                              if (appPath && targetFile) {
                                await onHandleOpenFileWith(targetFile, appPath);
                              }
                            } catch (err) {
                              console.error('Failed to open app picker:', err);
                              DialogService.error(
                                `Failed to open application picker: ${err}`,
                                'Open With Error',
                              );
                            }
                          }}
                        >
                          Other...
                        </button>
                      </div>
                    )}
                  </div>
                )}

              {/* Asset Details - Get Info */}
              {targetFile && (
                <>
                  <div className="context-divider" />
                  <button
                    className="context-item"
                    onClick={() => {
                      if (targetFile) {
                        onSetInfoModal({
                          visible: true,
                          file: targetFile,
                        });
                      }
                      onClose();
                    }}
                  >
                    <svg
                      className="context-icon"
                      viewBox="0 0 16 16"
                      fill="currentColor"
                    >
                      <path d="M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14zm0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16z" />
                      <path d="m8.93 6.588-2.29.287-.082.38.45.083c.294.07.352.176.288.469l-.738 3.468c-.194.897.105 1.319.808 1.319.545 0 1.178-.252 1.465-.598l.088-.416c-.2.176-.492.246-.686.246-.275 0-.375-.193-.304-.533L8.93 6.588zM9 4.5a1 1 0 1 1-2 0 1 1 0 0 1 2 0z" />
                    </svg>
                    Asset Details
                    <span className="context-shortcut">⌘I</span>
                  </button>
                </>
              )}

              {/* Reveal in Finder/Explorer - Only for local storage */}
              {targetFile &&
                selectedSource &&
                selectedSource.category === 'local' && (
                  <>
                    <div className="context-divider" />
                    <button
                      className="context-item"
                      onClick={async () => {
                        if (targetFile && selectedSource) {
                          try {
                            await invoke('vfs_reveal_in_finder', {
                              sourceId: selectedSource.id,
                              path: targetFile.path,
                            });
                          } catch (err) {
                            console.error('Failed to reveal in Finder:', err);
                            DialogService.error(
                              `Failed to reveal in Finder: ${err}`,
                              'Reveal Error',
                            );
                          }
                        }
                        onClose();
                      }}
                    >
                      <svg
                        className="context-icon"
                        viewBox="0 0 16 16"
                        fill="currentColor"
                      >
                        <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h2.764c.958 0 1.553.69 2.301 1.5A1.5 1.5 0 0 1 8.5 4h5a1.5 1.5 0 0 1 1.5 1.5v7a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 11.5v-8zM2.5 3a.5.5 0 0 0-.5.5V6h12v-.5a.5.5 0 0 0-.5-.5h-5a.5.5 0 0 1-.447-.276L8.5 2.947l-.553.277a.5.5 0 0 1-.447.276h-5zM14 7H2v4.5a.5.5 0 0 0 .5.5h11a.5.5 0 0 0 .5-.5V7z" />
                      </svg>
                      {(() => {
                        const p = getPlatformInfoSync();
                        return p.isMac
                          ? 'Reveal in Finder'
                          : p.isWindows
                            ? 'Reveal in Explorer'
                            : 'Reveal in File Manager';
                      })()}
                    </button>
                  </>
                )}

              {/* Move to Storage Tier - Only available for cloud storage or when moving from local to cloud */}
              {targetFile &&
                (() => {
                  const canMoveToTier =
                    selectedSource &&
                    (selectedSource.category === 'cloud' ||
                      selectedSource.category === 'local');
                  const tierTooltip = canMoveToTier
                    ? undefined
                    : 'Move to Storage Tier is only available for cloud storage or when moving from local storage to cloud';

                  if (!canMoveToTier) return null;

                  return (
                    <>
                      <div className="context-divider" />
                      <button
                        className="context-item"
                        onClick={() => {
                          const paths =
                            selectedFiles.size > 0
                              ? Array.from(selectedFiles)
                              : targetFile
                                ? [targetFile.path]
                                : [];
                          onSetTierDialogPaths(paths);
                          onSetShowTierDialog(true);
                          onClose();
                        }}
                        title={tierTooltip}
                      >
                        <svg
                          className="context-icon storage-tier"
                          viewBox="0 0 16 16"
                          fill="currentColor"
                        >
                          <path d="M.5 3l.04.87a1.99 1.99 0 0 0-.342 1.311l.637 7A2 2 0 0 0 2.826 14H9.81a2 2 0 0 0 1.991-1.819l.637-7a1.99 1.99 0 0 0-.342-1.311L12.5 3H.5zm.217 1h11.566l-.166 2.894a.5.5 0 0 1-.421.45l-5.5.894a.5.5 0 0 1-.578-.45L1.717 4zM14 2H2a1 1 0 0 0-1 1v1h14V3a1 1 0 0 0-1-1zM2 1a2 2 0 0 0-2 2v1h16V3a2 2 0 0 0-2-2H2z" />
                          <path d="M3 4.5a.5.5 0 0 1 .5-.5h6a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-6a.5.5 0 0 1-.5-.5v-1z" />
                        </svg>
                        Move to Storage Tier
                      </button>
                    </>
                  );
                })()}

              <div className="context-divider" />
            </>
          )}

          {/* Clipboard actions */}
          {(selectedFiles.size > 0 || targetFile) && (
            <>
              {/* Copy - works from any storage type */}
              <button
                className="context-item"
                onClick={async (e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  await onHandleCopy();
                  onClose();
                }}
              >
                <svg
                  className="context-icon"
                  viewBox="0 0 16 16"
                  fill="currentColor"
                >
                  <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z" />
                  <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z" />
                </svg>
                Copy
                <span className="context-shortcut">⌘C</span>
              </button>
              {/* Cut - available for all storage types */}
              <button
                className="context-item"
                onClick={async (e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  await onHandleCut();
                  onClose();
                }}
              >
                <svg
                  className="context-icon"
                  viewBox="0 0 16 16"
                  fill="currentColor"
                >
                  <path d="M9.5 0a.5.5 0 0 0-.5.5v3a.5.5 0 0 1-.5.5h-3a.5.5 0 0 0 0 1h3a.5.5 0 0 1 .5.5v3a.5.5 0 0 0 1 0v-3A1.5 1.5 0 0 0 9.5 4h-3A1.5 1.5 0 0 0 5 5.5v3a.5.5 0 0 0 1 0v-3a.5.5 0 0 1 .5-.5h3a.5.5 0 0 0 .5-.5v-3A.5.5 0 0 0 9.5 0z" />
                  <path d="M13.354.646a1.207 1.207 0 0 0-1.708 0l-10 10A1 1 0 0 0 2 12h3v2a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2h3a1 1 0 0 0 .708-1.646l-10-10zM9 14v-2H7v2h2zm1 0v-2h2l-2 2z" />
                </svg>
                Cut
                <span className="context-shortcut">⌘X</span>
              </button>
              {/* Paste - works to any storage type */}
              <button
                className="context-item"
                onClick={async (e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  const targetPath =
                    targetFile &&
                    (targetFile.mimeType === 'folder' || targetFile.isDirectory)
                      ? targetFile.path
                      : undefined;
                  await onHandlePaste(targetPath);
                  onClose();
                }}
              >
                <svg
                  className="context-icon"
                  viewBox="0 0 16 16"
                  fill="currentColor"
                >
                  <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z" />
                  <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z" />
                </svg>
                Paste
                <span className="context-shortcut">⌘V</span>
              </button>
              {/* Rename - only for single file/folder (bulk rename not supported) */}
              {selectedFiles.size <= 1 && (
                <button
                  className="context-item"
                  onClick={() => {
                    if (targetFile) onHandleRename(targetFile);
                    onClose();
                  }}
                >
                  <svg
                    className="context-icon"
                    viewBox="0 0 16 16"
                    fill="currentColor"
                  >
                    <path d="M12.146.146a.5.5 0 0 1 .708 0l3 3a.5.5 0 0 1 0 .708l-10 10a.5.5 0 0 1-.168.11l-5 2a.5.5 0 0 1-.65-.65l2-5a.5.5 0 0 1 .11-.168l10-10zM11.207 2.5 13.5 4.793 14.793 3.5 12.5 1.207 11.207 2.5zm1.586 3L10.5 3.207 4 9.707V10h.5a.5.5 0 0 1 .5.5v.5h.5a.5.5 0 0 1 .5.5v.5h.293l6.5-6.5z" />
                  </svg>
                  Rename
                </button>
              )}
            </>
          )}

          {/* AI Features: Transcription and Tagging */}
          {targetFile && !targetFile.isDirectory && (
            <>
              {(() => {
                // Use both mimeType AND file extension for detection
                const extension = targetFile.name?.split('.').pop()?.toLowerCase() || '';
                const videoExts = ['mp4', 'mov', 'avi', 'mkv', 'webm', 'wmv', 'flv', 'm4v', 'mpg', 'mpeg'];
                const audioExts = ['mp3', 'wav', 'aiff', 'flac', 'm4a', 'aac', 'ogg', 'wma'];
                const imageExts = ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'tiff', 'tif', 'heic', 'heif'];
                
                const isVideoFile =
                  targetFile.mimeType?.startsWith('video/') ||
                  videoExts.includes(extension);
                const isAudioFile =
                  targetFile.mimeType?.startsWith('audio/') ||
                  audioExts.includes(extension);
                const isMediaFile = isVideoFile || isAudioFile;
                const isImageFile = 
                  targetFile.mimeType?.startsWith('image/') ||
                  imageExts.includes(extension);
                const isPdfFile = 
                  targetFile.mimeType === 'application/pdf' ||
                  extension === 'pdf';

                // Show transcription for video, audio, PDF, and images
                // But only if AI models are available
                if (isMediaFile || isPdfFile || isImageFile) {
                  if (aiModelsAvailable) {
                    return (
                      <>
                        <div className="context-divider" />
                        <button
                          className="context-item"
                          onClick={() => {
                            if (targetFile) {
                              onHandleTranscribe(targetFile);
                            }
                            onClose();
                          }}
                        >
                          <svg
                            className="context-icon"
                            viewBox="0 0 16 16"
                            fill="currentColor"
                          >
                            <path d="M8 0a8 8 0 1 0 0 16A8 8 0 0 0 8 0zM7 11.5a.5.5 0 0 1-1 0V7.707L5.354 8.854a.5.5 0 1 1-.708-.708l2-2a.5.5 0 0 1 .708 0l2 2a.5.5 0 0 1-.708.708L7 7.707V11.5z" />
                          </svg>
                          {isMediaFile
                            ? 'Transcribe Audio'
                            : isPdfFile
                              ? 'Extract Text (OCR)'
                              : 'Extract Text (OCR)'}
                          <span className="context-shortcut">⌘⇧T</span>
                        </button>
                      </>
                    );
                  } else {
                    // Models not available - show settings option
                    return (
                      <>
                        <div className="context-divider" />
                        <button
                          className="context-item"
                          onClick={() => {
                            if (onOpenSettings) {
                              window.dispatchEvent(new CustomEvent('open-ai-settings'));
                              onOpenSettings();
                            }
                            onClose();
                          }}
                        >
                          <svg
                            className="context-icon"
                            viewBox="0 0 16 16"
                            fill="currentColor"
                          >
                            <path d="M8 4.754a3.246 3.246 0 1 0 0 6.492 3.246 3.246 0 0 0 0-6.492zM5.754 8a2.246 2.246 0 1 1 4.492 0 2.246 2.246 0 0 1-4.492 0z"/>
                            <path d="M9.796 1.343c-.527-1.79-3.065-1.79-3.592 0l-.094.319a.873.873 0 0 1-1.255.52l-.292-.16c-1.64-.892-3.433.902-2.54 2.541l.159.292a.873.873 0 0 1-.52 1.255l-.319.094c-1.79.527-1.79 3.065 0 3.592l.319.094a.873.873 0 0 1 .52 1.255l-.16.292c-.892 1.64.901 3.434 2.541 2.54l.292-.159a.873.873 0 0 1 1.255.52l.094.319c.527 1.79 3.065 1.79 3.592 0l.094-.319a.873.873 0 0 1 1.255-.52l.292.16c1.64.893 3.434-.902 2.54-2.541l-.159-.292a.873.873 0 0 1 .52-1.255l.319-.094c1.79-.527 1.79-3.065 0-3.592l-.319-.094a.873.873 0 0 1-.52-1.255l.16-.292c.893-1.64-.902-3.433-2.541-2.54l-.292.159a.873.873 0 0 1-1.255-.52l-.094-.319z"/>
                          </svg>
                          Configure AI Settings
                        </button>
                      </>
                    );
                  }
                }
                return null;
              })()}

              {/* AI Tagging: available for image, video, and audio files */}
              {/* But only if AI models are available */}
              {(() => {
                // Use both mimeType AND file extension for detection
                const extension = targetFile.name?.split('.').pop()?.toLowerCase() || '';
                const videoExts = ['mp4', 'mov', 'avi', 'mkv', 'webm', 'wmv', 'flv', 'm4v', 'mpg', 'mpeg'];
                const audioExts = ['mp3', 'wav', 'aiff', 'flac', 'm4a', 'aac', 'ogg', 'wma'];
                const imageExts = ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'tiff', 'tif', 'heic', 'heif'];
                
                const isTaggable =
                  targetFile.mimeType?.startsWith('video/') ||
                  targetFile.mimeType?.startsWith('audio/') ||
                  targetFile.mimeType?.startsWith('image/') ||
                  videoExts.includes(extension) ||
                  audioExts.includes(extension) ||
                  imageExts.includes(extension);

                if (isTaggable && aiModelsAvailable) {
                  return (
                    <>
                      <button
                        className="context-item"
                        onClick={() => {
                          if (targetFile && onHandleAutoTag) {
                            // Directly trigger AI auto-tagging
                            onHandleAutoTag(targetFile);
                          } else if (targetFile) {
                            // Fallback: open info modal for manual tagging
                            onSetInfoModal({ visible: true, file: targetFile });
                          }
                          onClose();
                        }}
                      >
                        <svg
                          className="context-icon"
                          viewBox="0 0 16 16"
                          fill="currentColor"
                        >
                          <path d="M3 2v4.586l7 7L13.586 9l-7-7H3zM2 2a1 1 0 0 1 1-1h4.586a1 1 0 0 1 .707.293l7 7a1 1 0 0 1 0 1.414l-4.586 4.586a1 1 0 0 1-1.414 0l-7-7A1 1 0 0 1 2 6.586V2z" />
                        </svg>
                        Generate AI Tags
                        <span className="context-shortcut">⌘⇧A</span>
                      </button>
                    </>
                  );
                }
                return null;
              })()}
            </>
          )}

          {/* Delete - available for all storage types */}
          <>
            <div className="context-divider" />
            <button
              className="context-item danger"
              onClick={() => {
                onHandleDelete();
                onClose();
              }}
            >
              <svg
                className="context-icon"
                viewBox="0 0 16 16"
                fill="currentColor"
              >
                <path d="M5.5 5.5A.5.5 0 0 1 6 6v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm2.5 0a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-1 0V6a.5.5 0 0 1 .5-.5zm3 .5a.5.5 0 0 0-1 0v6a.5.5 0 0 0 1 0V6z" />
                <path
                  fillRule="evenodd"
                  d="M14.5 3a1 1 0 0 1-1 1H13v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V4h-.5a1 1 0 0 1-1-1V2a1 1 0 0 1 1-1H6a1 1 0 0 1 1-1h2a1 1 0 0 1 1 1h3.5a1 1 0 0 1 1 1v1zM4.118 4 4 4.059V13a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V4.059L11.882 4H4.118zM2.5 3V2h11v1h-11z"
                />
              </svg>
              {supportsFilesystemOperations(selectedSource)
                ? 'Move to Trash'
                : 'Delete'}
              <span className="context-shortcut">⌘⌫</span>
            </button>
          </>
        </>
      )}

      {/* Object storage - empty space menu (right-click on background) */}
      {!targetFile && isObjStorage && (
        <>
          {/* Paste - if clipboard has content */}
          <button
            className="context-item"
            onClick={async (e) => {
              e.preventDefault();
              e.stopPropagation();
              await onHandlePaste();
              onClose();
            }}
          >
            <svg
              className="context-icon"
              viewBox="0 0 16 16"
              fill="currentColor"
            >
              <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z" />
              <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z" />
            </svg>
            Paste
            <span className="context-shortcut">⌘V</span>
          </button>

          <div className="context-divider" />

          {/* Upload Files or Folders */}
          <button
            className="context-item"
            onClick={async () => {
              console.log('[FinderPage] Upload clicked');
              onClose();
              await onHandleUpload();
            }}
          >
            <svg
              className="context-icon"
              viewBox="0 0 16 16"
              fill="currentColor"
            >
              <path d="M.5 9.9a.5.5 0 0 1 .5.5v2.5a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-2.5a.5.5 0 0 1 1 0v2.5a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2v-2.5a.5.5 0 0 1 .5-.5z" />
              <path d="M7.646 1.146a.5.5 0 0 1 .708 0l3 3a.5.5 0 0 1-.708.708L8.5 2.707V11.5a.5.5 0 0 1-1 0V2.707L5.354 4.854a.5.5 0 1 1-.708-.708l3-3z" />
            </svg>
            Upload Files or Folders
          </button>
        </>
      )}

      {/* Upload to S3 - only when storage is S3 (fallback for non-object storage cloud) */}
      {!targetFile &&
        !isObjStorage &&
        selectedSource &&
        (selectedSource.providerId === 's3' ||
          selectedSource.providerId === 'aws-s3' ||
          selectedSource.providerId === 's3-compatible' ||
          selectedSource.category === 'cloud') && (
          <>
            <div className="context-divider" />
            <button
              className="context-item"
              onClick={async () => {
                console.log('[FinderPage] Upload clicked');
                onClose();
                await onHandleUpload();
              }}
            >
              <svg
                className="context-icon"
                viewBox="0 0 16 16"
                fill="currentColor"
              >
                <path d="M.5 9.9a.5.5 0 0 1 .5.5v2.5a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-2.5a.5.5 0 0 1 1 0v2.5a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2v-2.5a.5.5 0 0 1 .5-.5z" />
                <path d="M7.646 1.146a.5.5 0 0 1 .708 0l3 3a.5.5 0 0 1-.708.708L8.5 2.707V11.5a.5.5 0 0 1-1 0V2.707L5.354 4.854a.5.5 0 1 1-.708-.708l3-3z" />
              </svg>
              Upload Files or Folders
            </button>
          </>
        )}

      {/* New Folder - available on empty space for all storage types */}
      {!targetFile && (
        <>
          <div className="context-divider" />
          <button
            className="context-item"
            onClick={() => {
              onHandleNewFolder();
              onClose();
            }}
          >
            <svg
              className="context-icon"
              viewBox="0 0 16 16"
              fill="currentColor"
            >
              <path d="M.5 3l.04.87a1.99 1.99 0 0 0-.342 1.311l.637 7A2 2 0 0 0 2.826 14H9.81a2 2 0 0 0 1.991-1.819l.637-7a1.99 1.99 0 0 0-.342-1.311L12.5 3H.5zm.217 1h11.566l-.166 2.894a.5.5 0 0 1-.421.45l-5.5.894a.5.5 0 0 1-.578-.45L1.717 4zM14 2H2a1 1 0 0 0-1 1v1h14V3a1 1 0 0 0-1-1zM2 1a2 2 0 0 0-2 2v1h16V3a2 2 0 0 0-2-2H2z" />
            </svg>
            New Folder
            <span className="context-shortcut">⌘⇧N</span>
          </button>
        </>
      )}

      {/* Download action - for cloud/remote storage files */}
      {!isMountedStorage() && targetFile && !targetFile.isDirectory && (
        <>
          <div className="context-divider" />
          <button
            className="context-item"
            onClick={async () => {
              if (!selectedSource || !targetFile) {
                onClose();
                return;
              }

              try {
                // Download to Downloads folder automatically
                // The operation will be tracked and shown in DownloadProgressPanel modal
                const operationId = await invoke<string>(
                  'vfs_download_to_downloads',
                  {
                    sourceId: selectedSource.id,
                    path: targetFile.path,
                  },
                );

                // Trigger download-started event for OperationsPanel
                if (operationId) {
                  setTimeout(() => {
                    window.dispatchEvent(
                      new CustomEvent('download-started', {
                        detail: { operationId },
                      }),
                    );
                  }, 100);
                }

                console.log(
                  '[Context Menu] Download started with operation_id:',
                  operationId,
                );
                // Don't show error dialog on success - OperationsPanel modal will show progress
              } catch (err) {
                console.error('Download failed:', err);
                DialogService.error(
                  `Download failed: ${err}`,
                  'Download Error',
                );
              }
              onClose();
            }}
          >
            <svg
              className="context-icon"
              viewBox="0 0 16 16"
              fill="currentColor"
            >
              <path d="M.5 9.9a.5.5 0 0 1 .5.5v2.5a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-2.5a.5.5 0 0 1 1 0v2.5a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2v-2.5a.5.5 0 0 1 .5-.5z" />
              <path d="M7.646 11.854a.5.5 0 0 0 .708 0l3-3a.5.5 0 0 0-.708-.708L8.5 10.293V1.5a.5.5 0 0 0-1 0v8.793L5.354 8.146a.5.5 0 1 0-.708.708l3 3z" />
            </svg>
            Download
          </button>
        </>
      )}

      {/* Video Player Modal */}
      {showVideoPlayer && videoPlayerProps && (
        <VideoPlayer
          fileName={videoPlayerProps.fileName}
          sourceId={videoPlayerProps.sourceId}
          filePath={videoPlayerProps.filePath}
          sizeHuman={videoPlayerProps.sizeHuman}
          onClose={() => {
            setShowVideoPlayer(false);
            setVideoPlayerProps(null);
          }}
        />
      )}
    </div>
  );
}
