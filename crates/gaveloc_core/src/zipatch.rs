//! ZiPatch file format types
//!
//! ZiPatch is Square Enix's proprietary binary patch format used for FFXIV updates.
//! It consists of a magic header followed by a series of chunks, each containing
//! instructions for modifying game files.
//!
//! Format structure:
//! - Magic header: 12 bytes (0x91 'Z' 'I' 'P' 'A' 'T' 'C' 'H' 0x0D 0x0A 0x1A 0x0A)
//! - Chunks: [size: u32 BE][type: 4 ASCII][data: N bytes][crc32: u32 BE]
//! - EOF chunk terminates the file

use std::fmt;

use serde::{Deserialize, Serialize};

/// ZiPatch file magic header bytes
/// 0x91 followed by "ZIPATCH" and control chars
pub const ZIPATCH_MAGIC: [u8; 12] = [
    0x91, 0x5A, 0x49, 0x50, 0x41, 0x54, 0x43, 0x48, 0x0D, 0x0A, 0x1A, 0x0A,
];

/// Platform identifier for file paths
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Win32,
    Ps3,
    Ps4,
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Platform::Win32 => write!(f, "win32"),
            Platform::Ps3 => write!(f, "ps3"),
            Platform::Ps4 => write!(f, "ps4"),
        }
    }
}

impl Default for Platform {
    fn default() -> Self {
        Platform::Win32
    }
}

// =============================================================================
// Chunk Types
// =============================================================================

/// A parsed ZiPatch chunk
#[derive(Debug, Clone)]
pub enum ZiPatchChunk {
    /// File header chunk (FHDR) - contains version and patch type info
    FileHeader(FileHeaderChunk),

    /// Apply option chunk (APLY) - configuration options for patch application
    ApplyOption(ApplyOptionChunk),

    /// Add directory chunk (ADIR) - creates a new directory
    AddDirectory(AddDirectoryChunk),

    /// Delete directory chunk (DELD) - removes a directory
    DeleteDirectory(DeleteDirectoryChunk),

    /// Apply free space chunk (APFS) - allocates free space (legacy, rarely used)
    ApplyFreeSpace(ApplyFreeSpaceChunk),

    /// SqPack command chunk (SQPK) - the main patching operations
    Sqpk(SqpkChunk),

    /// End of file chunk (EOF_) - marks end of patch file
    EndOfFile,

    /// Unknown chunk type - preserved for forward compatibility
    Unknown {
        chunk_type: String,
        offset: u64,
        size: u32,
    },
}

impl ZiPatchChunk {
    /// Get the 4-character type identifier for this chunk
    pub fn chunk_type(&self) -> &str {
        match self {
            ZiPatchChunk::FileHeader(_) => "FHDR",
            ZiPatchChunk::ApplyOption(_) => "APLY",
            ZiPatchChunk::AddDirectory(_) => "ADIR",
            ZiPatchChunk::DeleteDirectory(_) => "DELD",
            ZiPatchChunk::ApplyFreeSpace(_) => "APFS",
            ZiPatchChunk::Sqpk(_) => "SQPK",
            ZiPatchChunk::EndOfFile => "EOF_",
            ZiPatchChunk::Unknown { chunk_type, .. } => chunk_type,
        }
    }
}

/// File header chunk - appears at start of patch
#[derive(Debug, Clone)]
pub struct FileHeaderChunk {
    /// Patch file format version (usually 3)
    pub version: u16,
    /// Type of patch (e.g., "DIFF", "HIST")
    pub patch_type: String,
    /// Number of files affected (informational)
    pub entry_files: u32,
    /// Offset where chunk was found
    pub offset: u64,
}

/// Apply option chunk - patch application settings
#[derive(Debug, Clone)]
pub struct ApplyOptionChunk {
    /// Option type/operation
    pub option: ApplyOption,
    /// Option value
    pub value: u32,
    /// Offset where chunk was found
    pub offset: u64,
}

