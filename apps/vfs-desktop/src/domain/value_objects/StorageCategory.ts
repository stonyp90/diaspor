/**
 * Storage Category Value Object
 *
 * Represents the category/type of storage provider
 */
export type StorageCategory =
  | 'local' // Local filesystem
  | 'cloud' // Cloud object storage (S3, GCS, Azure Blob, etc.)
  | 'block' // Block storage (EBS, Azure Disk, FSx, etc.)
  | 'network' // Network shares (NFS, SMB, CIFS, AFP)
  | 'hybrid' // Hybrid solutions (FSx ONTAP, NetApp, etc.)
  | 'custom'; // User-defined / plugins
