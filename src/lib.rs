//! A crate that allow you to access the romfs of an unencrypted romfs.
//!
//! It contain the function `get_romfs_vfs`, that accept a File (or similar Read + Seek + some stuff) object, and return an object that implement `vfs::VFS`
//!
//! It also contain some additional function that can be usefull while handling decrypted .3ds file.
//!
//! This library should never crash, and always return an error.
//!
//! # Examples
//!
//! ```rust
//! use std::fs::File;
//! use fs3ds::get_romfs_vfs;
//! let file = File::open("rom.3ds").unwrap(); // get an access to an unencrypted romfs file
//! let _romfs_vfs = get_romfs_vfs(file).unwrap(); // get a vfs::VFS object to access the rom read only
//! ```

use std::error::Error;
use std::fmt;
use std::io;

mod ncsd;
pub use ncsd::{NCSDError, NCSDReader};

mod ncch;
pub use ncch::{NCCHError, NCCHReader};

mod partition;
pub use partition::Partition;
pub use partition::PartitionMutex;

mod ivfc;
pub use ivfc::{IVFCError, IVFCReader};

mod ivfc_vfs;
pub use ivfc_vfs::{IVFCMeta, IVFCVFS, IVFCVPATH};

#[derive(Debug, Clone, Copy)]
struct PartitionData {
    offset: u32,
    lenght: u32,
}

#[derive(Debug)]
pub enum GetRomfsError {
    ReadNcsdError(NCSDError),
    ReadNcchError(NCCHError),
    ReadIVFCError(IVFCError),
}

impl Error for GetRomfsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadNcchError(err) => Some(err),
            Self::ReadNcsdError(err) => Some(err),
            Self::ReadIVFCError(err) => Some(err),
        }
    }
}

impl fmt::Display for GetRomfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadNcchError(_) => write!(f, "error with an ncch file"),
            Self::ReadNcsdError(_) => write!(f, "error with an ncsd file"),
            Self::ReadIVFCError(_) => write!(f, "error with an ivfc file"),
        }
    }
}

impl From<NCCHError> for GetRomfsError {
    fn from(e: NCCHError) -> GetRomfsError {
        GetRomfsError::ReadNcchError(e)
    }
}

impl From<NCSDError> for GetRomfsError {
    fn from(e: NCSDError) -> GetRomfsError {
        GetRomfsError::ReadNcsdError(e)
    }
}

impl From<IVFCError> for GetRomfsError {
    fn from(e: IVFCError) -> GetRomfsError {
        GetRomfsError::ReadIVFCError(e)
    }
}

/// Read a .3ds file, and return an `IVFCVFS` object if succesfull.
pub fn get_romfs_vfs<T: io::Read + io::Seek + fmt::Debug + Send + Sync>(
    file: T,
) -> Result<IVFCVFS<Partition<Partition<T>>>, GetRomfsError> {
    let ncsd = NCSDReader::new(file)?;
    let partition = ncsd.load_partition(0)?;
    let ncch = NCCHReader::new(partition)?;
    let romfs = ncch.get_romfs()?;
    let ivfc = IVFCReader::new(romfs)?;
    Ok(IVFCVFS::new(ivfc))
}
