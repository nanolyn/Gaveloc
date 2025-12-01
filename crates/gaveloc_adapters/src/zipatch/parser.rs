//! ZiPatch file parser
//!
//! Parses ZiPatch binary files and extracts chunks for patch application.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use byteorder::{BigEndian, ReadBytesExt};
use crc32fast::Hasher;
use tracing::instrument;

use gaveloc_core::error::Error;
use gaveloc_core::ports::ZiPatchApplier;
use gaveloc_core::zipatch::*;

/// ZiPatch file parser
pub struct ZiPatchParser {
    /// Whether to verify CRC32 checksums on chunks
    verify_checksums: bool,
}

impl ZiPatchParser {
    pub fn new() -> Self {
        Self {
            verify_checksums: true,
        }
    }

    /// Create a parser that skips checksum verification (faster but less safe)
    pub fn without_checksum_verification() -> Self {
        Self {
            verify_checksums: false,
        }
    }

    /// Read a fixed-length string from the reader
    fn read_string(reader: &mut impl Read, len: usize) -> Result<String, Error> {
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        Ok(String::from_utf8_lossy(&buf)
            .trim_end_matches('\0')
            .to_string())
    }

    /// Read the magic header and verify it
    fn read_magic_header(reader: &mut impl Read) -> Result<(), Error> {
        let mut magic = [0u8; 12];
        reader.read_exact(&mut magic)?;

        if magic != ZIPATCH_MAGIC {
            return Err(Error::ZiPatchInvalidMagic);
        }

        Ok(())
    }

    /// Read a single chunk from the reader
    fn read_chunk(&self, reader: &mut BufReader<File>) -> Result<ZiPatchChunk, Error> {
        let offset = reader.stream_position()?;

        // Read chunk size (4 bytes, big-endian)
        let size = reader.read_u32::<BigEndian>()?;

        // Read chunk type (4 ASCII bytes)
        let chunk_type = Self::read_string(reader, 4)?;

        // Calculate start of chunk data
        let data_start = reader.stream_position()?;
        let data_size = size as usize;

        // If verifying checksums, we need to read all data
        let mut crc_hasher = if self.verify_checksums {
            let mut h = Hasher::new();
            // Include chunk type in CRC
            h.update(chunk_type.as_bytes());
            Some(h)
        } else {
            None
        };

        // Parse chunk based on type
        let chunk = match chunk_type.as_str() {
            "FHDR" => self.parse_file_header(reader, offset, size, &mut crc_hasher)?,
            "APLY" => self.parse_apply_option(reader, offset, &mut crc_hasher)?,
            "ADIR" => self.parse_add_directory(reader, offset, size, &mut crc_hasher)?,
            "DELD" => self.parse_delete_directory(reader, offset, size, &mut crc_hasher)?,
            "APFS" => self.parse_apply_free_space(reader, offset, &mut crc_hasher)?,
            "SQPK" => self.parse_sqpk_chunk(reader, offset, size, &mut crc_hasher)?,
            "EOF_" => {
                // Advance past any remaining data
                if let Some(ref mut hasher) = crc_hasher {
                    let mut buf = vec![0u8; data_size.saturating_sub(4)];
                    if !buf.is_empty() {
                        reader.read_exact(&mut buf)?;
                        hasher.update(&buf);
                    }
                } else {
                    let remaining = data_size.saturating_sub(4);
                    if remaining > 0 {
                        reader.seek(SeekFrom::Current(remaining as i64))?;
                    }
                }
                ZiPatchChunk::EndOfFile
            }
            _ => {
                // Unknown chunk - skip it
                tracing::warn!("Unknown chunk type: {}", chunk_type);
                if let Some(ref mut hasher) = crc_hasher {
                    let mut buf = vec![0u8; data_size.saturating_sub(4)];
                    if !buf.is_empty() {
                        reader.read_exact(&mut buf)?;
                        hasher.update(&buf);
                    }
                } else {
                    let remaining = data_size.saturating_sub(4);
                    if remaining > 0 {
                        reader.seek(SeekFrom::Current(remaining as i64))?;
                    }
                }
                ZiPatchChunk::Unknown {
                    chunk_type,
                    offset,
                    size,
                }
            }
        };

        // Ensure we're at the right position for CRC
        let expected_crc_pos = data_start + data_size as u64;
        let current_pos = reader.stream_position()?;
        if current_pos < expected_crc_pos {
            let skip = expected_crc_pos - current_pos;
            if let Some(ref mut hasher) = crc_hasher {
                let mut buf = vec![0u8; skip as usize];
                reader.read_exact(&mut buf)?;
                hasher.update(&buf);
            } else {
                reader.seek(SeekFrom::Start(expected_crc_pos))?;
            }
        }

        // Read and verify CRC32
        let stored_crc = reader.read_u32::<BigEndian>()?;

        if let Some(hasher) = crc_hasher {
            let computed_crc = hasher.finalize();
            if computed_crc != stored_crc {
                return Err(Error::ZiPatchChecksumMismatch { offset });
            }
        }

        Ok(chunk)
    }

    /// Parse file header chunk
    fn parse_file_header(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        size: u32,
        hasher: &mut Option<Hasher>,
    ) -> Result<ZiPatchChunk, Error> {
        // Version (2 bytes) + pad (2 bytes) + patch type (4 bytes) + entry files (4 bytes)
        let version = reader.read_u16::<BigEndian>()?;
        let _pad = reader.read_u16::<BigEndian>()?;
        let patch_type = Self::read_string(reader, 4)?;
        let entry_files = reader.read_u32::<BigEndian>()?;

        if let Some(ref mut h) = hasher {
            let mut buf = Vec::new();
            buf.extend_from_slice(&version.to_be_bytes());
            buf.extend_from_slice(&_pad.to_be_bytes());
            buf.extend_from_slice(patch_type.as_bytes());
            // Pad to 4 bytes if needed
            let pad_len = 4usize.saturating_sub(patch_type.len());
            buf.extend(std::iter::repeat(0u8).take(pad_len));
            buf.extend_from_slice(&entry_files.to_be_bytes());
            h.update(&buf);
        }

        // Skip remaining header data
        let read_so_far = 12; // 2 + 2 + 4 + 4
        let remaining = (size as usize).saturating_sub(read_so_far + 4); // +4 for chunk type already read
        if remaining > 0 {
            if let Some(ref mut h) = hasher {
                let mut buf = vec![0u8; remaining];
                reader.read_exact(&mut buf)?;
                h.update(&buf);
            } else {
                reader.seek(SeekFrom::Current(remaining as i64))?;
            }
        }

        Ok(ZiPatchChunk::FileHeader(FileHeaderChunk {
            version,
            patch_type,
            entry_files,
            offset,
        }))
    }

