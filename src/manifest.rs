//! Manifest support for Engram archives
//!
//! # Manifest Scope
//!
//! The Engram manifest (`manifest.json`) is **reserved for format-level metadata only**:
//! - Archive identification (name, version, description)
//! - File inventory with integrity hashes (SHA-256)
//! - Digital signatures for verification (Ed25519)
//! - Format capabilities and compression metadata
//!
//! **Applications must use separate files for application-specific metadata:**
//! - Recommended pattern: `<app-name>.json` (e.g., `crisis-frame.json`, `myapp.json`)
//! - Applications may store multiple metadata files as needed
//! - This allows multiple applications to coexist in one archive
//!
//! # Example Archive Structure
//!
//! ```text
//! archive.eng
//! ├── manifest.json           (Engram format metadata)
//! ├── crisis-frame.json       (Crisis Frame backup metadata)
//! ├── database/crisis.db      (Application data)
//! └── logs/frame.log
//! ```
//!
//! # Usage
//!
//! ```no_run
//! use engram_rs::{ArchiveWriter, manifest::{Manifest, Author}};
//! # use engram_rs::error::Result;
//!
//! # fn main() -> Result<()> {
//! let mut writer = ArchiveWriter::create("backup.eng")?;
//!
//! // 1. Add Engram format manifest (reserved fields)
//! let manifest = Manifest::new(
//!     "backup-2025-11-30".to_string(),
//!     "Crisis Frame Backup".to_string(),
//!     Author::new("Crisis Frame System"),
//!     "1.0.0".to_string()
//! );
//! writer.add_manifest(&serde_json::to_value(&manifest)?)?;
//!
//! // 2. Add application-specific manifest (separate file)
//! let app_manifest = serde_json::json!({
//!     "services": ["database", "logs", "config"],
//!     "backup_type": "nightly",
//!     "timestamp": "2025-11-30T08:00:00Z"
//! });
//! writer.add_file("crisis-frame.json",
//!     serde_json::to_string_pretty(&app_manifest)?.as_bytes())?;
//!
//! // 3. Add application data
//! writer.add_file_from_disk("database/crisis.db",
//!     &std::path::Path::new("path/to/crisis.db"))?;
//!
//! writer.finalize()?;
//! # Ok(())
//! # }
//! ```

use crate::error::{EngramError, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Engram manifest structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Manifest format version
    pub version: String,

    /// Archive identifier
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Archive description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Author information
    pub author: Author,

    /// Archive metadata
    pub metadata: Metadata,

    /// Capabilities this engram declares
    #[serde(default)]
    pub capabilities: Vec<String>,

    /// Files in the archive
    #[serde(default)]
    pub files: Vec<FileEntry>,

    /// Cryptographic signatures
    #[serde(default)]
    pub signatures: Vec<SignatureEntry>,
}

/// Author information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Author {
    /// Create a new author with just a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: None,
            url: None,
        }
    }
}

/// Archive metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// Semantic version
    pub version: String,

    /// Creation timestamp (Unix epoch)
    pub created: u64,

    /// Last modified timestamp (Unix epoch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,

    /// License identifier (SPDX)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// File entry in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// File path in archive
    pub path: String,

    /// SHA-256 hash of uncompressed content
    pub sha256: String,

    /// File size (uncompressed)
    pub size: u64,

    /// MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Cryptographic signature entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureEntry {
    /// Signature algorithm (e.g., "ed25519")
    pub algorithm: String,

    /// Public key (hex-encoded)
    pub public_key: String,

    /// Signature (hex-encoded)
    pub signature: String,

    /// Timestamp when signature was created
    pub timestamp: u64,

    /// Signer identity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer: Option<String>,
}

impl Manifest {
    /// Create a new manifest
    pub fn new(id: String, name: String, author: Author, version: String) -> Self {
        Self {
            version: "0.4.0".to_string(),
            id,
            name,
            description: None,
            author,
            metadata: Metadata {
                version,
                created: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                modified: None,
                license: None,
                tags: Vec::new(),
            },
            capabilities: Vec::new(),
            files: Vec::new(),
            signatures: Vec::new(),
        }
    }

    /// Add a file entry to the manifest
    pub fn add_file(&mut self, path: String, data: &[u8], mime_type: Option<String>) {
        let sha256 = hex::encode(Sha256::digest(data));
        self.files.push(FileEntry {
            path,
            sha256,
            size: data.len() as u64,
            mime_type,
        });
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> Result<Vec<u8>> {
        serde_json::to_vec_pretty(self).map_err(EngramError::from)
    }

    /// Parse from JSON
    pub fn from_json(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data).map_err(EngramError::from)
    }

