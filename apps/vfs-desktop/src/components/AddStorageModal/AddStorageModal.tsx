/**
 * AddStorageModal - Dynamic storage source configuration
 *
 * Allows users to add any supported storage type:
 * - Cloud: S3, GCS, Azure, MinIO, Wasabi, etc.
 * - Network: NFS, SMB/CIFS, WebDAV, SFTP
 * - Hybrid: FSx ONTAP, NetApp
 * - Custom: User-defined providers
 */
import React, { useState, useEffect } from 'react';
import { StorageCategory, StorageSource } from '../../types/storage';
import {
  IconCloud,
  IconNetwork,
  IconDatabase,
  IconCube,
  IconServer,
  IconFolder,
  IconLink,
} from '../CyberpunkIcons';
import './AddStorageModal.css';

interface AddStorageModalProps {
  isOpen: boolean;
  onClose: () => void;
  onAdd: (source: Partial<StorageSource>) => void;
  editingSource?: StorageSource | null;
}

// Helper to get provider icon component
const getProviderIcon = (providerId: string) => {
  const iconProps = { size: 24, color: 'currentColor' };
  switch (providerId) {
    case 's3':
    case 'gcs':
    case 'azure-blob':
    case 's3-compatible':
      return <IconCloud {...iconProps} />;
    case 'smb':
    case 'nfs':
      return <IconFolder {...iconProps} />;
    case 'sftp':
      return <IconServer {...iconProps} />;
    case 'webdav':
      return <IconLink {...iconProps} />;
    case 'fsx-ontap':
    case 'netapp':
      return <IconDatabase {...iconProps} />;
    case 'iscsi':
    case 'fc':
      return <IconCube {...iconProps} />;
    default:
      return <IconServer {...iconProps} />;
  }
};

// Storage provider templates (simplified - backend has full schema)
const PROVIDER_TEMPLATES = [
  {
    category: 'cloud' as StorageCategory,
    providers: [
      { id: 's3', name: 'Amazon S3' },
      { id: 'gcs', name: 'Google Cloud Storage' },
      { id: 'azure-blob', name: 'Azure Blob Storage' },
      {
        id: 's3-compatible',
        name: 'S3 Compatible',
        description: 'MinIO, Wasabi, R2, etc.',
      },
      { id: 'backblaze-b2', name: 'Backblaze B2' },
      { id: 'digitalocean-spaces', name: 'DigitalOcean Spaces' },
      { id: 'cloudflare-r2', name: 'Cloudflare R2' },
      { id: 'linode-object', name: 'Linode Object Storage' },
      { id: 'wasabi', name: 'Wasabi' },
      { id: 'minio', name: 'MinIO' },
    ],
  },
  {
    category: 'network' as StorageCategory,
    providers: [
      { id: 'smb', name: 'SMB/CIFS Share' },
      { id: 'nfs', name: 'NFS Mount' },
      { id: 'sftp', name: 'SFTP Server' },
      { id: 'webdav', name: 'WebDAV' },
    ],
  },
  {
    category: 'hybrid' as StorageCategory,
    providers: [
      { id: 'fsx-ontap', name: 'FSx for ONTAP' },
      { id: 'netapp', name: 'NetApp' },
    ],
  },
  {
    category: 'block' as StorageCategory,
    providers: [
      { id: 'iscsi', name: 'iSCSI Target' },
      { id: 'fc', name: 'Fibre Channel' },
    ],
  },
];

// Config fields per provider (subset - full validation on backend)
const PROVIDER_FIELDS: Record<
  string,
  {
    key: string;
    label: string;
    type: string;
    required: boolean;
    placeholder?: string;
    options?: Array<{ value: string; label: string }>;
  }[]
