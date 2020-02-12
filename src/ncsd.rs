use crate::Partition;
use crate::PartitionData;
use std::error::Error;
use std::fmt;
use std::io;
use std::io::{Read, Seek};

#[derive(Debug)]
pub enum NCSDError {
    SignatureReadError(io::Error),
    MagicReadError(io::Error),
    InvalidMagic([u8; 4]), // contain the invalid magic
    ReadSizeError(io::Error),
    MediaIDReadError(io::Error),
    PartitionTypeReadError(io::Error),
    CryptTypeReadError(io::Error),
    EncryptedRom,
    ReadPartitionOffsetError(io::Error, usize), // usize: partition_nb
    ReadPartitionLenghtError(io::Error, usize), // usize: partition_nb
    ReadExHeaderHashError(io::Error),
    ReadAdditionalHeaderSizeError(io::Error),
    SectorZeroOffsetReadError(io::Error),
    PartitionFlagReadError(io::Error),
    PartitionIdReadError(io::Error, usize), // usize: partition_nb
    InexistingPartition(usize),             // usize: partition_nb
    CreatePartitionFail(io::Error),
}

impl Error for NCSDError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NCSDError::SignatureReadError(ioerror) => Some(ioerror),
            NCSDError::MagicReadError(ioerror) => Some(ioerror),
            NCSDError::ReadSizeError(ioerror) => Some(ioerror),
            NCSDError::MediaIDReadError(ioerror) => Some(ioerror),
            NCSDError::PartitionTypeReadError(ioerror) => Some(ioerror),
            NCSDError::CryptTypeReadError(ioerror) => Some(ioerror),
            NCSDError::ReadPartitionOffsetError(ioerror, _) => Some(ioerror),
            NCSDError::ReadPartitionLenghtError(ioerror, _) => Some(ioerror),
            NCSDError::ReadExHeaderHashError(ioerror) => Some(ioerror),
            NCSDError::ReadAdditionalHeaderSizeError(ioerror) => Some(ioerror),
            NCSDError::SectorZeroOffsetReadError(ioerror) => Some(ioerror),
            NCSDError::PartitionFlagReadError(ioerror) => Some(ioerror),
            NCSDError::PartitionIdReadError(ioerror, _) => Some(ioerror),
            NCSDError::CreatePartitionFail(err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for NCSDError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NCSDError::SignatureReadError(_) => {
                write!(f, "Unable to read the signature of the CCI file")
            }
            NCSDError::MagicReadError(_) => write!(f, "Unable to read the magic of the CCI file"),
            NCSDError::InvalidMagic(magic) => write!(
                f,
                "The magic in the header of the CCI file is invalide (it's {:?})",
                magic
            ),
            NCSDError::ReadSizeError(_) => {
                write!(f, "Unable to read the size of the file in the CCI file")
            }
            _ => write!(f, "{:?}", self), //TODO: specific error message
        }
    }
}

pub struct NCSDReader<T: Read + Seek> {
    file: T,
    pub size: u32,
    pub media_id: u64,
    pub partition_type: u64,
    pub partitions_id: Vec<[u8; 8]>,
    pub partition_crypt_type: [u8; 8],
    partitions: Vec<PartitionData>,
}