/// Apply option types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOption {
    /// Ignore missing files during patching
    IgnoreMissing,
    /// Ignore mismatch files
    IgnoreOldMismatch,
    /// Unknown option
    Unknown(u32),
}

impl From<u32> for ApplyOption {
    fn from(value: u32) -> Self {
        match value {
            1 => ApplyOption::IgnoreMissing,
            2 => ApplyOption::IgnoreOldMismatch,
            _ => ApplyOption::Unknown(value),
        }
    }
}

/// Add directory chunk - creates a directory
#[derive(Debug, Clone)]
pub struct AddDirectoryChunk {
    /// Path to create (relative to game directory)
    pub path: String,
    /// Offset where chunk was found
    pub offset: u64,
}

/// Delete directory chunk - removes a directory
#[derive(Debug, Clone)]
pub struct DeleteDirectoryChunk {
    /// Path to delete (relative to game directory)
    pub path: String,
    /// Offset where chunk was found
    pub offset: u64,
}

/// Apply free space chunk - legacy allocator (rarely used)
#[derive(Debug, Clone)]
pub struct ApplyFreeSpaceChunk {
    /// Size to allocate
    pub alloc_size: u64,
    /// Offset where chunk was found
    pub offset: u64,
}

// =============================================================================
// SqPack Commands
// =============================================================================

/// SQPK chunk - contains SqPack-specific patching commands
#[derive(Debug, Clone)]
pub enum SqpkChunk {
    /// Add data to a dat file
    AddData(SqpkAddData),

    /// Delete data from a dat file
    DeleteData(SqpkDeleteData),

    /// Expand data in a dat file
    ExpandData(SqpkExpandData),

    /// Modify header of a sqpack file
    Header(SqpkHeader),

    /// Modify index file entries
    Index(SqpkIndex),

    /// File operation (create/modify files)
    File(SqpkFile),

    /// Patch metadata information
    PatchInfo(SqpkPatchInfo),

    /// Target platform/region information
    TargetInfo(SqpkTargetInfo),

    /// Unknown SQPK command
    Unknown { command: String, offset: u64 },
}

impl SqpkChunk {
    /// Get the single-character command identifier
    pub fn command(&self) -> &str {
        match self {
            SqpkChunk::AddData(_) => "A",
            SqpkChunk::DeleteData(_) => "D",
            SqpkChunk::ExpandData(_) => "E",
            SqpkChunk::Header(_) => "H",
            SqpkChunk::Index(_) => "I",
            SqpkChunk::File(_) => "F",
            SqpkChunk::PatchInfo(_) => "X",
            SqpkChunk::TargetInfo(_) => "T",
            SqpkChunk::Unknown { command, .. } => command,
        }
    }
}

/// Target file specification for SqPack operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqpackFileTarget {
    /// Main ID (repository identifier)
    pub main_id: u16,
    /// Sub ID (category/expansion)
    pub sub_id: u16,
    /// File ID within the category
    pub file_id: u32,
}

impl SqpackFileTarget {
    /// Get the expansion ID from the sub_id
    pub fn expansion_id(&self) -> u8 {
        (self.sub_id >> 8) as u8
    }

    /// Get the expansion folder name
    pub fn expansion_folder(&self) -> &'static str {
        match self.expansion_id() {
            0 => "ffxiv",
            1 => "ex1",
            2 => "ex2",
            3 => "ex3",
            4 => "ex4",
            5 => "ex5",
            _ => "ffxiv",
        }
    }

    /// Build the relative path for a dat file
    pub fn dat_path(&self, platform: Platform) -> String {
        format!(
            "sqpack/{}/{:02x}{:04x}.{}.dat{}",
            self.expansion_folder(),
            self.main_id,
            self.sub_id,
            platform,
            self.file_id
        )
    }

    /// Build the relative path for an index file
    pub fn index_path(&self, platform: Platform, index_type: IndexType) -> String {
        let suffix = match index_type {
            IndexType::Index => "index",
            IndexType::Index2 => "index2",
        };
        format!(
            "sqpack/{}/{:02x}{:04x}.{}.{}",
            self.expansion_folder(),
            self.main_id,
            self.sub_id,
            platform,
            suffix
        )
    }
}