> = {
  s3: [
    {
      key: 'bucket',
      label: 'Bucket Name',
      type: 'text',
      required: true,
      placeholder: 'my-bucket',
    },
    {
      key: 'region',
      label: 'Region',
      type: 'select',
      required: true,
      options: [
        { value: 'us-east-1', label: 'US East (N. Virginia)' },
        { value: 'us-east-2', label: 'US East (Ohio)' },
        { value: 'us-west-1', label: 'US West (N. California)' },
        { value: 'us-west-2', label: 'US West (Oregon)' },
        { value: 'eu-west-1', label: 'EU (Ireland)' },
        { value: 'ap-southeast-1', label: 'Asia Pacific (Singapore)' },
      ],
    },
    {
      key: 'accessKeyId',
      label: 'Access Key',
      type: 'text',
      required: false,
      placeholder: 'Leave empty to use environment variables',
    },
    {
      key: 'secretAccessKey',
      label: 'Secret Key',
      type: 'password',
      required: false,
      placeholder: 'Leave empty to use environment variables',
    },
    {
      key: 'sessionToken',
      label: 'Session Token',
      type: 'password',
      required: false,
      placeholder: 'Only needed for temporary credentials',
    },
    // Note: If credentials are not provided, the backend will read from environment variables
    // This allows users to either enter credentials here OR set them as environment variables
  ],
  gcs: [
    {
      key: 'bucket',
      label: 'Bucket Name',
      type: 'text',
      required: true,
      placeholder: 'my-gcs-bucket',
    },
    { key: 'projectId', label: 'Project ID', type: 'text', required: true },
    {
      key: 'credentialsPath',
      label: 'Service Account JSON Path',
      type: 'text',
      required: false,
      placeholder: 'Optional - leave empty to use environment variables',
    },
    {
      key: 'credentialsJson',
      label: 'Service Account JSON',
      type: 'textarea',
      required: false,
      placeholder: 'Paste JSON credentials here',
    },
  ],
  'azure-blob': [
    { key: 'container', label: 'Container Name', type: 'text', required: true },
    {
      key: 'accountName',
      label: 'Storage Account',
      type: 'text',
      required: true,
    },
    {
      key: 'accountKey',
      label: 'Account Key',
      type: 'password',
      required: false,
      placeholder: 'Optional - leave empty to use environment variables',
    },
    {
      key: 'sasToken',
      label: 'SAS Token',
      type: 'password',
      required: false,
      placeholder: 'Optional - alternative to account key',
    },
  ],
  's3-compatible': [
    {
      key: 'endpoint',
      label: 'Endpoint URL',
      type: 'text',
      required: true,
      placeholder: 'https://s3.example.com',
    },
    { key: 'bucket', label: 'Bucket Name', type: 'text', required: true },
    {
      key: 'region',
      label: 'Region',
      type: 'text',
      required: false,
      placeholder: 'auto',
    },
    {
      key: 'accessKeyId',
      label: 'Access Key ID',
      type: 'text',
      required: true,
    },
    {
      key: 'secretAccessKey',
      label: 'Secret Access Key',
      type: 'password',
      required: true,
    },
    {
      key: 'forcePathStyle',
      label: 'Force Path Style',
      type: 'checkbox',
      required: false,
      placeholder: 'Required for MinIO',
    },
  ],
  'backblaze-b2': [
    {
      key: 'bucket',
      label: 'Bucket Name',
      type: 'text',
      required: true,
      placeholder: 'my-b2-bucket',
    },
    {
      key: 'keyId',
      label: 'Application Key ID',
      type: 'text',
      required: true,
      placeholder: 'B2 Application Key ID',
    },
    {
      key: 'applicationKey',
      label: 'Application Key',
      type: 'password',
      required: true,
      placeholder: 'B2 Application Key',
    },
    {
      key: 'endpoint',
      label: 'Endpoint',
      type: 'text',
      required: false,
      placeholder: 'Optional',
    },
  ],
  'digitalocean-spaces': [
    {
      key: 'bucket',
      label: 'Space Name',
      type: 'text',
      required: true,
      placeholder: 'my-space',
    },
    {
      key: 'region',
      label: 'Region',
      type: 'text',
      required: true,
      placeholder: 'nyc3, sfo3, ams3, etc.',
    },
    {
      key: 'accessKeyId',
      label: 'Spaces Access Key',
      type: 'text',
      required: true,
    },
    {
      key: 'secretAccessKey',
      label: 'Spaces Secret Key',
      type: 'password',
      required: true,
    },
    {
      key: 'endpoint',
      label: 'Endpoint',
      type: 'text',
      required: false,
      placeholder: 'Optional',
    },
  ],
  'cloudflare-r2': [
    {
      key: 'bucket',
      label: 'Bucket Name',
      type: 'text',
      required: true,
      placeholder: 'my-r2-bucket',
    },
    {
      key: 'accountId',
      label: 'Account ID',
      type: 'text',
      required: true,
      placeholder: 'Cloudflare Account ID',
    },
    {
      key: 'accessKeyId',
      label: 'Access Key ID',
      type: 'text',
      required: true,
      placeholder: 'R2 Access Key ID',
    },
    {
      key: 'secretAccessKey',
      label: 'Secret Access Key',
      type: 'password',
      required: true,
      placeholder: 'R2 Secret Access Key',
    },
    {
      key: 'endpoint',
      label: 'Endpoint',
      type: 'text',
      required: false,
      placeholder: 'Optional',
    },
  ],
  'linode-object': [
    {
      key: 'bucket',
      label: 'Bucket Name',
      type: 'text',
      required: true,
      placeholder: 'my-linode-bucket',
    },
    {
      key: 'region',
      label: 'Region',
      type: 'text',
      required: true,
      placeholder: 'us-east-1, eu-central-1, etc.',
    },
    {
      key: 'accessKeyId',
      label: 'Access Key',
      type: 'text',
      required: true,
      placeholder: 'Linode Object Storage Access Key',
    },
    {
      key: 'secretAccessKey',
      label: 'Secret Key',
      type: 'password',
      required: true,
      placeholder: 'Linode Object Storage Secret Key',
    },
    {
      key: 'endpoint',
      label: 'Endpoint',
      type: 'text',
      required: false,
      placeholder: 'Optional',
    },
  ],
  wasabi: [
    {
      key: 'bucket',
      label: 'Bucket Name',
      type: 'text',
      required: true,
      placeholder: 'my-wasabi-bucket',
    },
    {
      key: 'region',
      label: 'Region',
      type: 'text',
      required: true,
      placeholder: 'us-east-1, us-west-1, eu-central-1, etc.',
    },
    {
      key: 'accessKeyId',
      label: 'Access Key',
      type: 'text',
      required: true,
      placeholder: 'Wasabi Access Key',
    },
    {
      key: 'secretAccessKey',
      label: 'Secret Key',
      type: 'password',
      required: true,
      placeholder: 'Wasabi Secret Key',
    },
    {
      key: 'endpoint',
      label: 'Endpoint',
      type: 'text',
      required: false,
      placeholder: 'Optional',
    },
  ],
  minio: [
    {
      key: 'endpoint',
      label: 'MinIO Endpoint',
      type: 'text',
      required: true,
      placeholder: 'https://minio.example.com:9000',
    },
    {
      key: 'bucket',
      label: 'Bucket Name',
      type: 'text',
      required: true,
      placeholder: 'my-minio-bucket',
    },
    {
      key: 'accessKeyId',
      label: 'Access Key',
      type: 'text',
      required: true,
      placeholder: 'MinIO Access Key',
    },
    {
      key: 'secretAccessKey',
      label: 'Secret Key',
      type: 'password',
      required: true,
      placeholder: 'MinIO Secret Key',
    },
    {
      key: 'region',
      label: 'Region',
      type: 'text',
      required: false,
      placeholder: 'Optional',
    },
    {
      key: 'forcePathStyle',
      label: 'Force Path Style',
      type: 'checkbox',
      required: false,
      placeholder: 'Required for MinIO',
    },
  ],
  smb: [
    {
      key: 'server',
      label: 'Server',
      type: 'text',
      required: true,
      placeholder: 'fileserver.local',
    },
    {
      key: 'share',
      label: 'Share Name',
      type: 'text',
      required: true,
      placeholder: 'media',
    },
    { key: 'username', label: 'Username', type: 'text', required: false },
    { key: 'password', label: 'Password', type: 'password', required: false },
  ],
  nfs: [
    {
      key: 'server',
      label: 'Server',
      type: 'text',
      required: true,
      placeholder: 'nfs.local',
    },
    {
      key: 'export',
      label: 'Export Path',
      type: 'text',
      required: true,
      placeholder: '/exports/media',
    },
  ],
  sftp: [
    {
      key: 'host',
      label: 'Host',
      type: 'text',
      required: true,
      placeholder: 'sftp.example.com',
    },
    {
      key: 'port',
      label: 'Port',
      type: 'text',
      required: false,
      placeholder: '22',
    },
    { key: 'username', label: 'Username', type: 'text', required: true },
    { key: 'password', label: 'Password', type: 'password', required: false },
  ],
  webdav: [
    {
      key: 'url',
      label: 'WebDAV URL',
      type: 'text',
      required: true,
      placeholder: 'https://dav.example.com/files',
    },
    { key: 'username', label: 'Username', type: 'text', required: false },
    { key: 'password', label: 'Password', type: 'password', required: false },
  ],
  'fsx-ontap': [
    {
      key: 'endpoint',
      label: 'Management Endpoint',
      type: 'text',
      required: true,
    },
    { key: 'volumePath', label: 'Volume Path', type: 'text', required: true },
  ],
  netapp: [
    { key: 'server', label: 'NetApp Server', type: 'text', required: true },
    { key: 'volume', label: 'Volume', type: 'text', required: true },
  ],
  iscsi: [
    { key: 'target', label: 'iSCSI Target', type: 'text', required: true },
    { key: 'portal', label: 'Portal Address', type: 'text', required: true },
  ],
  fc: [
    { key: 'wwnn', label: 'WWNN', type: 'text', required: true },
    { key: 'lun', label: 'LUN', type: 'text', required: true },
  ],
};

