/**
 * File Icon Utility
 *
 * Get appropriate icon for file types
 */
import { FileMetadata } from '../../domain/entities/FileMetadata';
import React from 'react';
import {
  getFileIcon as getFileIconComponent,
  IconFolder,
} from '../../components/CyberpunkIcons';

export function getFileIcon(file: FileMetadata, size = 48): React.ReactNode {
  const isFolder =
    file.isDirectory || file.mimeType === 'folder' || file.path.endsWith('/');
  if (isFolder) {
    return React.createElement(IconFolder, {
      size,
      color: 'currentColor',
      glow: false,
      className: 'folder-icon',
    });
  }
  const IconComponent = getFileIconComponent(file.name, file.mimeType);
  return React.createElement(IconComponent, {
    size,
    color: 'currentColor',
    glow: false,
  });
}
