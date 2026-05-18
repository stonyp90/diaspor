/**
 * FinderPage Types
 *
 * Shared types for the FinderPage and its sub-components.
 */
import type {
  StorageSource,
  FileMetadata,
  WarmProgress,
  GlobalFavorite,
} from '../../types/storage';

export type ViewMode = 'icon' | 'list';

export interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  targetFile?: FileMetadata;
}

export interface FinderPageProps {
  onOpenMetrics?: () => void;
  onOpenSearch?: () => void;
  isSearchOpen?: boolean;
  onCloseSearch?: () => void;
  onOpenSettings?: () => void;
}

export interface CrossStorageDragState {
  sourceId: string;
  destSourceId: string;
  paths: string[];
  isMove: boolean;
}

export interface ColumnFilters {
  name: string;
  date: string;
  size: string;
  tier: string;
}

export type SortColumn = 'name' | 'modified' | 'size' | 'storage-class';
export type SortDirection = 'asc' | 'desc';

export interface ColumnWidths {
  name: number;
  modified: number;
  size: number;
  tier: number;
  'storage-class'?: number;
}

export interface FileOperationState {
  type: string;
  inProgress: boolean;
}

export interface SidebarSection {
  id: string;
  title: string;
  items: SidebarItem[];
  isCollapsed?: boolean;
}

export interface SidebarItem {
  id: string;
  name: string;
  icon: React.ReactNode;
  path?: string;
  sourceId?: string;
  onClick?: () => void;
  isActive?: boolean;
}

export type { StorageSource, FileMetadata, WarmProgress, GlobalFavorite };