const getCategoryIcon = (category: StorageCategory) => {
  const iconProps = { size: 20, color: 'currentColor', glow: true };
  switch (category) {
    case 'cloud':
      return <IconCloud {...iconProps} />;
    case 'network':
      return <IconNetwork {...iconProps} />;
    case 'hybrid':
      return <IconDatabase {...iconProps} />;
    case 'block':
      return <IconCube {...iconProps} />;
    default:
      return <IconServer {...iconProps} />;
  }
};

export const AddStorageModal: React.FC<AddStorageModalProps> = ({
  isOpen,
  onClose,
  onAdd,
  editingSource,
}) => {
  const [step, setStep] = useState<'select' | 'configure'>('select');
  const [selectedProvider, setSelectedProvider] = useState<string | null>(null);
  const [selectedCategory, setSelectedCategory] =
    useState<StorageCategory | null>(null);
  const [name, setName] = useState('');
  const [config, setConfig] = useState<Record<string, string | boolean>>({});
  const [error, setError] = useState<string | null>(null);

  // Debug logging
  React.useEffect(() => {
    if (isOpen) {
      console.log('[AddStorageModal] Modal opened');
    }
  }, [isOpen]);

  // Initialize form when editing
  React.useEffect(() => {
    if (isOpen && editingSource) {
      setStep('configure');
      setSelectedProvider(editingSource.providerId);
      setSelectedCategory(editingSource.category);
      setName(editingSource.name);
      setConfig(editingSource.config as Record<string, string>);
      setError(null);
    } else if (isOpen && !editingSource) {
      // Reset form when opening for new source
      setStep('select');
      setSelectedProvider(null);
      setSelectedCategory(null);
      setName('');
      setConfig({});
      setError(null);
    }
  }, [isOpen, editingSource]);

  if (!isOpen) return null;

  const handleProviderSelect = (
    providerId: string,
    category: StorageCategory,
  ) => {
    setSelectedProvider(providerId);
    setSelectedCategory(category);
    setStep('configure');
    if (!editingSource) {
      setConfig({});
    }
    setError(null);
  };

  const handleBack = () => {
    setStep('select');
    setSelectedProvider(null);
    setError(null);
  };

  const handleSubmit = () => {
    if (!selectedProvider || !selectedCategory) return;

    const fields = PROVIDER_FIELDS[selectedProvider] || [];
    const missingRequired = fields.filter((f) => f.required && !config[f.key]);

    if (missingRequired.length > 0) {
      setError(
        `Missing required fields: ${missingRequired.map((f) => f.label).join(', ')}`,
      );
      return;
    }

    if (!name.trim()) {
      setError('Please enter a display name');
      return;
    }

    const source: Partial<StorageSource> = {
      id: editingSource?.id || `${selectedProvider}-${Date.now()}`,
      name: name.trim(),
      providerId: selectedProvider,
      category: selectedCategory,
      config,
      status: 'disconnected',
    };

    onAdd(source);
    handleClose();
  };

  const handleClose = () => {
    try {
      setStep('select');
      setSelectedProvider(null);
      setSelectedCategory(null);
      setName('');
      setConfig({});
      setError(null);
      onClose();
    } catch (err) {
      console.error('[AddStorageModal] Error closing modal:', err);
      onClose(); // Still try to close
    }
  };

  // Close on Escape key
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        handleClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []); // Empty deps - handleClose is stable

  const fields = selectedProvider
    ? PROVIDER_FIELDS[selectedProvider] || []
    : [];
  const providerName =
    PROVIDER_TEMPLATES.flatMap((g) => g.providers).find(
      (p) => p.id === selectedProvider,
    )?.name || 'Storage';

  return (
    <div
      className="add-storage-overlay"
      onClick={handleClose}
      role="dialog"
      aria-modal="true"
      aria-labelledby="add-storage-title"
    >
      <div className="add-storage-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2 id="add-storage-title">
            {editingSource
              ? `Edit ${editingSource.name}`
              : step === 'select'
                ? 'Add Storage'
                : `Configure ${providerName}`}
          </h2>
          <button className="close-btn" onClick={handleClose}>
            ×
          </button>
        </div>

        <div className="modal-content">
          {step === 'select' && (
            <div className="provider-grid">
              {PROVIDER_TEMPLATES.map((group) => (
                <div key={group.category} className="provider-group">
                  <div className="group-header">
                    {getCategoryIcon(group.category)}
                    <span>
                      {group.category.charAt(0).toUpperCase() +
                        group.category.slice(1)}
                    </span>
                  </div>
                  <div className="group-providers">
                    {group.providers.map((provider) => (
                      <button
                        key={provider.id}
                        className="provider-btn"
                        onClick={() =>
                          handleProviderSelect(provider.id, group.category)
                        }
                      >
                        <span className="provider-icon">
                          {getProviderIcon(provider.id)}
                        </span>
                        <span className="provider-name">{provider.name}</span>
                        {provider.description && (
                          <span className="provider-desc">
                            {provider.description}
                          </span>
                        )}
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}

          {step === 'configure' && (
            <div className="config-form">
              <div className="form-field">
                <label htmlFor="display-name">Display Name *</label>
                <input
                  id="display-name"
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={`My ${providerName}`}
                />
              </div>

              {fields.map((field) => (
                <div key={field.key} className="form-field">
                  <label htmlFor={`field-${field.key}`}>
                    {field.label}
                    {field.required && ' *'}
                  </label>
                  {field.type === 'textarea' ? (
                    <textarea
                      id={`field-${field.key}`}
                      value={
                        (typeof config[field.key] === 'string'
                          ? config[field.key]
                          : String(config[field.key] || '')) as string
                      }
                      onChange={(e) => {
                        setConfig({ ...config, [field.key]: e.target.value });
                      }}
                      placeholder={field.placeholder}
                    />
                  ) : field.type === 'checkbox' ? (
                    <label className="checkbox-label">
                      <input
                        id={`field-${field.key}`}
                        type="checkbox"
                        checked={
                          config[field.key] === 'true' ||
                          config[field.key] === true ||
                          config[field.key] === 'True'
                        }
                        onChange={(e) => {
                          setConfig({
                            ...config,
                            [field.key]: e.target.checked,
                          });
                        }}
                      />
                      <span>{field.placeholder || 'Enable'}</span>
                    </label>
                  ) : field.type === 'select' && field.options ? (
                    <select
                      id={`field-${field.key}`}
                      value={
                        (typeof config[field.key] === 'string'
                          ? config[field.key]
                          : String(config[field.key] || '')) as string
                      }
                      onChange={(e) => {
                        let value = e.target.value;
                        // Normalize region to lowercase (AWS requires lowercase regions)
                        if (field.key === 'region') {
                          value = value.toLowerCase();
                        }
                        setConfig({ ...config, [field.key]: value });
                      }}
                    >
                      <option value="">Select {field.label}</option>
                      {field.options.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <input
                      id={`field-${field.key}`}
                      type={field.type === 'password' ? 'password' : 'text'}
                      value={
                        (typeof config[field.key] === 'string'
                          ? config[field.key]
                          : String(config[field.key] || '')) as string
                      }
                      onChange={(e) => {
                        let value = e.target.value;
                        // Normalize region to lowercase (AWS requires lowercase regions)
                        if (field.key === 'region') {
                          value = value.toLowerCase();
                        }
                        setConfig({ ...config, [field.key]: value });
                      }}
                      placeholder={field.placeholder}
                    />
                  )}
                </div>
              ))}

              {error && <div className="form-error">{error}</div>}
            </div>
          )}
        </div>

        <div className="modal-footer">
          {step === 'configure' && (
            <button className="back-btn" onClick={handleBack}>
              Back
            </button>
          )}
          <div className="footer-spacer" />
          <button className="cancel-btn" onClick={handleClose}>
            Cancel
          </button>
          {step === 'configure' && (
            <button className="add-btn" onClick={handleSubmit}>
              {editingSource ? 'Save Changes' : 'Add Storage'}
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

export default AddStorageModal;
