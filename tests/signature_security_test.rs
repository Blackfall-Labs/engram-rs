//! Phase 1.3: Signature Security Tests
//!
//! Tests for Ed25519 signature validation, tampering detection, and attack scenarios.
//! Based on TESTING_PLAN.md Phase 1.3

use engram_rs::{ArchiveReader, ArchiveWriter, Author, Manifest};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use tempfile::NamedTempFile;

/// Helper: Create archive with signed manifest
fn create_signed_archive() -> (NamedTempFile, SigningKey) {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let signing_key = SigningKey::generate(&mut OsRng);

    let mut manifest = Manifest::new(
        "test-archive".to_string(),
        "Test Archive".to_string(),
        Author::new("Test Author"),
        "1.0.0".to_string(),
    );
    manifest.sign(&signing_key, Some("Test Signer".to_string())).unwrap();

    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.add_file("test.txt", b"Hello, World!").unwrap();

    let manifest_json = serde_json::to_value(&manifest).unwrap();
    writer.add_manifest(&manifest_json).unwrap();
    writer.finalize().unwrap();

    (temp_file, signing_key)
}

#[test]
fn test_valid_signature_verification() {
    let (temp_file, _signing_key) = create_signed_archive();
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    // Verify signatures
    let results = manifest.verify_signatures().unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0], "Valid signature should verify successfully");

    // Test is_fully_signed
    assert!(manifest.is_fully_signed().unwrap());
}

#[test]
fn test_modified_manifest_invalid_signature() {
    let (temp_file, _signing_key) = create_signed_archive();
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let mut manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    // Modify manifest content (change metadata version)
    manifest.metadata.version = "2.0.0".to_string();

    // Signature should now be invalid
    let results = manifest.verify_signatures().unwrap();
    assert_eq!(results.len(), 1);
    assert!(!results[0], "Modified manifest should have invalid signature");

    assert!(!manifest.is_fully_signed().unwrap());
}

#[test]
fn test_missing_signatures() {
    let (temp_file, _signing_key) = create_signed_archive();
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let mut manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    // Remove all signatures
    manifest.signatures.clear();

    // Should not be valid (no signatures)
    assert!(!manifest.is_fully_signed().unwrap());

    // Verification should return empty Vec
    let results = manifest.verify_signatures().unwrap();
    assert_eq!(results.len(), 0);
}

#[test]
fn test_multiple_signatures() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Create manifest with multiple signatures
    let key1 = SigningKey::generate(&mut OsRng);
    let key2 = SigningKey::generate(&mut OsRng);
    let key3 = SigningKey::generate(&mut OsRng);

    let mut manifest = Manifest::new(
        "multi-sig".to_string(),
        "Multi-Signature Test".to_string(),
        Author::new("Test Author"),
        "1.0.0".to_string(),
    );
    manifest.sign(&key1, Some("Signer 1".to_string())).unwrap();
    manifest.sign(&key2, Some("Signer 2".to_string())).unwrap();
    manifest.sign(&key3, Some("Signer 3".to_string())).unwrap();

    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.add_file("test.txt", b"data").unwrap();

    let manifest_json = serde_json::to_value(&manifest).unwrap();
    writer.add_manifest(&manifest_json).unwrap();
    writer.finalize().unwrap();

    // Read back and verify
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    assert_eq!(manifest.signatures.len(), 3);

    let results = manifest.verify_signatures().unwrap();
    assert_eq!(results.len(), 3);
    assert!(results[0], "Signature 1 should be valid");
    assert!(results[1], "Signature 2 should be valid");
    assert!(results[2], "Signature 3 should be valid");

    assert!(manifest.is_fully_signed().unwrap());
}

#[test]
fn test_corrupted_signature_hex() {
    let (temp_file, _signing_key) = create_signed_archive();
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let mut manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    // Corrupt the signature hex string
    if let Some(sig) = manifest.signatures.first_mut() {
        sig.signature = "INVALIDHEX!!!".to_string();
    }

    // Verification should fail gracefully
    let results = manifest.verify_signatures().unwrap();
    assert!(!results[0], "Corrupted signature should be invalid");

    assert!(!manifest.is_fully_signed().unwrap());
}

