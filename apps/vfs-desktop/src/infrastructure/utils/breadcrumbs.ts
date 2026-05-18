/**
 * Breadcrumbs Utility
 *
 * Build breadcrumbs that work across different storage types
 */
import React from 'react';
import { BreadcrumbItem } from '../../components/Breadcrumbs';
import { StorageSource } from '../../domain/entities/StorageSource';

export function buildBreadcrumbs(
  source: StorageSource | null,
  currentPath: string,
): BreadcrumbItem[] {
  if (!source) {
    return [];
  }

  const items: BreadcrumbItem[] = [];

  // Add storage source as first item
  items.push({
    name: source.name,
    path: '/',
    icon: getStorageIcon(),
  });

  // Parse path and add segments
  if (currentPath && currentPath !== '/') {
    const segments = currentPath.split('/').filter((s) => s.length > 0);

    let accumulatedPath = '';
    for (const segment of segments) {
      accumulatedPath += '/' + segment;
      items.push({
        name: segment,
        path: accumulatedPath + (currentPath.endsWith('/') ? '/' : ''),
      });
    }
  }

  return items;
}

function getStorageIcon(): React.ReactNode {
  // Map storage categories to icons
  // Return a simple string identifier for now - can be enhanced to return icon components
  return null; // Icons will be handled by the Breadcrumbs component
}
