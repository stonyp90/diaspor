//! Settings Domain Models
//!
//! Domain entities for application settings, particularly provider credentials.
//! These models are part of the domain layer and have no dependencies on
//! infrastructure or adapters.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Provider-specific credential settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "provider")]
pub enum ProviderCredentials {
    /// AWS S3 credentials
    #[serde(rename = "aws-s3")]
    AwsS3 {
        /// AWS Access Key ID
        access_key_id: String,
        /// AWS Secret Access Key (encrypted at rest)
        secret_access_key: EncryptedString,
        /// Optional session token for temporary credentials
        session_token: Option<EncryptedString>,
        /// AWS region
        region: String,
    },
    /// Google Cloud Storage credentials
    #[serde(rename = "gcs")]
    Gcs {
        /// Service account JSON (encrypted at rest)
        service_account_json: EncryptedString,
        /// Project ID
        project_id: String,
    },
    /// Azure Blob Storage credentials
    #[serde(rename = "azure-blob")]
    AzureBlob {
        /// Account name
        account_name: String,
        /// Account key (encrypted at rest)
        account_key: EncryptedString,
    },
    /// S3-compatible storage credentials
    #[serde(rename = "s3-compatible")]
    S3Compatible {
        /// Access Key ID
        access_key_id: String,
        /// Secret Access Key (encrypted at rest)
        secret_access_key: EncryptedString,
        /// Endpoint URL
        endpoint: String,
        /// Region (optional)
        region: Option<String>,
    },
    /// Oracle Object Storage credentials
    #[serde(rename = "oracle")]
    Oracle {
        /// Namespace
        namespace: String,
        /// User OCID
        user_ocid: String,
        /// Fingerprint
        fingerprint: String,
        /// Private key (encrypted at rest)
        private_key: EncryptedString,
        /// Passphrase (encrypted at rest, optional)
        passphrase: Option<EncryptedString>,
        /// Region
        region: String,
    },
    /// Generic provider with key-value pairs
    #[serde(rename = "custom")]
    Custom {
        /// Provider ID
        provider_id: String,
        /// Credential key-value pairs (values encrypted at rest)
        credentials: HashMap<String, EncryptedString>,
    },
}

/// Encrypted string wrapper
/// 
/// This type represents a string that should be encrypted at rest.
/// The actual encryption/decryption is handled by the settings adapter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub struct EncryptedString {
    /// The encrypted value (base64 encoded ciphertext)
    #[serde(skip_serializing_if = "Option::is_none")]
    encrypted: Option<String>,
    /// The plaintext value (only present in memory, never serialized)
    #[serde(skip)]
    plaintext: Option<String>,
}

impl EncryptedString {
    /// Create a new encrypted string from plaintext
    pub fn new_plaintext(plaintext: String) -> Self {
        Self {
            encrypted: None,
            plaintext: Some(plaintext),
        }
    }
    
    /// Create a new encrypted string from encrypted data
    pub fn new_encrypted(encrypted: String) -> Self {
        Self {
            encrypted: Some(encrypted),
            plaintext: None,
        }
    }
    
    /// Get plaintext (if available)
    pub fn plaintext(&self) -> Option<&str> {
        self.plaintext.as_deref()
    }
    
    /// Get encrypted value (if available)
    pub fn encrypted(&self) -> Option<&str> {
        self.encrypted.as_deref()
    }
    
    /// Set plaintext
    pub fn set_plaintext(&mut self, plaintext: String) {
        self.plaintext = Some(plaintext);
        self.encrypted = None; // Clear encrypted when plaintext is set
    }
    
    /// Set encrypted value
    pub fn set_encrypted(&mut self, encrypted: String) {
        self.encrypted = Some(encrypted);
        self.plaintext = None; // Clear plaintext when encrypted is set
    }
    
    /// Check if this is plaintext
    pub fn is_plaintext(&self) -> bool {
        self.plaintext.is_some()
    }
    
    /// Check if this is encrypted
    pub fn is_encrypted(&self) -> bool {
        self.encrypted.is_some()
    }
}


/// Storage source settings
/// 
/// Associates a storage source ID with its provider credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSourceSettings {
    /// Storage source ID
    pub source_id: String,
    /// Storage source name
    pub source_name: String,
    /// Provider credentials
    pub credentials: ProviderCredentials,
    /// When these credentials were last updated
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    /// Storage source settings (keyed by source_id)
    pub storage_sources: HashMap<String, StorageSourceSettings>,
    /// Global application preferences
    pub preferences: AppPreferences,
}

/// Application preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPreferences {
    /// Default cache directory
    pub cache_directory: Option<String>,
    /// Maximum cache size in bytes
    pub max_cache_size: Option<u64>,
    /// Whether to use encryption for stored credentials
    pub encrypt_credentials: bool,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            cache_directory: None,
            max_cache_size: None,
            encrypt_credentials: true, // Default to encrypting credentials
        }
    }
}