impl fmt::Display for SqpackFileTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}{:04x}.dat{}",
            self.main_id, self.sub_id, self.file_id
        )
    }
}

/// Index file type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    Index,
    Index2,
}

/// Add data command - writes data to a dat file
#[derive(Debug, Clone)]
pub struct SqpkAddData {
    /// Target dat file
    pub target_file: SqpackFileTarget,
    /// Byte offset in the file (left-shifted by 7 during parsing)
    pub block_offset: u64,
    /// Number of bytes to write
    pub block_number: u64,
    /// Number of bytes to delete after writing
    pub block_delete_number: u64,
    /// The actual data to write
    pub block_data: Vec<u8>,
    /// Offset in patch file where data was found
    pub data_source_offset: u64,
    /// Offset where chunk was found
    pub offset: u64,
}

/// Delete data command - removes data from a dat file
#[derive(Debug, Clone)]
pub struct SqpkDeleteData {
    /// Target dat file
    pub target_file: SqpackFileTarget,
    /// Byte offset to start deletion
    pub block_offset: u64,
    /// Number of bytes to delete
    pub block_number: u64,
    /// Offset where chunk was found
    pub offset: u64,
}

/// Expand data command - expands space in a dat file
#[derive(Debug, Clone)]
pub struct SqpkExpandData {
    /// Target dat file
    pub target_file: SqpackFileTarget,
    /// Byte offset for expansion
    pub block_offset: u64,
    /// Number of bytes to expand by
    pub block_number: u64,
    /// Offset where chunk was found
    pub offset: u64,
}

/// Header modification command
#[derive(Debug, Clone)]
pub struct SqpkHeader {
    /// Type of file (dat, index, etc.)
    pub file_kind: SqpkFileKind,
    /// Type of header operation
    pub header_kind: SqpkHeaderKind,
    /// Target file
    pub target_file: SqpackFileTarget,
    /// Header data to write
    pub header_data: Vec<u8>,
    /// Offset where chunk was found
    pub offset: u64,
}

/// Type of SqPack file being modified
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqpkFileKind {
    Dat,
    Index,
    Unknown(u8),
}

impl From<u8> for SqpkFileKind {
    fn from(value: u8) -> Self {
        match value {
            b'D' => SqpkFileKind::Dat,
            b'I' => SqpkFileKind::Index,
            _ => SqpkFileKind::Unknown(value),
        }
    }
}

/// Type of header operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqpkHeaderKind {
    Version,
    Index,
    Data,
    Unknown(u8),
}

impl From<u8> for SqpkHeaderKind {
    fn from(value: u8) -> Self {
        match value {
            b'V' => SqpkHeaderKind::Version,
            b'I' => SqpkHeaderKind::Index,
            b'D' => SqpkHeaderKind::Data,
            _ => SqpkHeaderKind::Unknown(value),
        }
    }
}

/// Index modification command
#[derive(Debug, Clone)]
pub struct SqpkIndex {
    /// Type of index (index or index2)
    pub index_type: IndexType,
    /// Whether this is a data update or sync
    pub is_synonym: bool,
    /// Target file
    pub target_file: SqpackFileTarget,
    /// Index entries to modify
    pub index_data: Vec<SqpkIndexData>,
    /// Offset where chunk was found
    pub offset: u64,
}

/// A single index entry modification
#[derive(Debug, Clone)]
pub struct SqpkIndexData {
    /// Hash of the file path
    pub file_hash: u64,
    /// Block offset in dat file
    pub block_offset: u32,
    /// Block number
    pub block_number: u32,
}

/// File operation command - for creating/modifying files
#[derive(Debug, Clone)]
pub struct SqpkFile {
    /// Operation to perform
    pub operation: SqpkFileOperation,
    /// Expansion ID
    pub expansion_id: u8,
    /// File path (relative)
    pub file_path: String,
    /// File data (for add operations)
    pub file_data: Vec<u8>,
    /// Offset where chunk was found
    pub offset: u64,
}

