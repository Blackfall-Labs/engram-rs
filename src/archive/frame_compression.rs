use crate::error::{EngramError, Result};
use crate::archive::format::CompressionMethod;
use std::io::{Read, Write};

/// Frame size for frame-based compression (64KB)
pub const FRAME_SIZE: usize = 65536; // 64KB

/// Minimum file size for frame-based compression (50MB)
pub const MIN_FRAME_COMPRESSION_SIZE: usize = 52_428_800; // 50MB

/// Compress data using frame-based compression
///
/// Each frame is compressed independently, allowing partial decompression.
/// Format: [frame_count: uint32][frame1_size: uint32][frame1_data][frame2_size: uint32][frame2_data]...
///
/// # Arguments
/// * `data` - Input data to compress
/// * `method` - Compression method (LZ4 or Zstd)
///
/// # Returns
/// Compressed data with frame headers
pub fn compress_frames(data: &[u8], method: CompressionMethod) -> Result<Vec<u8>> {
    if data.len() < MIN_FRAME_COMPRESSION_SIZE {
        return Err(EngramError::InvalidFormat(
            "File too small for frame compression".to_string(),
        ));
    }

    // Calculate number of frames
    let frame_count = data.len().div_ceil(FRAME_SIZE);
    let mut output = Vec::new();

    // Write frame count
    output.write_all(&(frame_count as u32).to_le_bytes())?;

    // Compress each frame
    for frame_idx in 0..frame_count {
        let start = frame_idx * FRAME_SIZE;
        let end = std::cmp::min(start + FRAME_SIZE, data.len());
        let frame_data = &data[start..end];

        // Compress frame
        let compressed_frame = match method {
            CompressionMethod::Lz4 => compress_lz4_frame(frame_data)?,
            CompressionMethod::Zstd => compress_zstd_frame(frame_data)?,
            CompressionMethod::None => {
                return Err(EngramError::InvalidFormat(
                    "Frame compression requires LZ4 or Zstd".to_string(),
                ));
            }
        };

        // Write frame size and data
        output.write_all(&(compressed_frame.len() as u32).to_le_bytes())?;
        output.write_all(&compressed_frame)?;
    }

    Ok(output)
}

/// Decompress frame-based compressed data
///
/// # Arguments
/// * `data` - Frame-compressed data with headers
/// * `method` - Compression method used
/// * `expected_size` - Expected uncompressed size for validation
///
/// # Returns
/// Decompressed data
pub fn decompress_frames(data: &[u8], method: CompressionMethod, expected_size: u64) -> Result<Vec<u8>> {
    let mut cursor = std::io::Cursor::new(data);
    let mut output = Vec::with_capacity(expected_size as usize);

    // Read frame count
    let mut frame_count_bytes = [0u8; 4];
    cursor.read_exact(&mut frame_count_bytes)?;
    let frame_count = u32::from_le_bytes(frame_count_bytes);

    // Decompress each frame
    for _ in 0..frame_count {
        // Read frame size
        let mut frame_size_bytes = [0u8; 4];
        cursor.read_exact(&mut frame_size_bytes)?;
        let frame_size = u32::from_le_bytes(frame_size_bytes) as usize;

        // Read compressed frame data
        let mut frame_data = vec![0u8; frame_size];
        cursor.read_exact(&mut frame_data)?;

        // Decompress frame
        let decompressed_frame = match method {
            CompressionMethod::Lz4 => decompress_lz4_frame(&frame_data)?,
            CompressionMethod::Zstd => decompress_zstd_frame(&frame_data)?,
            CompressionMethod::None => {
                return Err(EngramError::InvalidFormat(
                    "Frame compression requires LZ4 or Zstd".to_string(),
                ));
            }
        };

        output.extend_from_slice(&decompressed_frame);
    }

    // Validate size
    if output.len() != expected_size as usize {
        return Err(EngramError::DecompressionFailed(format!(
            "Frame decompression size mismatch: expected {}, got {}",
            expected_size,
            output.len()
        )));
    }

    Ok(output)
}

/// Compress a single frame with LZ4
fn compress_lz4_frame(data: &[u8]) -> Result<Vec<u8>> {
    Ok(lz4_flex::compress_prepend_size(data))
}

/// Compress a single frame with Zstd
fn compress_zstd_frame(data: &[u8]) -> Result<Vec<u8>> {
    zstd::encode_all(data, 6)
        .map_err(|e| EngramError::CompressionFailed(format!("Zstd frame compression failed: {}", e)))
}

/// Decompress a single LZ4 frame
fn decompress_lz4_frame(data: &[u8]) -> Result<Vec<u8>> {
    lz4_flex::decompress_size_prepended(data).map_err(|e| {
        EngramError::DecompressionFailed(format!("LZ4 frame decompression failed: {}", e))
    })
}

/// Decompress a single Zstd frame
fn decompress_zstd_frame(data: &[u8]) -> Result<Vec<u8>> {
    zstd::decode_all(data)
        .map_err(|e| EngramError::DecompressionFailed(format!("Zstd frame decompression failed: {}", e)))
}

/// Check if a file should use frame-based compression
pub fn should_use_frames(size: usize) -> bool {
    size >= MIN_FRAME_COMPRESSION_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_compression_lz4() {
        // Create 60MB of test data
        let size = 60 * 1024 * 1024;
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

        // Compress with frames
        let compressed = compress_frames(&data, CompressionMethod::Lz4).unwrap();

        // Should be significantly smaller
        assert!(compressed.len() < data.len());

        // Decompress
        let decompressed = decompress_frames(&compressed, CompressionMethod::Lz4, data.len() as u64).unwrap();

        // Verify exact match
        assert_eq!(decompressed.len(), data.len());
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_frame_compression_zstd() {
        // Create 60MB of test data (highly compressible pattern)
        let size = 60 * 1024 * 1024;
        let data: Vec<u8> = vec![42u8; size];

        // Compress with frames
        let compressed = compress_frames(&data, CompressionMethod::Zstd).unwrap();

        // Should be VERY small due to pattern
        assert!(compressed.len() < size / 100);

        // Decompress
        let decompressed = decompress_frames(&compressed, CompressionMethod::Zstd, data.len() as u64).unwrap();

        // Verify exact match
        assert_eq!(decompressed.len(), data.len());
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_should_use_frames() {
        assert!(!should_use_frames(10 * 1024 * 1024)); // 10MB - no
        assert!(!should_use_frames(40 * 1024 * 1024)); // 40MB - no
        assert!(should_use_frames(60 * 1024 * 1024));  // 60MB - yes
        assert!(should_use_frames(100 * 1024 * 1024)); // 100MB - yes
    }

    #[test]
    fn test_frame_size_calculation() {
        // Test that frames are correctly sized
        let size: usize = 60 * 1024 * 1024;
        let frame_count = size.div_ceil(FRAME_SIZE);

        // Should be approximately 960 frames for 60MB
        assert_eq!(frame_count, 960);
    }

    #[test]
    fn test_small_file_error() {
        let small_data = vec![0u8; 1024]; // 1KB
        let result = compress_frames(&small_data, CompressionMethod::Lz4);
        assert!(result.is_err());
    }
}