    /// Parse apply option chunk
    fn parse_apply_option(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        hasher: &mut Option<Hasher>,
    ) -> Result<ZiPatchChunk, Error> {
        let option = reader.read_u32::<BigEndian>()?;
        let value = reader.read_u32::<BigEndian>()?;

        if let Some(ref mut h) = hasher {
            h.update(&option.to_be_bytes());
            h.update(&value.to_be_bytes());
        }

        // Skip padding (4 bytes)
        if let Some(ref mut h) = hasher {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            h.update(&buf);
        } else {
            reader.seek(SeekFrom::Current(4))?;
        }

        Ok(ZiPatchChunk::ApplyOption(ApplyOptionChunk {
            option: ApplyOption::from(option),
            value,
            offset,
        }))
    }

    /// Parse add directory chunk
    fn parse_add_directory(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        size: u32,
        hasher: &mut Option<Hasher>,
    ) -> Result<ZiPatchChunk, Error> {
        // Directory name length is size - 4 (chunk type)
        let path_len = (size as usize).saturating_sub(4);
        let path = Self::read_string(reader, path_len)?;

        if let Some(ref mut h) = hasher {
            let mut buf = vec![0u8; path_len];
            buf[..path.len()].copy_from_slice(path.as_bytes());
            h.update(&buf);
        }

        Ok(ZiPatchChunk::AddDirectory(AddDirectoryChunk { path, offset }))
    }

    /// Parse delete directory chunk
    fn parse_delete_directory(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        size: u32,
        hasher: &mut Option<Hasher>,
    ) -> Result<ZiPatchChunk, Error> {
        let path_len = (size as usize).saturating_sub(4);
        let path = Self::read_string(reader, path_len)?;

        if let Some(ref mut h) = hasher {
            let mut buf = vec![0u8; path_len];
            buf[..path.len()].copy_from_slice(path.as_bytes());
            h.update(&buf);
        }

        Ok(ZiPatchChunk::DeleteDirectory(DeleteDirectoryChunk {
            path,
            offset,
        }))
    }

    /// Parse apply free space chunk
    fn parse_apply_free_space(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        hasher: &mut Option<Hasher>,
    ) -> Result<ZiPatchChunk, Error> {
        let alloc_size = reader.read_u64::<BigEndian>()?;

        if let Some(ref mut h) = hasher {
            h.update(&alloc_size.to_be_bytes());
        }

        Ok(ZiPatchChunk::ApplyFreeSpace(ApplyFreeSpaceChunk {
            alloc_size,
            offset,
        }))
    }

    /// Parse SQPK (SqPack) command chunk
    fn parse_sqpk_chunk(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        _outer_size: u32,
        hasher: &mut Option<Hasher>,
    ) -> Result<ZiPatchChunk, Error> {
        // Inner size (should match outer)
        let inner_size = reader.read_i32::<BigEndian>()?;
        if let Some(ref mut h) = hasher {
            h.update(&inner_size.to_be_bytes());
        }

        // Command type (1 byte)
        let mut cmd_buf = [0u8; 1];
        reader.read_exact(&mut cmd_buf)?;
        if let Some(ref mut h) = hasher {
            h.update(&cmd_buf);
        }
        let command = String::from_utf8_lossy(&cmd_buf).to_string();

        // Data size is inner_size - 5 (size:4 + command:1)
        let data_size = (inner_size as usize).saturating_sub(5);

        let sqpk = match command.as_str() {
            "A" => self.parse_sqpk_add_data(reader, offset, data_size, hasher)?,
            "D" => self.parse_sqpk_delete_data(reader, offset, hasher)?,
            "E" => self.parse_sqpk_expand_data(reader, offset, hasher)?,
            "H" => self.parse_sqpk_header(reader, offset, data_size, hasher)?,
            "I" => self.parse_sqpk_index(reader, offset, data_size, hasher)?,
            "F" => self.parse_sqpk_file(reader, offset, data_size, hasher)?,
            "X" => self.parse_sqpk_patch_info(reader, offset, hasher)?,
            "T" => self.parse_sqpk_target_info(reader, offset, hasher)?,
            _ => {
                tracing::warn!("Unknown SQPK command: {}", command);
                // Skip the data
                if let Some(ref mut h) = hasher {
                    let mut buf = vec![0u8; data_size];
                    reader.read_exact(&mut buf)?;
                    h.update(&buf);
                } else {
                    reader.seek(SeekFrom::Current(data_size as i64))?;
                }
                SqpkChunk::Unknown { command, offset }
            }
        };

        Ok(ZiPatchChunk::Sqpk(sqpk))
    }

    /// Parse SqPack target file specification (common to many commands)
    fn parse_sqpack_file_target(
        &self,
        reader: &mut BufReader<File>,
        hasher: &mut Option<Hasher>,
    ) -> Result<SqpackFileTarget, Error> {
        let main_id = reader.read_u16::<BigEndian>()?;
        let sub_id = reader.read_u16::<BigEndian>()?;
        let file_id = reader.read_u32::<BigEndian>()?;

        if let Some(ref mut h) = hasher {
            h.update(&main_id.to_be_bytes());
            h.update(&sub_id.to_be_bytes());
            h.update(&file_id.to_be_bytes());
        }

        Ok(SqpackFileTarget {
            main_id,
            sub_id,
            file_id,
        })
    }

