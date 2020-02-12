use crate::ivfc::FileMetadata;
use crate::ivfc::{DirectoryOrFile, IVFCError};
use crate::IVFCReader;
use crate::PartitionMutex;
use std::borrow::Cow;
use std::error::Error;
use std::fmt;
use std::io;
use std::io::{Read, Seek};

use std::path::PathBuf;
use std::sync::Arc;

use vfs::{OpenOptions, VFile, VMetadata, VPath, VFS};

pub struct IVFCVFS<T: 'static + Read + Seek + Send + Sync + fmt::Debug> {
    reader: Arc<IVFCReader<T>>,
}

impl<T: 'static + Read + Seek + Send + Sync + fmt::Debug> IVFCVFS<T> {
    pub fn new(reader: IVFCReader<T>) -> IVFCVFS<T> {
        IVFCVFS {
            reader: Arc::new(reader),
        }
    }
}

impl<T: 'static + Read + Seek + Send + Sync + fmt::Debug> VFS for IVFCVFS<T> {
    type PATH = IVFCVPATH<T>;
    type METADATA = IVFCMeta;
    type FILE = PartitionMutex<T>;

    fn path<A: Into<String>>(&self, path: A) -> Self::PATH {
        IVFCVPATH {
            reader: self.reader.clone(),
            path: PathBuf::from(path.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum IVFCMeta {
    File(u64),
    Dir,
}

impl VMetadata for IVFCMeta {
    fn is_dir(&self) -> bool {
        match self {
            Self::File(_) => false,
            Self::Dir => true,
        }
    }

    fn is_file(&self) -> bool {
        !self.is_dir()
    }

    fn len(&self) -> u64 {
        match self {
            Self::File(lenght) => *lenght,
            Self::Dir => 0,
        }
    }
}

#[derive(Debug)]
pub enum GetMetadataError {
    CantConvertOSStrToString,
    IVFCError(IVFCError),
    TryGetChildFile(FileMetadata),
}

impl GetMetadataError {
    #[allow(clippy::wrong_self_convention)]
    pub fn to_io_error(self) -> io::Error {
        io::Error::new(io::ErrorKind::NotFound, self)
    }
}

impl fmt::Display for GetMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CantConvertOSStrToString => write!(
                f,
                "impossible to convert an OSStr to a String in IVFCVPATh.exists"
            ),
            Self::IVFCError(_) => write!(f, "error while asking metadata to the IVFCReader object"),
            Self::TryGetChildFile(actual_file) => write!(
                f,
                "can't get a child of a file (file data: {:?})",
                actual_file
            ),
        }
    }
}

impl Error for GetMetadataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::IVFCError(err) => Some(err),
            Self::CantConvertOSStrToString => None,
            Self::TryGetChildFile(_) => None,
        }
    }
}

pub struct FileNameIterator<T: Read + Seek + Send + Sync + fmt::Debug> {
    child_iterator: std::vec::IntoIter<String>,
    child_of: IVFCVPATH<T>,
}

impl<T: Read + Seek + Send + Sync + std::fmt::Debug> FileNameIterator<T> {
    pub fn new(childs: Vec<String>, child_of: IVFCVPATH<T>) -> FileNameIterator<T> {
        FileNameIterator {
            child_iterator: childs.into_iter(),
            child_of,
        }
    }
}

impl<T: 'static + Read + Seek + Send + Sync + fmt::Debug> Iterator for FileNameIterator<T> {
    type Item = io::Result<Box<dyn VPath>>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.child_iterator.next() {
            Some(value) => Some(Ok(self.child_of.resolve(&value))),
            None => None,
        }
    }
}

#[derive(Debug)]
pub struct IVFCVPATH<T: Sync + Send + Read + Seek + fmt::Debug> {
    reader: Arc<IVFCReader<T>>,
    path: PathBuf,
}

impl<T: Sync + Send + Read + Seek + fmt::Debug> Clone for IVFCVPATH<T> {
    fn clone(&self) -> IVFCVPATH<T> {
        let new_path = self.path.clone();
        IVFCVPATH {
            reader: self.reader.clone(),
            path: new_path,
        }
    }
}