#[test]
fn test_signature_wrong_length() {
    let (temp_file, _signing_key) = create_signed_archive();
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let mut manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    // Set signature to valid hex but wrong length (Ed25519 signatures are 64 bytes)
    if let Some(sig) = manifest.signatures.first_mut() {
        sig.signature = "deadbeef".to_string(); // Only 4 bytes
    }

    // Verification should fail
    let results = manifest.verify_signatures().unwrap();
    assert!(!results[0], "Wrong-length signature should be invalid");

    assert!(!manifest.is_fully_signed().unwrap());
}

#[test]
fn test_signature_algorithm_mismatch() {
    let (temp_file, _signing_key) = create_signed_archive();
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let mut manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    // Change algorithm to unsupported value
    if let Some(sig) = manifest.signatures.first_mut() {
        sig.algorithm = "rsa2048".to_string();
    }

    // Should fail verification (unsupported algorithm)
    let results = manifest.verify_signatures().unwrap();
    assert!(!results[0], "Unsupported algorithm should fail");

    assert!(!manifest.is_fully_signed().unwrap());
}

#[test]
fn test_empty_public_key() {
    let (temp_file, _signing_key) = create_signed_archive();
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let mut manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    // Set public key to empty string
    if let Some(sig) = manifest.signatures.first_mut() {
        sig.public_key = "".to_string();
    }

    // Should fail verification
    let results = manifest.verify_signatures().unwrap();
    assert!(!results[0], "Empty public key should fail");

    assert!(!manifest.is_fully_signed().unwrap());
}

#[test]
fn test_signature_with_modified_public_key() {
    let (temp_file, _signing_key) = create_signed_archive();
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let mut manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    // Generate different key and replace public key
    let different_key = SigningKey::generate(&mut OsRng);
    let different_verifying_key = different_key.verifying_key();

    if let Some(sig) = manifest.signatures.first_mut() {
        sig.public_key = hex::encode(different_verifying_key.to_bytes());
    }

    // Signature was created with original key but public key is different
    // Verification should fail
    let results = manifest.verify_signatures().unwrap();
    assert!(!results[0], "Signature with wrong public key should be invalid");

    assert!(!manifest.is_fully_signed().unwrap());
}

#[test]
fn test_replay_attack_different_manifest() {
    // Create two different archives with two different keys
    let key1 = SigningKey::generate(&mut OsRng);
    let key2 = SigningKey::generate(&mut OsRng);

    // Archive 1
    let temp1 = NamedTempFile::new().unwrap();
    let mut manifest1 = Manifest::new(
        "archive1".to_string(),
        "Archive 1".to_string(),
        Author::new("Author 1"),
        "1.0.0".to_string(),
    );
    manifest1.sign(&key1, Some("Key 1".to_string())).unwrap();

    let mut writer1 = ArchiveWriter::create(temp1.path()).unwrap();
    writer1.add_file("file1.txt", b"data1").unwrap();

    let manifest1_json = serde_json::to_value(&manifest1).unwrap();
    writer1.add_manifest(&manifest1_json).unwrap();
    writer1.finalize().unwrap();

    // Archive 2
    let temp2 = NamedTempFile::new().unwrap();
    let mut manifest2 = Manifest::new(
        "archive2".to_string(),
        "Archive 2".to_string(),
        Author::new("Author 2"),
        "2.0.0".to_string(),
    );
    manifest2.sign(&key2, Some("Key 2".to_string())).unwrap();

    let mut writer2 = ArchiveWriter::create(temp2.path()).unwrap();
    writer2.add_file("file2.txt", b"data2").unwrap();

    let manifest2_json = serde_json::to_value(&manifest2).unwrap();
    writer2.add_manifest(&manifest2_json).unwrap();
    writer2.finalize().unwrap();

    // Read archive 1's manifest
    let mut reader1 = ArchiveReader::open_and_init(temp1.path()).unwrap();
    let manifest1_value = reader1.read_manifest().unwrap().unwrap();
    let manifest1: Manifest = serde_json::from_value(manifest1_value).unwrap();

    // Try to apply archive 1's signature to archive 2's content (replay attack)
    let mut reader2 = ArchiveReader::open_and_init(temp2.path()).unwrap();
    let manifest2_value = reader2.read_manifest().unwrap().unwrap();
    let mut manifest2: Manifest = serde_json::from_value(manifest2_value).unwrap();

    // Replace manifest2's signature with manifest1's signature
    manifest2.signatures = manifest1.signatures.clone();

    // Verification should fail (signature doesn't match content)
    let results = manifest2.verify_signatures().unwrap();
    assert!(!results[0], "Replayed signature should be invalid");

    assert!(!manifest2.is_fully_signed().unwrap());
}