    /// Parse SQPK Add Data command
    fn parse_sqpk_add_data(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        data_size: usize,
        hasher: &mut Option<Hasher>,
    ) -> Result<SqpkChunk, Error> {
        // 3 bytes alignment
        let mut align = [0u8; 3];
        reader.read_exact(&mut align)?;
        if let Some(ref mut h) = hasher {
            h.update(&align);
        }

        // Target file (8 bytes)
        let target_file = self.parse_sqpack_file_target(reader, hasher)?;

        // Block offset, number, delete number (4 bytes each, shifted left by 7)
        let block_offset = (reader.read_u32::<BigEndian>()? as u64) << 7;
        let block_number = (reader.read_u32::<BigEndian>()? as u64) << 7;
        let block_delete_number = (reader.read_u32::<BigEndian>()? as u64) << 7;

        if let Some(ref mut h) = hasher {
            h.update(&((block_offset >> 7) as u32).to_be_bytes());
            h.update(&((block_number >> 7) as u32).to_be_bytes());
            h.update(&((block_delete_number >> 7) as u32).to_be_bytes());
        }

        // Block data
        let data_source_offset = reader.stream_position()?;
        let block_data_len = block_number as usize;

        // Calculate how much we've read so far: 3 + 8 + 12 = 23
        // Data should be remaining
        let expected_data_len = data_size.saturating_sub(23);
        let actual_read_len = block_data_len.min(expected_data_len);

        let mut block_data = vec![0u8; actual_read_len];
        reader.read_exact(&mut block_data)?;

        if let Some(ref mut h) = hasher {
            h.update(&block_data);
        }

        Ok(SqpkChunk::AddData(SqpkAddData {
            target_file,
            block_offset,
            block_number,
            block_delete_number,
            block_data,
            data_source_offset,
            offset,
        }))
    }

    /// Parse SQPK Delete Data command
    fn parse_sqpk_delete_data(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        hasher: &mut Option<Hasher>,
    ) -> Result<SqpkChunk, Error> {
        // 3 bytes alignment
        let mut align = [0u8; 3];
        reader.read_exact(&mut align)?;
        if let Some(ref mut h) = hasher {
            h.update(&align);
        }

        let target_file = self.parse_sqpack_file_target(reader, hasher)?;

        let block_offset = (reader.read_u32::<BigEndian>()? as u64) << 7;
        let block_number = (reader.read_u32::<BigEndian>()? as u64) << 7;

        if let Some(ref mut h) = hasher {
            h.update(&((block_offset >> 7) as u32).to_be_bytes());
            h.update(&((block_number >> 7) as u32).to_be_bytes());
        }

        // 4 bytes padding
        let mut pad = [0u8; 4];
        reader.read_exact(&mut pad)?;
        if let Some(ref mut h) = hasher {
            h.update(&pad);
        }

        Ok(SqpkChunk::DeleteData(SqpkDeleteData {
            target_file,
            block_offset,
            block_number,
            offset,
        }))
    }

    /// Parse SQPK Expand Data command
    fn parse_sqpk_expand_data(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        hasher: &mut Option<Hasher>,
    ) -> Result<SqpkChunk, Error> {
        // Same structure as delete data
        let mut align = [0u8; 3];
        reader.read_exact(&mut align)?;
        if let Some(ref mut h) = hasher {
            h.update(&align);
        }

        let target_file = self.parse_sqpack_file_target(reader, hasher)?;

        let block_offset = (reader.read_u32::<BigEndian>()? as u64) << 7;
        let block_number = (reader.read_u32::<BigEndian>()? as u64) << 7;

        if let Some(ref mut h) = hasher {
            h.update(&((block_offset >> 7) as u32).to_be_bytes());
            h.update(&((block_number >> 7) as u32).to_be_bytes());
        }

        let mut pad = [0u8; 4];
        reader.read_exact(&mut pad)?;
        if let Some(ref mut h) = hasher {
            h.update(&pad);
        }

        Ok(SqpkChunk::ExpandData(SqpkExpandData {
            target_file,
            block_offset,
            block_number,
            offset,
        }))
    }

    /// Parse SQPK Header command
    fn parse_sqpk_header(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        data_size: usize,
        hasher: &mut Option<Hasher>,
    ) -> Result<SqpkChunk, Error> {
        // 3 bytes alignment
        let mut align = [0u8; 3];
        reader.read_exact(&mut align)?;
        if let Some(ref mut h) = hasher {
            h.update(&align);
        }

        let file_kind = reader.read_u8()?;
        let header_kind = reader.read_u8()?;
        let _pad = reader.read_u8()?;

        if let Some(ref mut h) = hasher {
            h.update(&[file_kind, header_kind, _pad]);
        }

        let target_file = self.parse_sqpack_file_target(reader, hasher)?;

        // Header data is remaining bytes
        let header_len = data_size.saturating_sub(14); // 3 + 3 + 8
        let mut header_data = vec![0u8; header_len];
        reader.read_exact(&mut header_data)?;

        if let Some(ref mut h) = hasher {
            h.update(&header_data);
        }

        Ok(SqpkChunk::Header(SqpkHeader {
            file_kind: SqpkFileKind::from(file_kind),
            header_kind: SqpkHeaderKind::from(header_kind),
            target_file,
            header_data,
            offset,
        }))
    }

    /// Parse SQPK Index command
    fn parse_sqpk_index(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        data_size: usize,
        hasher: &mut Option<Hasher>,
    ) -> Result<SqpkChunk, Error> {
        // 3 bytes alignment
        let mut align = [0u8; 3];
        reader.read_exact(&mut align)?;
        if let Some(ref mut h) = hasher {
            h.update(&align);
        }

        let index_cmd = reader.read_u8()?;
        let is_synonym = (index_cmd & 0x10) != 0;
        let index_type = if (index_cmd & 0x0F) == 0 {
            IndexType::Index
        } else {
            IndexType::Index2
        };

        if let Some(ref mut h) = hasher {
            h.update(&[index_cmd]);
        }

        // 2 bytes padding
        let mut pad = [0u8; 2];
        reader.read_exact(&mut pad)?;
        if let Some(ref mut h) = hasher {
            h.update(&pad);
        }

        let target_file = self.parse_sqpack_file_target(reader, hasher)?;

        // Index entries
        let entries_size = data_size.saturating_sub(14); // 3 + 1 + 2 + 8
        let entry_count = entries_size / 16; // Each entry is 16 bytes

        let mut index_data = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let file_hash = reader.read_u64::<BigEndian>()?;
            let block_offset = reader.read_u32::<BigEndian>()?;
            let block_number = reader.read_u32::<BigEndian>()?;

            if let Some(ref mut h) = hasher {
                h.update(&file_hash.to_be_bytes());
                h.update(&block_offset.to_be_bytes());
                h.update(&block_number.to_be_bytes());
            }

            index_data.push(SqpkIndexData {
                file_hash,
                block_offset,
                block_number,
            });
        }

