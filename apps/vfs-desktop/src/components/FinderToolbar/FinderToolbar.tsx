/**
 * FinderToolbar Component
 *
 * Extracted toolbar panel from FinderPage for better maintainability.
 * Contains navigation buttons, breadcrumbs, view toggles, and action buttons.
 */

import React from 'react';
import { Breadcrumbs, type BreadcrumbItem } from '../Breadcrumbs';
import { SearchBox } from '../SearchBox';
import type { FileMetadata } from '../../types/storage';
import type { ViewMode } from '../../pages/FinderPage/types';
import './FinderToolbar.css';

export interface FinderToolbarProps {
  canGoBack: boolean;
  canGoForward: boolean;
  canGoUp: boolean;
  viewMode: ViewMode;
  searchQuery: string;
  showHiddenFiles: boolean;
  showInfoPanel: boolean;
  files: FileMetadata[];
  selectedSource: {
    category: string;
    providerId?: string;
  } | null;
  breadcrumbs: BreadcrumbItem[];
  onGoBack: () => Promise<void>;
  onGoForward: () => Promise<void>;
  onGoUp: () => Promise<void>;
  onNavigateTo: (path: string) => Promise<void>;
  onSetViewMode: (mode: ViewMode) => void;
  onSetSearchQuery: (query: string) => void;
  onSetShowHiddenFiles: (show: boolean) => void;
  onSetShowInfoPanel: (show: boolean) => void;
  onHandleUpload: () => void;
}

export function FinderToolbar({
  canGoBack,
  canGoForward,
  canGoUp,
  viewMode,
  searchQuery,
  showHiddenFiles,
  showInfoPanel,
  files,
  selectedSource,
  breadcrumbs,
  onGoBack,
  onGoForward,
  onGoUp,
  onNavigateTo,
  onSetViewMode,
  onSetSearchQuery,
  onSetShowHiddenFiles,
  onSetShowInfoPanel,
  onHandleUpload,
}: FinderToolbarProps) {
  return (
    <div className="finder-toolbar">
      <div className="toolbar-nav">
        {/* Back button */}
        <button
          className="toolbar-btn nav"
          onClick={onGoBack}
          disabled={!canGoBack}
          title="Go Back (⌘[)"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M15 18l-6-6 6-6" />
          </svg>
        </button>
        {/* Forward button */}
        <button
          className="toolbar-btn nav"
          onClick={onGoForward}
          disabled={!canGoForward}
          title="Go Forward (⌘])"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M9 18l6-6-6-6" />
          </svg>
        </button>
        {/* Up button */}
        <button
          className="toolbar-btn nav"
          onClick={onGoUp}
          disabled={!canGoUp}
          title="Go Up (⌘↑)"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M18 15l-6-6-6 6" />
          </svg>
        </button>
      </div>

      <div className="toolbar-center">
        <Breadcrumbs
          items={breadcrumbs}
          onNavigate={onNavigateTo}
          maxVisible={5}
          showIcons={true}
        />
      </div>

      <div className="toolbar-right">
        {/* Upload button - only show for object storage */}
        {selectedSource &&
          (selectedSource.category === 'cloud' ||
            selectedSource.providerId === 's3' ||
            selectedSource.providerId === 'aws-s3' ||
            selectedSource.providerId === 's3-compatible' ||
            selectedSource.providerId === 'gcs' ||
            selectedSource.providerId === 'azure-blob') && (
            <button
              className="toolbar-btn upload-btn"
              onClick={onHandleUpload}
              title="Upload files, folders, or a mix of both to Object Storage"
            >
              <svg
                viewBox="0 0 16 16"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                width="18"
                height="18"
              >
                <path d="M8 11V1M8 1l3 3M8 1L5 4" />
                <path d="M2 6v8a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V6" />
              </svg>
              <span className="upload-btn-label">Upload</span>
            </button>
          )}

        <div className="view-switcher">
          <button
            className={`view-btn ${viewMode === 'icon' ? 'active' : ''}`}
            onClick={() => onSetViewMode('icon')}
            title="Grid View"
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
              <rect x="1" y="1" width="6" height="6" rx="1.5" />
              <rect x="9" y="1" width="6" height="6" rx="1.5" />
              <rect x="1" y="9" width="6" height="6" rx="1.5" />
              <rect x="9" y="9" width="6" height="6" rx="1.5" />
            </svg>
          </button>
          <button
            className={`view-btn ${viewMode === 'list' ? 'active' : ''}`}
            onClick={() => onSetViewMode('list')}
            title="List View"
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
              <rect x="1" y="2" width="14" height="2.5" rx="1" />
              <rect x="1" y="6.75" width="14" height="2.5" rx="1" />
              <rect x="1" y="11.5" width="14" height="2.5" rx="1" />
            </svg>
          </button>
        </div>

        <SearchBox
          value={searchQuery}
          onChange={onSetSearchQuery}
          files={files}
          placeholder="Search files..."
        />

        {/* Toggle hidden files */}
        <button
          className={`toolbar-btn ${showHiddenFiles ? 'active' : ''}`}
          onClick={(e) => {
            e.stopPropagation();
            e.preventDefault();
            onSetShowHiddenFiles(!showHiddenFiles);
          }}
          title={showHiddenFiles ? 'Hide Hidden Files' : 'Show Hidden Files'}
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.75"
            width="20"
            height="20"
          >
            {showHiddenFiles ? (
              <>
                <path d="M2.5 12s3.5-7 9.5-7 9.5 7 9.5 7-3.5 7-9.5 7-9.5-7-9.5-7z" />
                <circle cx="12" cy="12" r="3.5" />
                <circle cx="12" cy="12" r="1" fill="currentColor" />
              </>
            ) : (
              <>
                <path d="M2 2l20 20" strokeWidth="2" />
                <path d="M6.7 6.7C4.2 8.5 2.5 12 2.5 12s3.5 7 9.5 7c2 0 3.8-.6 5.3-1.5" />
                <path d="M17.3 14.3c1.3-1.2 2.2-2.3 2.2-2.3s-3.5-7-9.5-7c-.7 0-1.4.1-2 .2" />
                <circle cx="12" cy="12" r="3.5" />
              </>
            )}
          </svg>
        </button>

        {/* Toggle Info Panel */}
        <button
          className={`toolbar-btn ${showInfoPanel ? 'active' : ''}`}
          onClick={() => onSetShowInfoPanel(!showInfoPanel)}
          title="Toggle Info Panel"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.75"
            width="20"
            height="20"
          >
            <rect x="3" y="3" width="18" height="18" rx="2.5" />
            <line x1="9" y1="3" x2="9" y2="21" />
            <circle cx="15" cy="10" r="1.5" fill="currentColor" />
            <line
              x1="15"
              y1="13"
              x2="15"
              y2="17"
              strokeWidth="2"
              strokeLinecap="round"
            />
          </svg>
        </button>
      </div>
    </div>
  );
}