/// File operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqpkFileOperation {
    /// Add or overwrite a file
    AddFile,
    /// Remove a file
    RemoveAll,
    /// Delete a file (alias for RemoveAll in some contexts)
    DeleteFile,
    /// Make a directory
    MakeDir,
    /// Unknown operation
    Unknown(u8),
}

impl From<u8> for SqpkFileOperation {
    fn from(value: u8) -> Self {
        match value {
            b'A' => SqpkFileOperation::AddFile,
            b'R' => SqpkFileOperation::RemoveAll,
            b'D' => SqpkFileOperation::DeleteFile,
            b'M' => SqpkFileOperation::MakeDir,
            _ => SqpkFileOperation::Unknown(value),
        }
    }
}

/// Patch information command - metadata about the patch
#[derive(Debug, Clone)]
pub struct SqpkPatchInfo {
    /// Status code
    pub status: u8,
    /// Version string
    pub version: u8,
    /// Install size
    pub install_size: u64,
    /// Offset where chunk was found
    pub offset: u64,
}

/// Target information command - platform and region info
#[derive(Debug, Clone)]
pub struct SqpkTargetInfo {
    /// Target platform
    pub platform: Platform,
    /// Region code
    pub region: u16,
    /// Is debug build
    pub is_debug: bool,
    /// Target version
    pub version: u16,
    /// Deleted data size
    pub deleted_data_size: u64,
    /// Seek count
    pub seek_count: u64,
    /// Offset where chunk was found
    pub offset: u64,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_magic_header() {
        assert_eq!(ZIPATCH_MAGIC.len(), 12);
        assert_eq!(ZIPATCH_MAGIC[0], 0x91);
        assert_eq!(&ZIPATCH_MAGIC[1..8], b"ZIPATCH");
    }

    #[test]
    fn test_sqpack_file_target_expansion() {
        let target = SqpackFileTarget {
            main_id: 0x04,
            sub_id: 0x0100, // expansion 1
            file_id: 0,
        };
        assert_eq!(target.expansion_id(), 1);
        assert_eq!(target.expansion_folder(), "ex1");
    }

    #[test]
    fn test_sqpack_file_target_dat_path() {
        let target = SqpackFileTarget {
            main_id: 0x04,
            sub_id: 0x0000, // base game
            file_id: 0,
        };
        assert_eq!(
            target.dat_path(Platform::Win32),
            "sqpack/ffxiv/040000.win32.dat0"
        );
    }

    #[test]
    fn test_apply_option_from() {
        assert_eq!(ApplyOption::from(1), ApplyOption::IgnoreMissing);
        assert_eq!(ApplyOption::from(2), ApplyOption::IgnoreOldMismatch);
        assert!(matches!(ApplyOption::from(99), ApplyOption::Unknown(99)));
    }

    #[test]
    fn test_sqpk_file_kind_from() {
        assert_eq!(SqpkFileKind::from(b'D'), SqpkFileKind::Dat);
        assert_eq!(SqpkFileKind::from(b'I'), SqpkFileKind::Index);
        assert!(matches!(SqpkFileKind::from(b'X'), SqpkFileKind::Unknown(b'X')));
    }

    #[test]
    fn test_sqpk_file_operation_from() {
        assert_eq!(SqpkFileOperation::from(b'A'), SqpkFileOperation::AddFile);
        assert_eq!(SqpkFileOperation::from(b'R'), SqpkFileOperation::RemoveAll);
        assert_eq!(SqpkFileOperation::from(b'M'), SqpkFileOperation::MakeDir);
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(Platform::Win32.to_string(), "win32");
        assert_eq!(Platform::Ps4.to_string(), "ps4");
    }

