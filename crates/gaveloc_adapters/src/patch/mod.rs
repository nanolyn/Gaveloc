//! Patching adapters for version checking and patch management

mod downloader;
mod server;
mod version;

pub use downloader::HttpPatchDownloader;
pub use server::SquareEnixPatchServer;
pub use version::FileVersionRepository;