#[test]
fn test_partial_signature_list() {
    // Test case: Archive with 3 signatures, one is invalid
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let key1 = SigningKey::generate(&mut OsRng);
    let key2 = SigningKey::generate(&mut OsRng);
    let key3 = SigningKey::generate(&mut OsRng);

    let mut manifest = Manifest::new(
        "partial-sig".to_string(),
        "Partial Signature Test".to_string(),
        Author::new("Test Author"),
        "1.0.0".to_string(),
    );
    manifest.sign(&key1, Some("Valid 1".to_string())).unwrap();
    manifest.sign(&key2, Some("Valid 2".to_string())).unwrap();
    manifest.sign(&key3, Some("Valid 3".to_string())).unwrap();

    // Corrupt the second signature
    manifest.signatures[1].signature = "deadbeefdeadbeef".to_string();

    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.add_file("test.txt", b"data").unwrap();

    let manifest_json = serde_json::to_value(&manifest).unwrap();
    writer.add_manifest(&manifest_json).unwrap();
    writer.finalize().unwrap();

    // Read and verify
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    let results = manifest.verify_signatures().unwrap();
    assert_eq!(results.len(), 3);
    assert!(results[0], "First signature should be valid");
    assert!(!results[1], "Second signature should be invalid (corrupted)");
    assert!(results[2], "Third signature should be valid");

    // is_fully_signed should return false if ANY signature is invalid
    assert!(!manifest.is_fully_signed().unwrap());
}

#[test]
fn test_signature_timestamp() {
    let (temp_file, _signing_key) = create_signed_archive();
    let path = temp_file.path();

    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    // Check that signature has a timestamp
    assert_eq!(manifest.signatures.len(), 1);
    let sig = &manifest.signatures[0];

    // Timestamp should be reasonable (within last hour and not in future)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    assert!(sig.timestamp <= now, "Timestamp should not be in future");
    assert!(sig.timestamp > now - 3600, "Timestamp should be recent (within 1 hour)");
}

#[test]
fn test_signature_without_signer_name() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let signing_key = SigningKey::generate(&mut OsRng);

    let mut manifest = Manifest::new(
        "no-signer".to_string(),
        "No Signer Test".to_string(),
        Author::new("Test Author"),
        "1.0.0".to_string(),
    );
    // Sign without signer name (None)
    manifest.sign(&signing_key, None).unwrap();

    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.add_file("test.txt", b"data").unwrap();

    let manifest_json = serde_json::to_value(&manifest).unwrap();
    writer.add_manifest(&manifest_json).unwrap();
    writer.finalize().unwrap();

    // Read back and verify
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let manifest: Manifest = serde_json::from_value(manifest_value).unwrap();

    // Signature should still be valid even without signer name
    let results = manifest.verify_signatures().unwrap();
    assert!(results[0], "Signature without signer name should still be valid");

    assert!(manifest.is_fully_signed().unwrap());

    // Check that signer is None
    assert!(manifest.signatures[0].signer.is_none());
}