    #[test]
    fn test_chunk_type() {
        let chunk = ZiPatchChunk::EndOfFile;
        assert_eq!(chunk.chunk_type(), "EOF_");

        let chunk = ZiPatchChunk::AddDirectory(AddDirectoryChunk {
            path: "/test".to_string(),
            offset: 0,
        });
        assert_eq!(chunk.chunk_type(), "ADIR");
    }

    #[test]
    fn test_sqpk_command() {
        let chunk = SqpkChunk::PatchInfo(SqpkPatchInfo {
            status: 0,
            version: 1,
            install_size: 0,
            offset: 0,
        });
        assert_eq!(chunk.command(), "X");
    }

    // ==========================================================================
    // Additional Tests for Coverage
    // ==========================================================================

    #[test]
    fn test_platform_default() {
        assert_eq!(Platform::default(), Platform::Win32);
    }

    #[test]
    fn test_platform_display_all() {
        assert_eq!(Platform::Win32.to_string(), "win32");
        assert_eq!(Platform::Ps3.to_string(), "ps3");
        assert_eq!(Platform::Ps4.to_string(), "ps4");
    }

    #[test]
    fn test_chunk_type_all_variants() {
        // FileHeader
        assert_eq!(
            ZiPatchChunk::FileHeader(FileHeaderChunk {
                version: 1,
                patch_type: "DIFF".to_string(),
                entry_files: 0,
                offset: 0,
            })
            .chunk_type(),
            "FHDR"
        );

        // ApplyOption
        assert_eq!(
            ZiPatchChunk::ApplyOption(ApplyOptionChunk {
                option: ApplyOption::IgnoreMissing,
                value: 1,
                offset: 0,
            })
            .chunk_type(),
            "APLY"
        );

        // DeleteDirectory
        assert_eq!(
            ZiPatchChunk::DeleteDirectory(DeleteDirectoryChunk {
                path: "/test".to_string(),
                offset: 0,
            })
            .chunk_type(),
            "DELD"
        );

        // ApplyFreeSpace
        assert_eq!(
            ZiPatchChunk::ApplyFreeSpace(ApplyFreeSpaceChunk {
                alloc_size: 1024,
                offset: 0,
            })
            .chunk_type(),
            "APFS"
        );

        // Sqpk
        assert_eq!(
            ZiPatchChunk::Sqpk(SqpkChunk::PatchInfo(SqpkPatchInfo {
                status: 0,
                version: 0,
                install_size: 0,
                offset: 0,
            }))
            .chunk_type(),
            "SQPK"
        );

        // Unknown
        assert_eq!(
            ZiPatchChunk::Unknown {
                chunk_type: "TEST".to_string(),
                offset: 0,
                size: 0,
            }
            .chunk_type(),
            "TEST"
        );
    }

