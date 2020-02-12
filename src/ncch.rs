use crate::Partition;
use crate::PartitionData;
use std::error::Error;
use std::fmt;
use std::io;
use std::io::SeekFrom;
use std::io::{Read, Seek};

#[derive(Debug)]
pub enum NCCHError {
    ReadNCCHSignatureError(io::Error),
    ReadMagicError(io::Error),
    InvalidMagic([u8; 4]), // the invalid magic
    SizeReadError(io::Error),
    PartitionIdReadError(io::Error),
    MakerCodeReadError(io::Error),
    VersionReadError(io::Error),
    FlagsSeekError(io::Error),
    FlagsReadError(io::Error),
    OffsetSeekError(io::Error, &'static str),
    OffsetReadError(io::Error, &'static str),
    LenghtReadError(io::Error, &'static str),
    CreatePartitionError(io::Error),
}

impl Error for NCCHError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadNCCHSignatureError(ioerror) => Some(ioerror),
            Self::ReadMagicError(ioerror) => Some(ioerror),
            Self::SizeReadError(ioerror) => Some(ioerror),
            Self::PartitionIdReadError(ioerror) => Some(ioerror),
            Self::MakerCodeReadError(ioerror) => Some(ioerror),
            Self::VersionReadError(ioerror) => Some(ioerror),
            Self::FlagsSeekError(ioerror) => Some(ioerror),
            Self::FlagsReadError(ioerror) => Some(ioerror),
            Self::OffsetSeekError(ioerror, _) => Some(ioerror),
            Self::OffsetReadError(ioerror, _) => Some(ioerror),
            Self::LenghtReadError(ioerror, _) => Some(ioerror),
            Self::CreatePartitionError(ioerror) => Some(ioerror),
            _ => None,
        }
    }
}

impl fmt::Display for NCCHError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            _ => write!(f, "{:?}", self), //TODO: specific error message
        }
    }
}

pub struct NCCHReader<T: Read + Seek> {
    file: T,
    pub content_size: u32,
    pub version: u16,
    plain_region: PartitionData,
    logo_region: PartitionData,
    exefs: PartitionData,
    romfs: PartitionData,
}

impl<T: Read + Seek> NCCHReader<T> {
    pub fn new(mut file: T) -> Result<NCCHReader<T>, NCCHError> {
        // header signature
        //TODO: check the signature
        let mut _ncch_header_signature = [0; 0x100];
        match file.read_exact(&mut _ncch_header_signature) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::ReadNCCHSignatureError(err)),
        };

        // magic
        let mut magic = [0; 4];
        match file.read_exact(&mut magic) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::ReadMagicError(err)),
        };

        if magic != [78, 67, 67, 72] {
            return Err(NCCHError::InvalidMagic(magic));
        };

        // content size
        let mut content_size = [0; 4];
        match file.read_exact(&mut content_size) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::SizeReadError(err)),
        };

        let content_size = u32::from_le_bytes(content_size) * 0x200;

        // partition id
        let mut partition_id = [0; 8];
        match file.read_exact(&mut partition_id) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::PartitionIdReadError(err)),
        };

        // make code
        let mut maker_code = [0; 2];
        match file.read_exact(&mut maker_code) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::MakerCodeReadError(err)),
        };

        // version
        let mut version = [0; 2];
        match file.read_exact(&mut version) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::VersionReadError(err)),
        };

        let version = u16::from_le_bytes(version);

        // flags
        match file.seek(SeekFrom::Start(0x188)) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::FlagsSeekError(err)),
        };

        let mut flags = [0; 8];
        match file.read_exact(&mut flags) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::FlagsReadError(err)),
        }

        //TODO: check for no crypto

        // data
        let mut plain_region_offset = [0; 4];
        match file.read_exact(&mut plain_region_offset) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::OffsetReadError(err, "plain region")),
        }
        let plain_region_offset = u32::from_le_bytes(plain_region_offset) * 0x200;

        let mut plain_region_lenght = [0; 4];
        match file.read_exact(&mut plain_region_lenght) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::LenghtReadError(err, "plain region")),
        }
        let plain_region_lenght = u32::from_le_bytes(plain_region_lenght) * 0x200;

        let plain_region = PartitionData {
            offset: plain_region_offset,
            lenght: plain_region_lenght,
        };

        let mut logo_region_offset = [0; 4];
        match file.read_exact(&mut logo_region_offset) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::OffsetReadError(err, "logo region")),
        }
        let logo_region_offset = u32::from_le_bytes(logo_region_offset) * 0x200;

        let mut logo_region_lenght = [0; 4];
        match file.read_exact(&mut logo_region_lenght) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::LenghtReadError(err, "logo region")),
        }
        let logo_region_lenght = u32::from_le_bytes(logo_region_lenght) * 0x200;

        let logo_region = PartitionData {
            offset: logo_region_offset,
            lenght: logo_region_lenght,
        };

        let mut exefs_offset = [0; 4];
        match file.read_exact(&mut exefs_offset) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::OffsetReadError(err, "exefs")),
        }
        let exefs_offset = u32::from_le_bytes(exefs_offset) * 0x200;

        let mut exefs_lenght = [0; 4];
        match file.read_exact(&mut exefs_lenght) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::LenghtReadError(err, "exefs")),
        }
        let exefs_lenght = u32::from_le_bytes(exefs_lenght) * 0x200;

        let exefs = PartitionData {
            offset: exefs_offset,
            lenght: exefs_lenght,
        };

        match file.seek(SeekFrom::Start(0x1B0)) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::OffsetSeekError(err, "romfs")),
        };

        let mut romfs_offset = [0; 4];
        match file.read_exact(&mut romfs_offset) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::OffsetReadError(err, "romfs")),
        }
        let romfs_offset = u32::from_le_bytes(romfs_offset) * 0x200;

        let mut romfs_lenght = [0; 4];
        match file.read_exact(&mut romfs_lenght) {
            Ok(_) => (),
            Err(err) => return Err(NCCHError::LenghtReadError(err, "exefs")),
        }
        let romfs_lenght = u32::from_le_bytes(romfs_lenght) * 0x200;

        let romfs = PartitionData {
            offset: romfs_offset,
            lenght: romfs_lenght,
        };

        Ok(NCCHReader {
            file,
            content_size,
            version,
            plain_region,
            logo_region,
            exefs,
            romfs,
        })
    }

    pub fn get_plain_region(self) -> Result<Partition<T>, NCCHError> {
        let data = self.plain_region;
        self.get_partition(data)
    }

    pub fn get_logo_region(self) -> Result<Partition<T>, NCCHError> {
        let data = self.logo_region;
        self.get_partition(data)
    }

    pub fn get_exefs(self) -> Result<Partition<T>, NCCHError> {
        let data = self.exefs;
        self.get_partition(data)
    }

    pub fn get_romfs(self) -> Result<Partition<T>, NCCHError> {
        let data = self.romfs;
        self.get_partition(data)
    }

    fn get_partition(self, partdata: PartitionData) -> Result<Partition<T>, NCCHError> {
        match Partition::new(self.file, partdata.offset, partdata.lenght) {
            Ok(value) => Ok(value),
            Err(err) => Err(NCCHError::CreatePartitionError(err)),
        }
    }
}