impl<T: 'static + Read + Seek + fmt::Debug + Sync + Send> IVFCVPATH<T> {
    pub fn new(reader: Arc<IVFCReader<T>>) -> IVFCVPATH<T> {
        IVFCVPATH {
            reader,
            path: PathBuf::new(),
        }
    }

    pub fn get_internal_meta(&self) -> Result<DirectoryOrFile, GetMetadataError> {
        let mut actual_meta = DirectoryOrFile::Dir(self.reader.first_dir_metadata.clone());
        for path_part in self.path.iter() {
            match actual_meta {
                DirectoryOrFile::Dir(actual_dir) => {
                    match self.reader.get_child(
                        &actual_dir,
                        match path_part.to_str() {
                            Some(value) => value,
                            None => return Err(GetMetadataError::CantConvertOSStrToString),
                        },
                    ) {
                        Ok(new_dir) => actual_meta = new_dir,
                        Err(err) => return Err(GetMetadataError::IVFCError(err)),
                    }
                }
                DirectoryOrFile::File(actual_file) => {
                    return Err(GetMetadataError::TryGetChildFile(actual_file))
                }
            }
        }
        Ok(actual_meta)
    }
}

fn return_ro_error<T>() -> io::Result<T> {
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "read only file system",
    ))
}

impl<T: 'static + Read + Seek + fmt::Debug + Sync + Send> VPath for IVFCVPATH<T> {
    fn open_with_options(&self, opt: &OpenOptions) -> io::Result<Box<dyn VFile>> {
        if opt.write || opt.create || opt.append || opt.truncate {
            return return_ro_error();
        };

        let file_meta = match self.get_internal_meta() {
            Ok(DirectoryOrFile::File(file_meta)) => file_meta,
            Ok(DirectoryOrFile::Dir(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trying to open a directory",
                ))
            }
            Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err)),
        };

        Ok(Box::new(
            match PartitionMutex::new(
                self.reader.file.clone(),
                self.reader.get_file_real_offset(&file_meta) as usize,
                file_meta.lenght_file_data as usize,
            ) {
                Ok(value) => value,
                Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err)),
            },
        ))
    }

    #[allow(clippy::type_complexity)]
    fn read_dir(&self) -> io::Result<Box<dyn Iterator<Item = io::Result<Box<dyn VPath>>>>> {
        let dir_meta = match self.get_internal_meta() {
            Ok(DirectoryOrFile::File(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "trying to list content for a file",
                ))
            }
            Ok(DirectoryOrFile::Dir(dir_meta)) => dir_meta,
            Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err)),
        };

        let child_list = match self.reader.list_child(&dir_meta) {
            Ok(value) => value,
            Err(err) => return Err(io::Error::new(io::ErrorKind::Other, err)),
        };

        Ok(Box::new(FileNameIterator::new(child_list, self.clone())))
    }

    fn mkdir(&self) -> io::Result<()> {
        return_ro_error()
    }

    fn rm(&self) -> io::Result<()> {
        return_ro_error()
    }

    fn rmrf(&self) -> io::Result<()> {
        return_ro_error()
    }

    fn file_name(&self) -> Option<String> {
        self.path.file_name()
    }

    fn extension(&self) -> Option<String> {
        self.path.extension()
    }

    fn resolve(&self, path: &String) -> Box<dyn VPath> {
        let mut new_path = self.path.clone();
        new_path.push(path);
        Box::new(IVFCVPATH {
            reader: self.reader.clone(),
            path: new_path,
        })
    }

    fn parent(&self) -> Option<Box<dyn VPath>> {
        let mut new_path = self.path.clone();
        if !new_path.pop() {
            return None;
        };
        Some(Box::new(IVFCVPATH {
            reader: self.reader.clone(),
            path: new_path,
        }))
    }

    fn to_string(&self) -> Cow<str> {
        format!("romfs://{:?}", self.path).into()
    }

    fn box_clone(&self) -> Box<dyn VPath> {
        let new_path = self.path.clone();
        Box::new(IVFCVPATH {
            reader: self.reader.clone(),
            path: new_path,
        })
    }

    fn to_path_buf(&self) -> Option<PathBuf> {
        Some(self.path.clone())
    }

    fn exists(&self) -> bool {
        self.get_internal_meta().is_ok()
    }

    fn metadata(&self) -> io::Result<Box<dyn VMetadata>> {
        let metadata = match self.get_internal_meta() {
            Ok(metadata) => metadata,
            Err(err) => return Err(err.to_io_error()),
        };

        Ok(Box::new(match metadata {
            DirectoryOrFile::Dir(_) => IVFCMeta::Dir,
            DirectoryOrFile::File(meta) => IVFCMeta::File(meta.lenght_file_data),
        }))
    }
}