    #[test]
    fn test_sqpk_command_all_variants() {
        // AddData
        assert_eq!(
            SqpkChunk::AddData(SqpkAddData {
                target_file: SqpackFileTarget {
                    main_id: 0,
                    sub_id: 0,
                    file_id: 0
                },
                block_offset: 0,
                block_number: 0,
                block_delete_number: 0,
                block_data: vec![],
                data_source_offset: 0,
                offset: 0,
            })
            .command(),
            "A"
        );

        // DeleteData
        assert_eq!(
            SqpkChunk::DeleteData(SqpkDeleteData {
                target_file: SqpackFileTarget {
                    main_id: 0,
                    sub_id: 0,
                    file_id: 0
                },
                block_offset: 0,
                block_number: 0,
                offset: 0,
            })
            .command(),
            "D"
        );

        // ExpandData
        assert_eq!(
            SqpkChunk::ExpandData(SqpkExpandData {
                target_file: SqpackFileTarget {
                    main_id: 0,
                    sub_id: 0,
                    file_id: 0
                },
                block_offset: 0,
                block_number: 0,
                offset: 0,
            })
            .command(),
            "E"
        );

        // Header
        assert_eq!(
            SqpkChunk::Header(SqpkHeader {
                file_kind: SqpkFileKind::Dat,
                header_kind: SqpkHeaderKind::Version,
                target_file: SqpackFileTarget {
                    main_id: 0,
                    sub_id: 0,
                    file_id: 0
                },
                header_data: vec![],
                offset: 0,
            })
            .command(),
            "H"
        );

        // Index
        assert_eq!(
            SqpkChunk::Index(SqpkIndex {
                index_type: IndexType::Index,
                is_synonym: false,
                target_file: SqpackFileTarget {
                    main_id: 0,
                    sub_id: 0,
                    file_id: 0
                },
                index_data: vec![],
                offset: 0,
            })
            .command(),
            "I"
        );

        // File
        assert_eq!(
            SqpkChunk::File(SqpkFile {
                operation: SqpkFileOperation::AddFile,
                expansion_id: 0,
                file_path: String::new(),
                file_data: vec![],
                offset: 0,
            })
            .command(),
            "F"
        );

        // TargetInfo
        assert_eq!(
            SqpkChunk::TargetInfo(SqpkTargetInfo {
                platform: Platform::Win32,
                region: 0,
                is_debug: false,
                version: 0,
                deleted_data_size: 0,
                seek_count: 0,
                offset: 0,
            })
            .command(),
            "T"
        );

        // Unknown
        assert_eq!(
            SqpkChunk::Unknown {
                command: "Z".to_string(),
                offset: 0
            }
            .command(),
            "Z"
        );
    }

    #[test]
    fn test_sqpack_file_target_all_expansions() {
        // Base game
        let target = SqpackFileTarget {
            main_id: 0,
            sub_id: 0x0000,
            file_id: 0,
        };
        assert_eq!(target.expansion_folder(), "ffxiv");

        // Expansion 2-5
        for (exp, folder) in [(2, "ex2"), (3, "ex3"), (4, "ex4"), (5, "ex5")] {
            let target = SqpackFileTarget {
                main_id: 0,
                sub_id: (exp << 8) as u16,
                file_id: 0,
            };
            assert_eq!(target.expansion_folder(), folder);
        }

        // Unknown expansion falls back to ffxiv
        let target = SqpackFileTarget {
            main_id: 0,
            sub_id: 0x0F00, // expansion 15
            file_id: 0,
        };
        assert_eq!(target.expansion_folder(), "ffxiv");
    }

    #[test]
    fn test_sqpack_file_target_index_path() {
        let target = SqpackFileTarget {
            main_id: 0x04,
            sub_id: 0x0100,
            file_id: 0,
        };
        assert_eq!(
            target.index_path(Platform::Win32, IndexType::Index),
            "sqpack/ex1/040100.win32.index"
        );
        assert_eq!(
            target.index_path(Platform::Win32, IndexType::Index2),
            "sqpack/ex1/040100.win32.index2"
        );
    }

    #[test]
    fn test_sqpack_file_target_display() {
        let target = SqpackFileTarget {
            main_id: 0x04,
            sub_id: 0x0100,
            file_id: 5,
        };
        assert_eq!(format!("{}", target), "040100.dat5");
    }

    #[test]
    fn test_sqpk_header_kind_from() {
        assert_eq!(SqpkHeaderKind::from(b'V'), SqpkHeaderKind::Version);
        assert_eq!(SqpkHeaderKind::from(b'I'), SqpkHeaderKind::Index);
        assert_eq!(SqpkHeaderKind::from(b'D'), SqpkHeaderKind::Data);
        assert!(matches!(
            SqpkHeaderKind::from(b'X'),
            SqpkHeaderKind::Unknown(b'X')
        ));
    }

    #[test]
    fn test_sqpk_file_operation_delete_file() {
        assert_eq!(SqpkFileOperation::from(b'D'), SqpkFileOperation::DeleteFile);
    }

    #[test]
    fn test_sqpk_file_operation_unknown() {
        assert!(matches!(
            SqpkFileOperation::from(b'Z'),
            SqpkFileOperation::Unknown(b'Z')
        ));
    }
}