    /// Calculate canonical hash for signing
    ///
    /// This creates a deterministic representation of the manifest
    /// (excluding signatures) for cryptographic signing.
    pub fn canonical_hash(&self) -> Result<[u8; 32]> {
        // Create a copy without signatures
        let mut manifest_copy = self.clone();
        manifest_copy.signatures.clear();

        // Serialize to JSON with sorted keys (serde_json does this by default)
        let json = serde_json::to_vec(&manifest_copy)?;

        // Hash the JSON
        Ok(Sha256::digest(&json).into())
    }

    /// Sign the manifest with a signing key
    pub fn sign(&mut self, signing_key: &SigningKey, signer: Option<String>) -> Result<()> {
        let hash = self.canonical_hash()?;
        let signature = signing_key.sign(&hash);

        let public_key = signing_key.verifying_key();

        self.signatures.push(SignatureEntry {
            algorithm: "ed25519".to_string(),
            public_key: hex::encode(public_key.to_bytes()),
            signature: hex::encode(signature.to_bytes()),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signer,
        });

        Ok(())
    }

    /// Verify all signatures in the manifest
    pub fn verify_signatures(&self) -> Result<Vec<bool>> {
        let mut results = Vec::new();
        let hash = self.canonical_hash()?;

        for sig_entry in &self.signatures {
            let result = self.verify_signature_entry(sig_entry, &hash);
            results.push(result.is_ok());
        }

        Ok(results)
    }

    /// Verify a single signature entry
    fn verify_signature_entry(&self, entry: &SignatureEntry, hash: &[u8; 32]) -> Result<()> {
        if entry.algorithm != "ed25519" {
            return Err(EngramError::InvalidSignature);
        }

        // Decode public key
        let public_key_bytes =
            hex::decode(&entry.public_key).map_err(|_| EngramError::InvalidPublicKey)?;
        let public_key_array: [u8; 32] = public_key_bytes
            .try_into()
            .map_err(|_| EngramError::InvalidPublicKey)?;
        let public_key = VerifyingKey::from_bytes(&public_key_array)?;

        // Decode signature
        let signature_bytes =
            hex::decode(&entry.signature).map_err(|_| EngramError::InvalidSignature)?;
        let signature_array: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| EngramError::InvalidSignature)?;
        let signature = Signature::from_bytes(&signature_array);

        // Verify
        public_key.verify(hash, &signature)?;

        Ok(())
    }

    /// Check if all signatures are valid
    pub fn is_fully_signed(&self) -> Result<bool> {
        if self.signatures.is_empty() {
            return Ok(false);
        }

        let results = self.verify_signatures()?;
        Ok(results.iter().all(|&valid| valid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_manifest_creation() {
        let manifest = Manifest::new(
            "test-engram".to_string(),
            "Test Engram".to_string(),
            Author {
                name: "Test Author".to_string(),
                email: Some("test@example.com".to_string()),
                url: None,
            },
            "0.1.0".to_string(),
        );

        assert_eq!(manifest.version, "0.4.0");
        assert_eq!(manifest.id, "test-engram");
        assert_eq!(manifest.author.name, "Test Author");
    }

    #[test]
    fn test_add_file() {
        let mut manifest = Manifest::new(
            "test".to_string(),
            "Test".to_string(),
            Author {
                name: "Test".to_string(),
                email: None,
                url: None,
            },
            "0.1.0".to_string(),
        );

        let data = b"Hello, World!";
        manifest.add_file("test.txt".to_string(), data, Some("text/plain".to_string()));

        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].path, "test.txt");
        assert_eq!(manifest.files[0].size, 13);
    }

    #[test]
    fn test_signature_roundtrip() {
        let mut manifest = Manifest::new(
            "test".to_string(),
            "Test".to_string(),
            Author {
                name: "Test".to_string(),
                email: None,
                url: None,
            },
            "0.1.0".to_string(),
        );

        // Generate a key pair
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);

        // Sign the manifest
        manifest
            .sign(&signing_key, Some("Test Signer".to_string()))
            .unwrap();

        assert_eq!(manifest.signatures.len(), 1);
        assert_eq!(manifest.signatures[0].algorithm, "ed25519");

        // Verify signatures
        let results = manifest.verify_signatures().unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0]);

        assert!(manifest.is_fully_signed().unwrap());
    }

    #[test]
    fn test_json_roundtrip() {
        let manifest = Manifest::new(
            "test".to_string(),
            "Test".to_string(),
            Author {
                name: "Test".to_string(),
                email: None,
                url: None,
            },
            "0.1.0".to_string(),
        );

        let json = manifest.to_json().unwrap();
        let parsed = Manifest::from_json(&json).unwrap();

        assert_eq!(parsed.id, manifest.id);
        assert_eq!(parsed.name, manifest.name);
    }
}
