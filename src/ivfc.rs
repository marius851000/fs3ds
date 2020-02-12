use std::error::Error;
use std::fmt;
use std::io;

use std::io::SeekFrom;
use std::io::{Read, Seek};
use std::string::FromUtf16Error;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub enum IVFCError {
    ReadError(io::Error, &'static str),
    SeekError(io::Error, &'static str),
    FirstMagicError([u8; 4]),
    SecondMagicError([u8; 4]),
    Level3HeaderLenghtInvalid(u32),
    UTF16LenghtNonMultiple2(&'static str, u32), // what, lenght
    ToUTF16Error(FromUtf16Error, &'static str),
    DirNotFound,
    FileNotFound,
    Poisoned,
}

impl Error for IVFCError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadError(err, _) => Some(err),
            Self::SeekError(err, _) => Some(err),
            Self::ToUTF16Error(err, _) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for IVFCError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadError(_err, what_failed) => write!(
                f,
                "failed to read the {} due to an error in the source input",
                what_failed
            ),
            Self::SeekError(_err, what_failed) => write!(
                f,
                "failed to seek to the \"{}\" due to an error in the source input",
                what_failed
            ),
            Self::FirstMagicError(first_magic) => write!(
                f,
                "failed to read the first magic. Found {:?}, expected  [73, 86, 70, 67].",
                first_magic
            ),
            Self::SecondMagicError(second_magic) => write!(
                f,
                "failed to read the second magic. Found {:?}, expected [0, 0, 0, 0].",
                second_magic
            ),
            Self::Level3HeaderLenghtInvalid(level_3_header_lenght) => write!(
                f,
                "the lenght of the header of the level 3 is not good. Found the lenght {}, expected 0x28.",
                level_3_header_lenght
            ),
            Self::UTF16LenghtNonMultiple2(what, lenght) => write!(
                f,
                "the lenght of a UTF16 string is not a multiple of 2. This error is found while decoding the {}. The lenght is {}.",
                what, lenght
            ),
            Self::DirNotFound => write!(
                f,
                "the directory is not found in the hiearchy"
            ),
            Self::FileNotFound => write!(
                f,
                "the file is not found in the hiearchy"
            ),
            Self::Poisoned => write!(
                f,
                "the Mutex is poisoned"
            ),
            Self::ToUTF16Error(_, what) => write!(
                f,
                "Impossible to convert \"{}\" to an UTF16 String",
                what
            )
        }
    }
}

#[allow(non_snake_case)]
fn IVFC_read_u32<T: Read>(file: &mut T, what: &'static str) -> Result<u32, IVFCError> {
    let mut buffer = [0; 4];
    match file.read_exact(&mut buffer) {
        Ok(_) => (),
        Err(err) => return Err(IVFCError::ReadError(err, what)),
    };
    Ok(u32::from_le_bytes(buffer))
}

#[allow(non_snake_case)]
fn IVFC_read_u64<T: Read>(file: &mut T, what: &'static str) -> Result<u64, IVFCError> {
    let mut buffer = [0; 8];
    match file.read_exact(&mut buffer) {
        Ok(_) => (),
        Err(err) => return Err(IVFCError::ReadError(err, what)),
    };
    Ok(u64::from_le_bytes(buffer))
}

#[allow(non_snake_case)]
fn IVFC_read_utf_16<T: Read>(
    file: &mut T,
    lenght: u32,
    what: &'static str,
) -> Result<String, IVFCError> {
    if lenght % 2 != 0 {
        return Err(IVFCError::UTF16LenghtNonMultiple2(what, lenght));
    };

    let mut string_numbered = Vec::new();
    for _ in 0..lenght / 2 {
        let mut buffer = [0; 2];
        match file.read_exact(&mut buffer) {
            Ok(_) => (),
            Err(err) => return Err(IVFCError::ReadError(err, what)),
        };
        string_numbered.push(u16::from_le_bytes(buffer));
    }
    match String::from_utf16(&string_numbered) {
        Ok(value) => Ok(value),
        Err(err) => Err(IVFCError::ToUTF16Error(err, what)),
    }
}

#[derive(Clone, Debug)]
pub enum DirectoryOrFile {
    Dir(DirectoryMetadata),
    File(FileMetadata),
}

#[derive(Debug, Clone)]
pub struct DirectoryMetadata {
    pub offset_parent: Option<u32>,
    pub offset_next_sibling: Option<u32>,
    pub offset_first_subdir: Option<u32>,
    pub offset_first_file: Option<u32>,
    pub name: Option<String>,
}

impl DirectoryMetadata {
    pub fn new<T: Read + Seek>(
        file: &mut T,
        is_root: bool,
    ) -> Result<DirectoryMetadata, IVFCError> {
        let offset_parent = Some(IVFC_read_u32(
            file,
            "offset of the parent directory in a directory metadata",
        )?);
        let offset_next_sibling = match IVFC_read_u32(
            file,
            "offset of the next sibling directory in a directory metadata",
        )? {
            0xFFFF_FFFF => None,
            value => Some(value),
        };
        let offset_first_subdir = match IVFC_read_u32(
            file,
            "offset of the first subdirectory in a directory metadata",
        )? {
            0xFFFF_FFFF => None,
            value => Some(value),
        };
        let offset_first_file =
            match IVFC_read_u32(file, "offset of the first file in a directory metadata")? {
                0xFFFF_FFFF => None,
                value => Some(value),
            };
        let _ = IVFC_read_u32(
            file,
            "offset of the next directory in the same hash table in a directory metadata",
        )?;

        let name;
        if !is_root {
            let name_lenght = IVFC_read_u32(file, "lenght of the name of a directory")?;
            //let physical_name_lenght = (((name_lenght as f32)/4.0).ceil()*4.0+0.01) as u32;
            name = Some(IVFC_read_utf_16(file, name_lenght, "directory name")?);
        } else {
            name = None;
        };
        Ok(DirectoryMetadata {
            offset_parent,
            offset_next_sibling,
            offset_first_subdir,
            offset_first_file,
            name,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub offset_parent: u32,
    pub offset_sibling: Option<u32>,
    pub offset_file_data: u64,
    pub lenght_file_data: u64,
    pub name: String,
}

impl FileMetadata {
    fn new<T: Read + Seek>(file: &mut T) -> Result<FileMetadata, IVFCError> {
        let offset_parent = IVFC_read_u32(file, "an offset of the parent of a file metadata")?;
        let offset_sibling = match IVFC_read_u32(file, "an offset the sibling of a file metadata")?
        {
            0xFFFF_FFFF => None,
            offset => Some(offset),
        };
        let offset_file_data = IVFC_read_u64(file, "the offset of a file in a file metadata")?;
        let lenght_file_data = IVFC_read_u64(file, "the lenght of a file in a file metadata")?;
        let _ = IVFC_read_u32(
            file,
            "the offset of the next file in it's Hash Table Bucket in a file metadata",
        )?;
        let name_lenght = IVFC_read_u32(file, "the lenght of a name of a file")?;
        let name = IVFC_read_utf_16(file, name_lenght, "file name")?;
        Ok(FileMetadata {
            offset_parent,
            offset_sibling,
            offset_file_data,
            lenght_file_data,
            name,
        })
    }
}

#[derive(Debug)]
pub struct IVFCReader<T: Read + Seek> {
    pub file: Arc<Mutex<T>>,
    pub dir_metadata_part_offset: u32,
    pub file_metadata_part_offset: u32,
    pub first_dir_metadata: DirectoryMetadata,
    pub file_data_offset: u32,
}

impl<T: Read + Seek> IVFCReader<T> {
    pub fn new(mut file: T) -> Result<IVFCReader<T>, IVFCError> {
        // magic "IVFC"
        let mut magic_1 = [0; 4];
        match file.read_exact(&mut magic_1) {
            Ok(_) => (),
            Err(err) => return Err(IVFCError::ReadError(err, "magic \"IVFC\"")),
        };

        if magic_1 != [73, 86, 70, 67] {
            return Err(IVFCError::FirstMagicError(magic_1));
        };

        // magic 0x10000
        let mut magic_2 = [0; 4];
        match file.read_exact(&mut magic_2) {
            Ok(_) => (),
            Err(err) => return Err(IVFCError::ReadError(err, "magic 0x00010000")),
        };

        if magic_2 != [0, 0, 1, 0] {
            return Err(IVFCError::SecondMagicError(magic_2));
        };
        // seek to the table 3

        let offset_table_3 = 4096;

        match file.seek(SeekFrom::Start(offset_table_3 as u64)) {
            Ok(_) => (),
            Err(err) => return Err(IVFCError::SeekError(err, "level 3")),
        };

        // check we are in the good section

        let level_3_header_lenght = IVFC_read_u32(&mut file, "level 3 header lenght")?;

        if level_3_header_lenght != 0x28 {
            return Err(IVFCError::Level3HeaderLenghtInvalid(level_3_header_lenght));
        };

        // read header information

        let _relative_offset_dir_hashdata =
            IVFC_read_u32(&mut file, "offset of the directory hashdata")?;
        let _dir_hashdata_lenght = IVFC_read_u32(&mut file, "lenght of the directory hashdata")?;

        let relative_offset_dir_metadata =
            IVFC_read_u32(&mut file, "offset of the directory metadata")?;
        let _dir_metadata_lenght = IVFC_read_u32(&mut file, "lenght of the directory metadata")?;
        let dir_metadata_part_offset = offset_table_3 + relative_offset_dir_metadata;

        let _relative_offset_file_hashdata =
            IVFC_read_u32(&mut file, "offset of the file hashdata")?;
        let _file_hashdata_lenght = IVFC_read_u32(&mut file, "lenght of the file hashdata")?;

        let relative_offset_file_metadata =
            IVFC_read_u32(&mut file, "offset of the file metadata")?;

        let file_metadata_part_offset = offset_table_3 + relative_offset_file_metadata;

        let _lenght_file_metadata = IVFC_read_u32(&mut file, "lenght of the file metadata")?;
        let file_data_offset = IVFC_read_u32(&mut file, "file data offset")? + offset_table_3;

        // Seek to root directory
        match file.seek(SeekFrom::Start((dir_metadata_part_offset) as u64)) {
            Ok(_) => (),
            Err(err) => return Err(IVFCError::SeekError(err, "first directory metadata")),
        };

        // parse it
        let first_dir_metadata = DirectoryMetadata::new(&mut file, true)?;

        Ok(IVFCReader {
            file: Arc::new(Mutex::new(file)),
            dir_metadata_part_offset,
            file_metadata_part_offset,
            first_dir_metadata,
            file_data_offset,
        })
    }

    /// Return a child by it's name. It may either be a folder or a file
    pub fn get_child(
        &self,
        dir: &DirectoryMetadata,
        path: &str,
    ) -> Result<DirectoryOrFile, IVFCError> {
        let mut file = match self.file.lock() {
            Ok(guard) => guard,
            Err(_err) => return Err(IVFCError::Poisoned),
        };
        // check for folder
        match file.seek(SeekFrom::Start(match dir.offset_first_subdir {
            Some(value) => (value + self.dir_metadata_part_offset) as u64,
            None => return Err(IVFCError::DirNotFound),
        })) {
            Ok(_) => (),
            Err(err) => return Err(IVFCError::SeekError(err, "a directory metadata")),
        };
        let mut actual_subdir = DirectoryMetadata::new(&mut *file, false)?;
        loop {
            if actual_subdir.name.as_ref().unwrap() == path {
                return Ok(DirectoryOrFile::Dir(actual_subdir));
            };
            //get the next one
            let offset_to_seek = match actual_subdir.offset_next_sibling {
                Some(value) => (value + self.dir_metadata_part_offset) as u64,
                None => break,
            };
            match file.seek(SeekFrom::Start(offset_to_seek)) {
                Ok(_) => (),
                Err(err) => return Err(IVFCError::SeekError(err, "a directory metadata")),
            };
            actual_subdir = DirectoryMetadata::new(&mut *file, false)?;
        }
        //check for file
        // get the first sub-file
        match file.seek(SeekFrom::Start(match dir.offset_first_file {
            Some(value) => (value + self.file_metadata_part_offset) as u64,
            None => return Err(IVFCError::FileNotFound),
        })) {
            Ok(_) => (),
            Err(err) => return Err(IVFCError::SeekError(err, "a file metadata")),
        };
        let mut actual_file = FileMetadata::new(&mut *file)?;
        loop {
            if actual_file.name == path {
                return Ok(DirectoryOrFile::File(actual_file));
            };
            let offset_to_seek = match actual_file.offset_sibling {
                Some(value) => (value + self.file_metadata_part_offset) as u64,
                None => break,
            };
            match file.seek(SeekFrom::Start(offset_to_seek)) {
                Ok(_) => (),
                Err(err) => return Err(IVFCError::SeekError(err, "a file metadata")),
            };
            actual_file = FileMetadata::new(&mut *file)?;
        }
        Err(IVFCError::FileNotFound)
    }

    pub fn list_file_child(
        &self,
        dir: &DirectoryMetadata,
        childs: &mut Vec<String>,
    ) -> Result<(), IVFCError> {
        let mut file = match self.file.lock() {
            Ok(file) => file,
            Err(_) => return Err(IVFCError::Poisoned),
        };

        let first_child_offset = match dir.offset_first_file {
            Some(value) => value as u64,
            None => return Ok(()),
        } + self.file_metadata_part_offset as u64;

        match file.seek(SeekFrom::Start(first_child_offset)) {
            Ok(_) => (),
            Err(err) => {
                return Err(IVFCError::SeekError(
                    err,
                    "a file metadata for listing child files",
                ))
            }
        };

        let mut actual_file_metadata = FileMetadata::new(&mut *file)?;

        loop {
            childs.push(actual_file_metadata.name.clone());

            let sibling_file_offset = match actual_file_metadata.offset_sibling {
                Some(value) => value as u64,
                None => return Ok(()),
            } + self.file_metadata_part_offset as u64;

            match file.seek(SeekFrom::Start(sibling_file_offset)) {
                Ok(_) => (),
                Err(err) => {
                    return Err(IVFCError::SeekError(
                        err,
                        "a file metadata for listing child files on not the first loop",
                    ))
                }
            };

            actual_file_metadata = FileMetadata::new(&mut *file)?;
        }
    }

    pub fn list_dir_child(
        &self,
        dir: &DirectoryMetadata,
        childs: &mut Vec<String>,
    ) -> Result<(), IVFCError> {
        let mut file = match self.file.lock() {
            Ok(file) => file,
            Err(_) => return Err(IVFCError::Poisoned),
        };

        let first_dir_offset = match dir.offset_first_subdir {
            Some(value) => value as u64,
            None => return Ok(()),
        } + self.dir_metadata_part_offset as u64;

        match file.seek(SeekFrom::Start(first_dir_offset)) {
            Ok(_) => (),
            Err(err) => {
                return Err(IVFCError::SeekError(
                    err,
                    "a file directory for listing child files",
                ))
            }
        };

        let mut actual_dir_metadata = DirectoryMetadata::new(&mut *file, false)?;

        loop {
            childs.push(actual_dir_metadata.name.unwrap().clone());

            let sibling_dir_offset = match actual_dir_metadata.offset_next_sibling {
                Some(value) => value as u64,
                None => return Ok(()),
            } + self.dir_metadata_part_offset as u64;

            match file.seek(SeekFrom::Start(sibling_dir_offset)) {
                Ok(_) => (),
                Err(err) => {
                    return Err(IVFCError::SeekError(
                        err,
                        "a directory metadata for listing child directory on not the first loop",
                    ))
                }
            };

            actual_dir_metadata = DirectoryMetadata::new(&mut *file, false)?;
        }
    }

    pub fn list_child(&self, dir: &DirectoryMetadata) -> Result<Vec<String>, IVFCError> {
        let mut childs = Vec::new();
        self.list_file_child(dir, &mut childs)?;
        self.list_dir_child(dir, &mut childs)?;
        Ok(childs)
    }

    pub fn get_file_real_offset(&self, file: &FileMetadata) -> u64 {
        file.offset_file_data + self.file_data_offset as u64
    }
}