        Ok(SqpkChunk::Index(SqpkIndex {
            index_type,
            is_synonym,
            target_file,
            index_data,
            offset,
        }))
    }

    /// Parse SQPK File command
    fn parse_sqpk_file(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        data_size: usize,
        hasher: &mut Option<Hasher>,
    ) -> Result<SqpkChunk, Error> {
        // 3 bytes alignment
        let mut align = [0u8; 3];
        reader.read_exact(&mut align)?;
        if let Some(ref mut h) = hasher {
            h.update(&align);
        }

        let operation = reader.read_u8()?;
        let _pad = reader.read_u8()?;
        let expansion_id = reader.read_u8()?;

        if let Some(ref mut h) = hasher {
            h.update(&[operation, _pad, expansion_id]);
        }

        // File path length (4 bytes)
        let path_len = reader.read_u32::<BigEndian>()?;
        if let Some(ref mut h) = hasher {
            h.update(&path_len.to_be_bytes());
        }

        // File path
        let file_path = Self::read_string(reader, path_len as usize)?;
        if let Some(ref mut h) = hasher {
            let mut buf = vec![0u8; path_len as usize];
            buf[..file_path.len()].copy_from_slice(file_path.as_bytes());
            h.update(&buf);
        }

        // File data (remaining bytes)
        let header_len = 3 + 3 + 4 + path_len as usize;
        let file_data_len = data_size.saturating_sub(header_len);
        let mut file_data = vec![0u8; file_data_len];
        reader.read_exact(&mut file_data)?;

        if let Some(ref mut h) = hasher {
            h.update(&file_data);
        }

        Ok(SqpkChunk::File(SqpkFile {
            operation: SqpkFileOperation::from(operation),
            expansion_id,
            file_path,
            file_data,
            offset,
        }))
    }

    /// Parse SQPK PatchInfo command
    fn parse_sqpk_patch_info(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        hasher: &mut Option<Hasher>,
    ) -> Result<SqpkChunk, Error> {
        // 3 bytes alignment
        let mut align = [0u8; 3];
        reader.read_exact(&mut align)?;
        if let Some(ref mut h) = hasher {
            h.update(&align);
        }

        let status = reader.read_u8()?;
        let version = reader.read_u8()?;
        let _pad = reader.read_u8()?;
        let _pad2 = reader.read_u8()?;
        let install_size = reader.read_u64::<BigEndian>()?;

        if let Some(ref mut h) = hasher {
            h.update(&[status, version, _pad, _pad2]);
            h.update(&install_size.to_be_bytes());
        }

        Ok(SqpkChunk::PatchInfo(SqpkPatchInfo {
            status,
            version,
            install_size,
            offset,
        }))
    }

    /// Parse SQPK TargetInfo command
    fn parse_sqpk_target_info(
        &self,
        reader: &mut BufReader<File>,
        offset: u64,
        hasher: &mut Option<Hasher>,
    ) -> Result<SqpkChunk, Error> {
        // 3 bytes alignment + padding
        let mut header = [0u8; 3];
        reader.read_exact(&mut header)?;
        if let Some(ref mut h) = hasher {
            h.update(&header);
        }

        let platform_byte = reader.read_u8()?;
        let platform = match platform_byte {
            0 => Platform::Win32,
            1 => Platform::Ps3,
            2 => Platform::Ps4,
            _ => Platform::Win32,
        };

        let region = reader.read_u16::<BigEndian>()?;
        let is_debug = reader.read_u8()? != 0;
        let version = reader.read_u16::<BigEndian>()?;

        if let Some(ref mut h) = hasher {
            h.update(&[platform_byte]);
            h.update(&region.to_be_bytes());
            h.update(&[if is_debug { 1 } else { 0 }]);
            h.update(&version.to_be_bytes());
        }

        // Padding
        let mut pad = [0u8; 1];
        reader.read_exact(&mut pad)?;
        if let Some(ref mut h) = hasher {
            h.update(&pad);
        }

        let deleted_data_size = reader.read_u64::<BigEndian>()?;
        let seek_count = reader.read_u64::<BigEndian>()?;

        if let Some(ref mut h) = hasher {
            h.update(&deleted_data_size.to_be_bytes());
            h.update(&seek_count.to_be_bytes());
        }

        Ok(SqpkChunk::TargetInfo(SqpkTargetInfo {
            platform,
            region,
            is_debug,
            version,
            deleted_data_size,
            seek_count,
            offset,
        }))
    }
}

impl Default for ZiPatchParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ZiPatchApplier for ZiPatchParser {
    #[instrument(skip(self))]
    fn parse_patch(&self, patch_path: &Path) -> Result<Vec<ZiPatchChunk>, Error> {
        let file = File::open(patch_path)?;
        let mut reader = BufReader::new(file);

        // Read and verify magic header
        Self::read_magic_header(&mut reader)?;

        // Read all chunks
        let mut chunks = Vec::new();
        loop {
            let chunk = self.read_chunk(&mut reader)?;
            let is_eof = matches!(chunk, ZiPatchChunk::EndOfFile);
            chunks.push(chunk);

            if is_eof {
                break;
            }
        }

        Ok(chunks)
    }

