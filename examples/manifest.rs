/// Example demonstrating manifests and Ed25519 signatures
///
/// Run with: cargo run --example manifest

use engram_rs::{ArchiveWriter, ArchiveReader, Manifest, Author, Metadata};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Engram-rs Manifest & Signatures Example ===\n");

    // Generate keypair
    println!("1. Generating Ed25519 keypair...");
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    println!("   ✓ Keypair generated");
    println!("   Public key: {}", hex::encode(verifying_key.as_bytes()));

    // Create archive with manifest
    println!("\n2. Creating archive with manifest...");
    create_signed_archive(&signing_key)?;

    // Verify signature
    println!("\n3. Verifying signature...");
    verify_archive()?;

    println!("\n✓ Example complete!");
    Ok(())
}

fn create_signed_archive(signing_key: &SigningKey) -> Result<(), Box<dyn Error>> {
    let mut writer = ArchiveWriter::create("example_manifest.eng")?;

    // Add some files
    writer.add_file("important.txt", b"This data is cryptographically signed.")?;
    writer.add_file("data.json", br#"{"secure": true}"#)?;

    // Create manifest
    let mut manifest = Manifest {
        version: "0.4.0".to_string(),
        id: "signed-example".to_string(),
        name: "Signed Example Archive".to_string(),
        description: Some("An example of a signed Engram archive".to_string()),
        author: Author {
            name: "Example Author".to_string(),
            email: Some("author@example.com".to_string()),
            url: Some("https://example.com".to_string()),
        },
        metadata: Metadata {
            version: "1.0.0".to_string(),
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            modified: None,
            license: Some("MIT".to_string()),
            tags: vec!["example".to_string(), "signed".to_string()],
        },
        capabilities: vec!["read".to_string()],
        files: vec![],
        signatures: vec![],
    };

    // Sign the manifest
    manifest.sign(signing_key, Some("Example Author".to_string()))?;
    println!("   ✓ Manifest signed");

    // Write manifest to archive
    writer.add_manifest(&serde_json::to_value(&manifest)?)?;
    println!("   ✓ Manifest written");

    writer.finalize()?;
    println!("   ✓ Archive created: example_manifest.eng");

    Ok(())
}

fn verify_archive() -> Result<(), Box<dyn Error>> {
    let mut reader = ArchiveReader::open("example_manifest.eng")?;
    reader.initialize()?;

    // Read manifest
    if let Some(manifest_value) = reader.read_manifest()? {
        let manifest: Manifest = serde_json::from_value(manifest_value)?;

        println!("   Archive info:");
        println!("     ID: {}", manifest.id);
        println!("     Name: {}", manifest.name);
        println!("     Author: {}", manifest.author.name);
        println!("     Signatures: {}", manifest.signatures.len());

        // Verify signatures
        let results = manifest.verify_signatures()?;
        let all_valid = results.iter().all(|&v| v);

        if all_valid && !results.is_empty() {
            println!("\n   ✓ Signature verification: VALID ({} signature(s))", results.len());
        } else if results.is_empty() {
            println!("\n   ⚠ No signatures to verify");
        } else {
            println!("\n   ✗ Signature verification: INVALID");
        }
    } else {
        println!("   ✗ No manifest found");
    }

    Ok(())
}