impl<T: Read + Seek> NCSDReader<T> {
    pub fn new(mut file: T) -> Result<NCSDReader<T>, NCSDError> {
        //TODO: seek ok
        // signature
        let mut signature = [0; 0x100];
        match file.read_exact(&mut signature) {
            Ok(_) => (),
            Err(err) => return Err(NCSDError::SignatureReadError(err)),
        }; //TODO: check the signature

        // magic
        let mut magic = [0; 0x4];
        match file.read_exact(&mut magic) {
            Ok(_) => (),
            Err(err) => return Err(NCSDError::MagicReadError(err)),
        };

        if magic != [78, 67, 83, 68] {
            return Err(NCSDError::InvalidMagic(magic));
        };

        // size of the file
        let mut size_media_image = [0; 0x4];
        match file.read_exact(&mut size_media_image) {
            Ok(_) => (),
            Err(err) => return Err(NCSDError::ReadSizeError(err)),
        };

        let size = u32::from_le_bytes(size_media_image) * 0x200;

        // media id
        let mut media_id = [0; 0x8];
        match file.read_exact(&mut media_id) {
            Ok(_) => (),
            Err(err) => return Err(NCSDError::MediaIDReadError(err)),
        }

        let media_id = u64::from_le_bytes(media_id);

        // partition fs type
        let mut partition_type = [0; 0x8];
        match file.read_exact(&mut partition_type) {
            Ok(_) => (),
            Err(err) => return Err(NCSDError::PartitionTypeReadError(err)),
        }

        let partition_type = u64::from_le_bytes(partition_type);

        // crypt type
        let mut partition_crypt_type = [0; 0x8];
        match file.read_exact(&mut partition_crypt_type) {
            Ok(_) => (),
            Err(err) => return Err(NCSDError::CryptTypeReadError(err)),
        }

        if partition_crypt_type != [0; 8] {
            return Err(NCSDError::EncryptedRom);
        };

        // partition data
        let mut partitions = Vec::new();

        for partition_nb in 0..8 {
            let mut offset = [0; 0x4];
            match file.read_exact(&mut offset) {
                Ok(_) => (),
                Err(err) => return Err(NCSDError::ReadPartitionOffsetError(err, partition_nb)),
            };
            let offset = u32::from_le_bytes(offset) * 0x200;

            let mut lenght = [0; 0x4];
            match file.read_exact(&mut lenght) {
                Ok(_) => (),
                Err(err) => return Err(NCSDError::ReadPartitionLenghtError(err, partition_nb)),
            };
            let lenght = u32::from_le_bytes(lenght) * 0x200;

            partitions.push(PartitionData { offset, lenght });
        }

        // ex header
        let mut exheader_hash = [0; 0x20];
        match file.read_exact(&mut exheader_hash) {
            Ok(_) => (),
            Err(err) => return Err(NCSDError::ReadExHeaderHashError(err)),
        };
        //TODO: check the hash

        let mut additional_header_size = [0; 0x4];
        match file.read_exact(&mut additional_header_size) {
            Ok(_) => (),
            Err(err) => return Err(NCSDError::ReadAdditionalHeaderSizeError(err)),
        };

        let mut sector_zero_offset = [0; 0x4];
        match file.read_exact(&mut sector_zero_offset) {
            Ok(_) => (),
            Err(err) => return Err(NCSDError::SectorZeroOffsetReadError(err)),
        }

        let mut partition_flags = [0; 0x8];
        match file.read_exact(&mut partition_flags) {
            Ok(_) => (),
            Err(err) => return Err(NCSDError::PartitionFlagReadError(err)),
        }

        let mut partitions_id = Vec::new();

        for partition_nb in 0..8 {
            let mut partition_id = [0; 0x8];
            match file.read_exact(&mut partition_id) {
                Ok(_) => (),
                Err(err) => return Err(NCSDError::PartitionIdReadError(err, partition_nb)),
            };

            partitions_id.push(partition_id);
        }

        Ok(NCSDReader {
            file,
            size,
            media_id,
            partition_type,
            partitions_id,
            partition_crypt_type,
            partitions,
        })
    }

    pub fn load_partition(self, partition_nb: usize) -> Result<Partition<T>, NCSDError> {
        if partition_nb >= 8 {
            return Err(NCSDError::InexistingPartition(partition_nb));
        };
        let partition = &self.partitions[partition_nb];
        if partition.offset == 0 {
            return Err(NCSDError::InexistingPartition(partition_nb));
        };
        match Partition::new(self.file, partition.offset, partition.lenght) {
            Ok(value) => Ok(value),
            Err(err) => Err(NCSDError::CreatePartitionFail(err)),
        }
    }
}
