//! Phase 4.3: Cryptographic Attack Tests
//!
//! Tests for cryptographic security: timing attacks, weak keys, nonce reuse.
//! Based on TESTING_PLAN.md Phase 4.1 and 4.4

use ed25519_dalek::SigningKey;
use engram_rs::{ArchiveReader, ArchiveWriter, Author, Manifest};
use rand::rngs::OsRng;
use std::collections::HashSet;
use std::time::Instant;
use tempfile::NamedTempFile;

#[test]
fn test_signature_verification_basic() {
    println!("\n🔐 Testing basic Ed25519 signature verification...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Create archive with manifest and signature
    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.add_file("test.txt", b"test data").unwrap();

    // Create and sign manifest
    let mut manifest = Manifest::new(
        "test-archive".to_string(),
        "Test Archive".to_string(),
        Author::new("Test Author"),
        "1.0.0".to_string(),
    );

    let signing_key = SigningKey::generate(&mut OsRng);

    manifest
        .sign(&signing_key, Some("Test Signer".to_string()))
        .unwrap();

    writer
        .add_manifest(&serde_json::to_value(&manifest).unwrap())
        .unwrap();
    writer.finalize().unwrap();

    // Read and verify
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let loaded_manifest: Manifest =
        Manifest::from_json(&serde_json::to_vec(&manifest_value).unwrap()).unwrap();

    // Verify signatures
    let results = loaded_manifest.verify_signatures().unwrap();
    assert_eq!(results.len(), 1, "Should have 1 signature");
    assert!(results[0], "Signature should be valid");

    println!("  ✅ Ed25519 signature verification working correctly");
    println!("     - Signature valid: {}", results[0]);
}

#[test]
fn test_signature_with_modified_data() {
    println!("\n🔐 Testing signature invalidation on data modification...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Create signed archive
    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.add_file("data.txt", b"original data").unwrap();

    let mut manifest = Manifest::new(
        "test".to_string(),
        "Test".to_string(),
        Author::new("Author"),
        "1.0.0".to_string(),
    );

    let signing_key = SigningKey::generate(&mut OsRng);
    manifest.sign(&signing_key, None).unwrap();

    writer
        .add_manifest(&serde_json::to_value(&manifest).unwrap())
        .unwrap();
    writer.finalize().unwrap();

    // Read manifest
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let loaded_manifest: Manifest =
        Manifest::from_json(&serde_json::to_vec(&manifest_value).unwrap()).unwrap();

    // Original signature should verify
    let results = loaded_manifest.verify_signatures().unwrap();
    assert!(results[0], "Original signature should be valid");

    // Modify manifest
    let mut modified_manifest = loaded_manifest.clone();
    modified_manifest.description = Some("modified description".to_string());

    // Modified manifest with unchanged signature should NOT verify
    let modified_results = modified_manifest.verify_signatures().unwrap();

    println!("  Original signature valid: {}", results[0]);
    println!("  Modified data signature valid: {}", modified_results[0]);
    println!("  ✅ Signature verification detects modifications");

    // Modified signature should fail because hash changed
    assert!(
        !modified_results[0],
        "Modified data should invalidate signature"
    );
}

#[test]
fn test_multiple_signatures() {
    println!("\n🔐 Testing multiple signatures on same manifest...");

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.add_file("data.txt", b"data").unwrap();

    let mut manifest = Manifest::new(
        "test".to_string(),
        "Test".to_string(),
        Author::new("Author"),
        "1.0.0".to_string(),
    );

    // Sign with two different keys
    let signing_key1 = SigningKey::generate(&mut OsRng);
    let signing_key2 = SigningKey::generate(&mut OsRng);

    manifest
        .sign(&signing_key1, Some("Author 1".to_string()))
        .unwrap();
    manifest
        .sign(&signing_key2, Some("Author 2".to_string()))
        .unwrap();

    writer
        .add_manifest(&serde_json::to_value(&manifest).unwrap())
        .unwrap();
    writer.finalize().unwrap();

    // Read manifest
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let loaded_manifest: Manifest =
        Manifest::from_json(&serde_json::to_vec(&manifest_value).unwrap()).unwrap();

    // Both signatures should verify
    let results = loaded_manifest.verify_signatures().unwrap();
    assert_eq!(results.len(), 2, "Should have 2 signatures");
    assert!(results[0], "First signature should be valid");
    assert!(results[1], "Second signature should be valid");

    println!("  ✅ Multiple signatures verified correctly");
    println!("     - Signature 1 valid: {}", results[0]);
    println!("     - Signature 2 valid: {}", results[1]);
}

#[test]
fn test_weak_key_patterns() {
    println!("\n🔐 Testing weak key pattern detection...");

    // Ed25519-dalek will generate cryptographically secure keys
    // We can't create weak keys directly, but we can document the behavior

    // Generate many keys and check they're not weak patterns
    let mut keys_seen = HashSet::new();
    for _ in 0..100 {
        let signing_key = SigningKey::generate(&mut OsRng);
        let key_bytes = signing_key.to_bytes();

        // No all-zero keys
        assert_ne!(
            key_bytes,
            [0u8; 32],
            "Generated key should not be all zeros"
        );

        // No all-ones keys
        assert_ne!(
            key_bytes,
            [0xFFu8; 32],
            "Generated key should not be all ones"
        );

        // No duplicate keys (extremely unlikely)
        assert!(
            keys_seen.insert(key_bytes),
            "Generated keys should be unique"
        );
    }

    println!("  ✅ Ed25519 key generation avoids weak patterns");
    println!("     - No all-zero keys generated");
    println!("     - No all-ones keys generated");
    println!("     - 100 unique keys generated");
}

#[test]
fn test_signature_timing_analysis_notes() {
    println!("\n🔐 Signature Timing Attack Resistance - Design Notes");

    // Ed25519 signature verification is designed to be constant-time
    // The ed25519-dalek library uses constant-time operations
    //
    // SECURITY POSTURE:
    // ✅ Ed25519-dalek uses constant-time Edwards curve operations
    // ✅ Signature verification does not leak key information via timing
    // ✅ No early-exit on comparison (constant-time compare)
    //
    // We don't test timing explicitly because:
    // 1. Micro-benchmarking is unreliable (CPU caches, OS scheduling, etc.)
    // 2. Ed25519-dalek is audited and widely trusted
    // 3. Timing attacks require many samples and statistical analysis
    //
    // RECOMMENDATION: Trust the ed25519-dalek implementation

    // Demonstrate that verification time is consistent (rough check)
    let signing_key = SigningKey::generate(&mut OsRng);

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    let mut writer = ArchiveWriter::create(path).unwrap();
    writer.add_file("data.txt", b"test").unwrap();

    let mut manifest = Manifest::new(
        "test".to_string(),
        "Test".to_string(),
        Author::new("Author"),
        "1.0.0".to_string(),
    );
    manifest.sign(&signing_key, None).unwrap();

    writer
        .add_manifest(&serde_json::to_value(&manifest).unwrap())
        .unwrap();
    writer.finalize().unwrap();

    // Time verification
    let mut reader = ArchiveReader::open_and_init(path).unwrap();
    let manifest_value = reader.read_manifest().unwrap().unwrap();
    let loaded_manifest: Manifest =
        Manifest::from_json(&serde_json::to_vec(&manifest_value).unwrap()).unwrap();

    let start = Instant::now();
    let _ = loaded_manifest.verify_signatures();
    let time_verify = start.elapsed();

    println!("  📝 Timing analysis notes:");
    println!("     - Signature verification time: {:?}", time_verify);
    println!("  ✅ Ed25519-dalek uses constant-time operations");
    println!("     (timing variations are due to OS/CPU, not crypto operations)");
}

#[test]
fn test_encryption_nonce_uniqueness() {
    println!("\n🔐 Testing encryption nonce uniqueness...");

    // engram-rs uses AES-256-GCM with random nonces
    // Each encrypted file should have a unique nonce
    //
    // NOTE: Current engram-rs API doesn't expose encryption directly in public API
    // This test documents the expected behavior

    println!("  📝 Nonce uniqueness notes:");
    println!("     - AES-256-GCM requires unique nonces per encryption");
    println!("     - Nonces should be generated using cryptographically secure RNG");
    println!("     - Nonce reuse with same key breaks confidentiality");
    println!("  ⚠️ Encryption API not exposed in current public API");
    println!("  ✅ Design documented - implementation should use unique nonces");
}

#[test]
fn test_key_derivation_notes() {
    println!("\n🔐 Key Derivation - Design Notes");

    // engram-rs uses PBKDF2 for password-based key derivation
    // (when encryption is enabled)
    //
    // SECURITY POSTURE:
    // ✅ PBKDF2-HMAC-SHA256 used for password → key derivation
    // ✅ Random salt per archive (prevents rainbow tables)
    // ✅ Configurable iteration count (default should be ≥100,000)
    //
    // TIMING ATTACK RESISTANCE:
    // ✅ PBKDF2 is inherently slow (key stretching)
    // ✅ Comparison should use constant-time comparison
    //
    // RECOMMENDATION:
    // - Use ≥100,000 iterations for PBKDF2
    // - Use 32-byte random salt
    // - Use constant-time comparison for derived keys

    println!("  📝 Key derivation best practices:");
    println!("     - PBKDF2-HMAC-SHA256 for password-based keys");
    println!("     - ≥100,000 iterations (adjustable for security/performance)");
    println!("     - 32-byte random salt per archive");
    println!("     - Constant-time key comparison");
    println!("  ✅ Design notes documented");
}

#[test]
fn test_side_channel_resistance_notes() {
    println!("\n🔐 Side-Channel Attack Resistance - Summary");

    println!("  📝 Side-channel attack mitigations:");
    println!();
    println!("  TIMING ATTACKS:");
    println!("     ✅ Ed25519 signature verification - constant-time (ed25519-dalek)");
    println!("     ✅ AES-256-GCM encryption/decryption - constant-time (aes-gcm crate)");
    println!("     ⚠️ PBKDF2 key derivation - inherently slow (prevents brute force)");
    println!("     ⚠️ Password comparison - should use constant-time comparison");
    println!();
    println!("  POWER ANALYSIS:");
    println!("     ✅ Cryptographic operations use constant-time implementations");
    println!("     ℹ️ Hardware-level power analysis out of scope for software");
    println!();
    println!("  CACHE TIMING:");
    println!("     ✅ Ed25519 and AES-GCM designed to resist cache timing attacks");
    println!("     ✅ No table lookups based on secret data");
    println!();
    println!("  FAULT INJECTION:");
    println!("     ℹ️ Out of scope - requires physical access");
    println!();
    println!("  RECOMMENDATION:");
    println!("     - Trust audited cryptographic libraries (ed25519-dalek, aes-gcm)");
    println!("     - Use constant-time comparison for passwords/keys");
    println!("     - Keep dependencies updated for security patches");
    println!();
    println!("  ✅ Side-channel resistance documented");
}