    #[instrument(skip(self))]
    fn apply_patch(&self, patch_path: &Path, game_path: &Path) -> Result<(), Error> {
        let chunks = self.parse_patch(patch_path)?;

        tracing::info!(
            "Applying patch with {} chunks to {:?}",
            chunks.len(),
            game_path
        );

        for chunk in chunks {
            match chunk {
                ZiPatchChunk::FileHeader(fh) => {
                    tracing::debug!(
                        "Patch type: {}, version: {}, files: {}",
                        fh.patch_type,
                        fh.version,
                        fh.entry_files
                    );
                }
                ZiPatchChunk::ApplyOption(opt) => {
                    tracing::debug!("Apply option: {:?} = {}", opt.option, opt.value);
                }
                ZiPatchChunk::AddDirectory(dir) => {
                    let full_path = game_path.join(&dir.path.trim_start_matches('/'));
                    tracing::debug!("Creating directory: {:?}", full_path);
                    std::fs::create_dir_all(&full_path)?;
                }
                ZiPatchChunk::DeleteDirectory(dir) => {
                    let full_path = game_path.join(&dir.path.trim_start_matches('/'));
                    tracing::debug!("Deleting directory: {:?}", full_path);
                    if full_path.exists() {
                        std::fs::remove_dir_all(&full_path)?;
                    }
                }
                ZiPatchChunk::Sqpk(sqpk) => {
                    // TODO: Implement SQPK command application
                    // This requires implementing the SqPack file format handling
                    tracing::debug!("SQPK command: {}", sqpk.command());
                }
                ZiPatchChunk::EndOfFile => {
                    tracing::debug!("End of patch file");
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    // ==========================================================================
    // Test Fixture Helpers
    // ==========================================================================

    /// Compute CRC32 for chunk data (chunk_type + data)
    fn compute_crc32(data: &[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(data);
        hasher.finalize()
    }

    /// Build a complete chunk with size prefix and CRC32 suffix
    /// Format: [size: u32 BE][type: 4 ASCII][data: N bytes][crc32: u32 BE]
    /// Where size = N (data bytes only, NOT including type)
    fn build_chunk(chunk_type: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let size = data.len() as u32;
        let mut chunk = Vec::new();

        // Size (4 bytes, big-endian) - data length only
        chunk.extend_from_slice(&size.to_be_bytes());

        // Chunk type (4 bytes)
        chunk.extend_from_slice(chunk_type);

        // Data
        chunk.extend_from_slice(data);

        // CRC32 (computed over chunk_type + data)
        let mut crc_data = Vec::new();
        crc_data.extend_from_slice(chunk_type);
        crc_data.extend_from_slice(data);
        let crc = compute_crc32(&crc_data);
        chunk.extend_from_slice(&crc.to_be_bytes());

        chunk
    }

    /// Build a minimal FHDR (file header) chunk
    fn build_fhdr_chunk() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&3u16.to_be_bytes()); // version = 3
        data.extend_from_slice(&0u16.to_be_bytes()); // padding
        data.extend_from_slice(b"DIFF"); // patch type
        data.extend_from_slice(&10u32.to_be_bytes()); // entry_files = 10
        build_chunk(b"FHDR", &data)
    }

    /// Build an APLY (apply option) chunk
    fn build_aply_chunk(option: u32, value: u32) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&option.to_be_bytes());
        data.extend_from_slice(&value.to_be_bytes());
        data.extend_from_slice(&[0u8; 4]); // padding
        build_chunk(b"APLY", &data)
    }

    /// Build an ADIR (add directory) chunk
    /// ADIR/DELD parsing expects size = 4 (chunk_type) + path_len
    /// and CRC at position data_start + size (4 bytes after path data)
    fn build_adir_chunk(path: &str) -> Vec<u8> {
        let mut path_data = path.as_bytes().to_vec();
        path_data.push(0); // null terminator

        // Size = 4 (chunk_type) + path_data.len()
        let size = (4 + path_data.len()) as u32;
        let mut chunk = Vec::new();

        // Size (4 bytes, big-endian)
        chunk.extend_from_slice(&size.to_be_bytes());

        // Chunk type (4 bytes)
        chunk.extend_from_slice(b"ADIR");

        // Path data
        chunk.extend_from_slice(&path_data);

        // 4 bytes padding (parser seeks to data_start + size for CRC)
        chunk.extend_from_slice(&[0u8; 4]);

        // CRC32 (computed over chunk_type + path_data + padding)
        let mut crc_data = Vec::new();
        crc_data.extend_from_slice(b"ADIR");
        crc_data.extend_from_slice(&path_data);
        crc_data.extend_from_slice(&[0u8; 4]);
        let crc = compute_crc32(&crc_data);
        chunk.extend_from_slice(&crc.to_be_bytes());

        chunk
    }

    /// Build a DELD (delete directory) chunk
    /// ADIR/DELD parsing expects size = 4 (chunk_type) + path_len
    /// and CRC at position data_start + size (4 bytes after path data)
    fn build_deld_chunk(path: &str) -> Vec<u8> {
        let mut path_data = path.as_bytes().to_vec();
        path_data.push(0); // null terminator

        // Size = 4 (chunk_type) + path_data.len()
        let size = (4 + path_data.len()) as u32;
        let mut chunk = Vec::new();

        // Size (4 bytes, big-endian)
        chunk.extend_from_slice(&size.to_be_bytes());

        // Chunk type (4 bytes)
        chunk.extend_from_slice(b"DELD");

        // Path data
        chunk.extend_from_slice(&path_data);

        // 4 bytes padding (parser seeks to data_start + size for CRC)
        chunk.extend_from_slice(&[0u8; 4]);

        // CRC32 (computed over chunk_type + path_data + padding)
        let mut crc_data = Vec::new();
        crc_data.extend_from_slice(b"DELD");
        crc_data.extend_from_slice(&path_data);
        crc_data.extend_from_slice(&[0u8; 4]);
        let crc = compute_crc32(&crc_data);
        chunk.extend_from_slice(&crc.to_be_bytes());

        chunk
    }

    /// Build an APFS (apply free space) chunk
    fn build_apfs_chunk(alloc_size: u64) -> Vec<u8> {
        let data = alloc_size.to_be_bytes();
        build_chunk(b"APFS", &data)
    }

    /// Build an EOF_ chunk
    fn build_eof_chunk() -> Vec<u8> {
        build_chunk(b"EOF_", &[])
    }

    /// Build a minimal SQPK chunk with PatchInfo (X command)
    fn build_sqpk_patch_info_chunk() -> Vec<u8> {
        // SQPK structure: inner_size(4) + command(1) + data
        let mut sqpk_data = Vec::new();

        // Inner size (data after this field)
        // 1 (command) + 3 (align) + 4 (status/version/pad) + 8 (install_size) = 16
        let inner_size: i32 = 16;
        sqpk_data.extend_from_slice(&inner_size.to_be_bytes());

        // Command: 'X' for PatchInfo
        sqpk_data.push(b'X');

        // Alignment (3 bytes)
        sqpk_data.extend_from_slice(&[0u8; 3]);

        // Status, version, padding
        sqpk_data.push(1); // status
        sqpk_data.push(2); // version
        sqpk_data.push(0); // pad
        sqpk_data.push(0); // pad2

        // Install size (8 bytes)
        sqpk_data.extend_from_slice(&1024u64.to_be_bytes());

        build_chunk(b"SQPK", &sqpk_data)
    }

    /// Build a SQPK chunk with TargetInfo (T command)
    fn build_sqpk_target_info_chunk() -> Vec<u8> {
        let mut sqpk_data = Vec::new();

        // Inner size: 1 (cmd) + 3 (align) + 1 (platform) + 2 (region) + 1 (is_debug) + 2 (version) + 1 (pad) + 8 + 8 = 27
        let inner_size: i32 = 27;
        sqpk_data.extend_from_slice(&inner_size.to_be_bytes());

        // Command: 'T'
        sqpk_data.push(b'T');

        // Alignment (3 bytes)
        sqpk_data.extend_from_slice(&[0u8; 3]);

        // Platform (0 = Win32)
        sqpk_data.push(0);

        // Region
        sqpk_data.extend_from_slice(&1u16.to_be_bytes());

        // Is debug
        sqpk_data.push(0);

        // Version
        sqpk_data.extend_from_slice(&100u16.to_be_bytes());

        // Padding
        sqpk_data.push(0);

        // Deleted data size
        sqpk_data.extend_from_slice(&512u64.to_be_bytes());

        // Seek count
        sqpk_data.extend_from_slice(&256u64.to_be_bytes());

        build_chunk(b"SQPK", &sqpk_data)
    }

    /// Build a complete minimal patch file
    fn build_minimal_patch() -> Vec<u8> {
        let mut patch = Vec::new();

        // Magic header
        patch.extend_from_slice(&ZIPATCH_MAGIC);

        // FHDR chunk
        patch.extend_from_slice(&build_fhdr_chunk());

        // EOF chunk
        patch.extend_from_slice(&build_eof_chunk());

        patch
    }

    /// Build a patch file with multiple chunks
    fn build_multi_chunk_patch() -> Vec<u8> {
        let mut patch = Vec::new();

        // Magic header
        patch.extend_from_slice(&ZIPATCH_MAGIC);

        // FHDR chunk
        patch.extend_from_slice(&build_fhdr_chunk());

        // APLY chunk (ignore missing = 1)
        patch.extend_from_slice(&build_aply_chunk(1, 1));

        // ADIR chunk
        patch.extend_from_slice(&build_adir_chunk("/game/sqpack/test"));

        // SQPK PatchInfo chunk
        patch.extend_from_slice(&build_sqpk_patch_info_chunk());

        // EOF chunk
        patch.extend_from_slice(&build_eof_chunk());

        patch
    }

    /// Create a temp file with given data and return the path
    fn create_temp_patch(data: &[u8]) -> NamedTempFile {
        use std::io::Write;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(data).unwrap();
        file.flush().unwrap();
        file
    }

    // ==========================================================================
    // Basic Tests
    // ==========================================================================

    #[test]
    fn test_parser_new() {
        let parser = ZiPatchParser::new();
        assert!(parser.verify_checksums);
    }

    #[test]
    fn test_parser_without_verification() {
        let parser = ZiPatchParser::without_checksum_verification();
        assert!(!parser.verify_checksums);
    }

    #[test]
    fn test_parser_default() {
        let parser = ZiPatchParser::default();
        assert!(parser.verify_checksums);
    }

    #[test]
    fn test_read_string() {
        let data = b"TEST\0\0\0\0";
        let mut cursor = std::io::Cursor::new(data);
        let result = ZiPatchParser::read_string(&mut cursor, 8).unwrap();
        assert_eq!(result, "TEST");
    }

    #[test]
    fn test_read_string_empty() {
        let data = b"\0\0\0\0";
        let mut cursor = std::io::Cursor::new(data);
        let result = ZiPatchParser::read_string(&mut cursor, 4).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_read_string_no_null() {
        let data = b"ABCD";
        let mut cursor = std::io::Cursor::new(data);
        let result = ZiPatchParser::read_string(&mut cursor, 4).unwrap();
        assert_eq!(result, "ABCD");
    }

    #[test]
    fn test_magic_validation() {
        // Valid magic
        let valid_magic = ZIPATCH_MAGIC;
        let mut cursor = std::io::Cursor::new(valid_magic);
        assert!(ZiPatchParser::read_magic_header(&mut cursor).is_ok());

        // Invalid magic
        let invalid_magic = [0u8; 12];
        let mut cursor = std::io::Cursor::new(invalid_magic);
        assert!(matches!(
            ZiPatchParser::read_magic_header(&mut cursor),
            Err(Error::ZiPatchInvalidMagic)
        ));
    }

    #[test]
    fn test_magic_validation_partial() {
        // Partial match should fail
        let mut partial_magic = ZIPATCH_MAGIC;
        partial_magic[11] = 0xFF; // corrupt last byte
        let mut cursor = std::io::Cursor::new(partial_magic);
        assert!(matches!(
            ZiPatchParser::read_magic_header(&mut cursor),
            Err(Error::ZiPatchInvalidMagic)
        ));
    }

    // ==========================================================================
    // End-to-End Parsing Tests
    // ==========================================================================

    #[test]
    fn test_parse_minimal_patch() {
        let patch_data = build_minimal_patch();
        let temp_file = create_temp_patch(&patch_data);

        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        assert_eq!(chunks.len(), 2);
        assert!(matches!(chunks[0], ZiPatchChunk::FileHeader(_)));
        assert!(matches!(chunks[1], ZiPatchChunk::EndOfFile));
    }

    #[test]
    fn test_parse_minimal_patch_no_verification() {
        let patch_data = build_minimal_patch();
        let temp_file = create_temp_patch(&patch_data);

        let parser = ZiPatchParser::without_checksum_verification();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn test_parse_multi_chunk_patch() {
        let patch_data = build_multi_chunk_patch();
        let temp_file = create_temp_patch(&patch_data);

        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        assert_eq!(chunks.len(), 5);
        assert!(matches!(chunks[0], ZiPatchChunk::FileHeader(_)));
        assert!(matches!(chunks[1], ZiPatchChunk::ApplyOption(_)));
        assert!(matches!(chunks[2], ZiPatchChunk::AddDirectory(_)));
        assert!(matches!(chunks[3], ZiPatchChunk::Sqpk(_)));
        assert!(matches!(chunks[4], ZiPatchChunk::EndOfFile));
    }

    #[test]
    fn test_parse_file_header_values() {
        let patch_data = build_minimal_patch();
        let temp_file = create_temp_patch(&patch_data);

        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        if let ZiPatchChunk::FileHeader(fh) = &chunks[0] {
            assert_eq!(fh.version, 3);
            assert_eq!(fh.patch_type, "DIFF");
            assert_eq!(fh.entry_files, 10);
        } else {
            panic!("Expected FileHeader chunk");
        }
    }

    #[test]
    fn test_parse_apply_option_values() {
        let patch_data = build_multi_chunk_patch();
        let temp_file = create_temp_patch(&patch_data);

        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        if let ZiPatchChunk::ApplyOption(opt) = &chunks[1] {
            assert_eq!(opt.option, ApplyOption::IgnoreMissing);
            assert_eq!(opt.value, 1);
        } else {
            panic!("Expected ApplyOption chunk");
        }
    }

    #[test]
    fn test_parse_add_directory_values() {
        let patch_data = build_multi_chunk_patch();
        let temp_file = create_temp_patch(&patch_data);

        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        if let ZiPatchChunk::AddDirectory(dir) = &chunks[2] {
            assert_eq!(dir.path, "/game/sqpack/test");
        } else {
            panic!("Expected AddDirectory chunk");
        }
    }

    #[test]
    fn test_parse_sqpk_patch_info_values() {
        let patch_data = build_multi_chunk_patch();
        let temp_file = create_temp_patch(&patch_data);

        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        if let ZiPatchChunk::Sqpk(SqpkChunk::PatchInfo(info)) = &chunks[3] {
            assert_eq!(info.status, 1);
            assert_eq!(info.version, 2);
            assert_eq!(info.install_size, 1024);
        } else {
            panic!("Expected SQPK PatchInfo chunk, got {:?}", chunks[3]);
        }
    }

    // ==========================================================================
    // Individual Chunk Type Tests
    // ==========================================================================

    #[test]
    fn test_parse_deld_chunk() {
        let mut patch = Vec::new();
        patch.extend_from_slice(&ZIPATCH_MAGIC);
        patch.extend_from_slice(&build_fhdr_chunk());
        patch.extend_from_slice(&build_deld_chunk("/old/directory"));
        patch.extend_from_slice(&build_eof_chunk());

        let temp_file = create_temp_patch(&patch);
        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        assert_eq!(chunks.len(), 3);
        if let ZiPatchChunk::DeleteDirectory(dir) = &chunks[1] {
            assert_eq!(dir.path, "/old/directory");
        } else {
            panic!("Expected DeleteDirectory chunk");
        }
    }

    #[test]
    fn test_parse_apfs_chunk() {
        let mut patch = Vec::new();
        patch.extend_from_slice(&ZIPATCH_MAGIC);
        patch.extend_from_slice(&build_fhdr_chunk());
        patch.extend_from_slice(&build_apfs_chunk(65536));
        patch.extend_from_slice(&build_eof_chunk());

        let temp_file = create_temp_patch(&patch);
        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        assert_eq!(chunks.len(), 3);
        if let ZiPatchChunk::ApplyFreeSpace(apfs) = &chunks[1] {
            assert_eq!(apfs.alloc_size, 65536);
        } else {
            panic!("Expected ApplyFreeSpace chunk");
        }
    }

    #[test]
    fn test_parse_sqpk_target_info() {
        let mut patch = Vec::new();
        patch.extend_from_slice(&ZIPATCH_MAGIC);
        patch.extend_from_slice(&build_fhdr_chunk());
        patch.extend_from_slice(&build_sqpk_target_info_chunk());
        patch.extend_from_slice(&build_eof_chunk());

        let temp_file = create_temp_patch(&patch);
        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        assert_eq!(chunks.len(), 3);
        if let ZiPatchChunk::Sqpk(SqpkChunk::TargetInfo(info)) = &chunks[1] {
            assert_eq!(info.platform, Platform::Win32);
            assert_eq!(info.region, 1);
            assert!(!info.is_debug);
            assert_eq!(info.version, 100);
            assert_eq!(info.deleted_data_size, 512);
            assert_eq!(info.seek_count, 256);
        } else {
            panic!("Expected SQPK TargetInfo chunk");
        }
    }

    // ==========================================================================
    // Unknown Chunk Handling Tests
    // ==========================================================================

    #[test]
    fn test_parse_unknown_chunk() {
        let mut patch = Vec::new();
        patch.extend_from_slice(&ZIPATCH_MAGIC);
        patch.extend_from_slice(&build_fhdr_chunk());
        // Build an unknown chunk type
        patch.extend_from_slice(&build_chunk(b"UNKN", b"test data"));
        patch.extend_from_slice(&build_eof_chunk());

        let temp_file = create_temp_patch(&patch);
        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        assert_eq!(chunks.len(), 3);
        if let ZiPatchChunk::Unknown { chunk_type, size, .. } = &chunks[1] {
            assert_eq!(chunk_type, "UNKN");
            assert_eq!(*size, 9); // data only (not including type)
        } else {
            panic!("Expected Unknown chunk");
        }
    }

    // ==========================================================================
    // Error Handling Tests
    // ==========================================================================

    #[test]
    fn test_parse_nonexistent_file() {
        let parser = ZiPatchParser::new();
        let result = parser.parse_patch(Path::new("/nonexistent/file.patch"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_magic() {
        let mut patch = vec![0u8; 12]; // Invalid magic
        patch.extend_from_slice(&build_eof_chunk());

        let temp_file = create_temp_patch(&patch);
        let parser = ZiPatchParser::new();
        let result = parser.parse_patch(temp_file.path());

        assert!(matches!(result, Err(Error::ZiPatchInvalidMagic)));
    }

    #[test]
    fn test_parse_corrupted_crc() {
        let mut patch = Vec::new();
        patch.extend_from_slice(&ZIPATCH_MAGIC);

        // Manually build a chunk with corrupted CRC
        // Format: [size][type][data][crc] where size = data.len()
        let mut data = Vec::new();
        data.extend_from_slice(&3u16.to_be_bytes()); // version = 3
        data.extend_from_slice(&0u16.to_be_bytes()); // padding
        data.extend_from_slice(b"DIFF"); // patch type
        data.extend_from_slice(&10u32.to_be_bytes()); // entry_files

        let size = data.len() as u32; // size = data only
        patch.extend_from_slice(&size.to_be_bytes());
        patch.extend_from_slice(b"FHDR");
        patch.extend_from_slice(&data);
        patch.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // Bad CRC

        let temp_file = create_temp_patch(&patch);
        let parser = ZiPatchParser::new();
        let result = parser.parse_patch(temp_file.path());

        assert!(matches!(result, Err(Error::ZiPatchChecksumMismatch { .. })));
    }

    #[test]
    fn test_parse_corrupted_crc_with_verification_disabled() {
        let mut patch = Vec::new();
        patch.extend_from_slice(&ZIPATCH_MAGIC);

        // Build a proper FHDR but with bad CRC - should still parse without verification
        let mut data = Vec::new();
        data.extend_from_slice(&3u16.to_be_bytes()); // version = 3
        data.extend_from_slice(&0u16.to_be_bytes()); // padding
        data.extend_from_slice(b"DIFF"); // patch type
        data.extend_from_slice(&10u32.to_be_bytes()); // entry_files

        let size = data.len() as u32; // size = data only
        patch.extend_from_slice(&size.to_be_bytes());
        patch.extend_from_slice(b"FHDR");
        patch.extend_from_slice(&data);
        patch.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // Bad CRC

        // Add EOF chunk (also with bad CRC)
        patch.extend_from_slice(&0u32.to_be_bytes()); // size = 0 (no data)
        patch.extend_from_slice(b"EOF_");
        patch.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // Bad CRC

        let temp_file = create_temp_patch(&patch);
        let parser = ZiPatchParser::without_checksum_verification();
        let result = parser.parse_patch(temp_file.path());

        // Should succeed without verification
        assert!(result.is_ok());
        let chunks = result.unwrap();
        assert_eq!(chunks.len(), 2);
    }

    // ==========================================================================
    // Apply Patch Tests
    // ==========================================================================

    #[test]
    fn test_apply_patch_creates_directory() {
        let mut patch = Vec::new();
        patch.extend_from_slice(&ZIPATCH_MAGIC);
        patch.extend_from_slice(&build_fhdr_chunk());
        patch.extend_from_slice(&build_adir_chunk("test_subdir"));
        patch.extend_from_slice(&build_eof_chunk());

        let temp_file = create_temp_patch(&patch);
        let game_dir = tempfile::tempdir().unwrap();

        let parser = ZiPatchParser::new();
        parser.apply_patch(temp_file.path(), game_dir.path()).unwrap();

        // Check that directory was created
        assert!(game_dir.path().join("test_subdir").exists());
    }

    #[test]
    fn test_apply_patch_deletes_directory() {
        let game_dir = tempfile::tempdir().unwrap();
        let dir_to_delete = game_dir.path().join("to_delete");
        std::fs::create_dir(&dir_to_delete).unwrap();
        assert!(dir_to_delete.exists());

        let mut patch = Vec::new();
        patch.extend_from_slice(&ZIPATCH_MAGIC);
        patch.extend_from_slice(&build_fhdr_chunk());
        patch.extend_from_slice(&build_deld_chunk("to_delete"));
        patch.extend_from_slice(&build_eof_chunk());

        let temp_file = create_temp_patch(&patch);

        let parser = ZiPatchParser::new();
        parser.apply_patch(temp_file.path(), game_dir.path()).unwrap();

        // Check that directory was deleted
        assert!(!dir_to_delete.exists());
    }

    // ==========================================================================
    // Chunk Type String Tests
    // ==========================================================================

    #[test]
    fn test_chunk_type_strings() {
        let patch_data = build_multi_chunk_patch();
        let temp_file = create_temp_patch(&patch_data);

        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        assert_eq!(chunks[0].chunk_type(), "FHDR");
        assert_eq!(chunks[1].chunk_type(), "APLY");
        assert_eq!(chunks[2].chunk_type(), "ADIR");
        assert_eq!(chunks[3].chunk_type(), "SQPK");
        assert_eq!(chunks[4].chunk_type(), "EOF_");
    }

    // ==========================================================================
    // CRC32 Helper Tests
    // ==========================================================================

    #[test]
    fn test_build_chunk_crc() {
        // Verify our test fixture builds correct CRC
        let chunk = build_chunk(b"TEST", b"data");

        // Size should be 4 (data only, not including type)
        let size = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        assert_eq!(size, 4);

        // Verify chunk type
        assert_eq!(&chunk[4..8], b"TEST");

        // Verify data
        assert_eq!(&chunk[8..12], b"data");

        // Verify CRC is present (4 bytes at end)
        assert_eq!(chunk.len(), 4 + 4 + 4 + 4); // size + type + data + crc
    }

    // ==========================================================================
    // SQPK Command Tests
    // ==========================================================================

    #[test]
    fn test_sqpk_command_strings() {
        let patch_data = build_multi_chunk_patch();
        let temp_file = create_temp_patch(&patch_data);

        let parser = ZiPatchParser::new();
        let chunks = parser.parse_patch(temp_file.path()).unwrap();

        if let ZiPatchChunk::Sqpk(sqpk) = &chunks[3] {
            assert_eq!(sqpk.command(), "X");
        } else {
            panic!("Expected SQPK chunk");
        }
    }
}
