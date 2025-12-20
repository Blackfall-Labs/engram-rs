mod end_record;
mod format;
mod frame_compression;
mod local_entry;
mod reader;
mod writer;

pub use end_record::{EndRecord, END_RECORD_SIGNATURE, END_RECORD_SIZE};
pub use format::{
    CompressionMethod, EntryInfo, FileHeader, CD_ENTRY_SIZE, FORMAT_VERSION_MAJOR,
    FORMAT_VERSION_MINOR, HEADER_SIZE, MAGIC_NUMBER, MAX_PATH_LENGTH,
};
pub use frame_compression::{
    compress_frames, decompress_frames, should_use_frames, FRAME_SIZE,
    MIN_FRAME_COMPRESSION_SIZE,
};
pub use local_entry::{LocalEntryHeader, LOCAL_ENTRY_SIGNATURE};
pub use reader::ArchiveReader;
pub use writer::ArchiveWriter;
