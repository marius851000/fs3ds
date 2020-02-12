use std::io;
use std::io::SeekFrom;
use std::io::{Read, Seek, Write};
use std::sync::{Arc, Mutex};

fn partition_read<T: Read + Seek>(
    buf: &mut [u8],
    file: &mut T,
    start: usize,
    end: usize,
    mut pointer: usize,
) -> (usize, io::Result<usize>) {
    let end_byte_absolute = buf.len() + pointer as usize;
    if end_byte_absolute >= end {
        let loop_total_nb = end - pointer;
        let mut single_value = [0];

        for loop_nb in 0..loop_total_nb {
            match file.read_exact(&mut single_value) {
                Ok(_) => (),
                Err(err) => {
                    let _ = file.seek(SeekFrom::Start(start as u64 + pointer as u64));
                    return (pointer, Err(err));
                }
            }
            pointer += 1;
            buf[loop_nb] = single_value[0];
        }
        (pointer, Ok(loop_total_nb))
    } else {
        (pointer, file.read(buf))
    }
}

fn partition_seek<T: Read + Seek>(
    file: &mut T,
    start: usize,
    end: usize,
    pointer: usize,
    target: SeekFrom,
) -> (usize, io::Result<u64>) {
    let new_real_pos = match target {
        SeekFrom::Start(nb) => start + nb as usize,
        SeekFrom::End(nb) => (end as i64 + nb) as usize,
        SeekFrom::Current(nb) => ((start + pointer) as i64 + nb) as usize,
    };
    if new_real_pos < start {
        return (
            pointer,
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "can't seek before the beggining of the partition",
            )),
        );
    };
    // do not block seeking post-partition, as it will be caught by read
    match file.seek(SeekFrom::Start(new_real_pos as u64)) {
        Ok(_) => (),
        Err(err) => return (pointer, Err(err)),
    };
    (pointer, Ok(new_real_pos as u64 - start as u64))
}

#[derive(Debug)]
pub struct Partition<T: Read + Seek> {
    file: T,
    /// The offset of the first byte that should be included
    start: usize,
    pointer: usize,
    /// The offset of the first byte that should be NOT included
    end: usize,
}

impl<T: Read + Seek> Partition<T> {
    pub fn new(file: T, start: u32, lenght: u32) -> io::Result<Partition<T>> {
        let mut result = Partition {
            file,
            start: start as usize,
            pointer: start as usize,
            end: start as usize + lenght as usize,
        };
        result.seek(SeekFrom::Start(0))?;
        Ok(result)
    }
}

impl<T: Read + Seek + std::fmt::Debug> Read for Partition<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let end_byte_absolute = self.pointer as usize + buf.len();
        if end_byte_absolute >= self.end {
            let loop_total_nb = self.end - self.pointer;
            let mut single_value = [0];

            for loop_nb in 0..loop_total_nb {
                match self.file.read_exact(&mut single_value) {
                    Ok(_) => (),
                    Err(err) => {
                        let _ = self
                            .file
                            .seek(SeekFrom::Start(self.start as u64 + self.pointer as u64));
                        return Err(err);
                    }
                }
                self.pointer += 1;
                buf[loop_nb] = single_value[0];
            }

            Ok(loop_total_nb)
        } else {
            self.pointer = end_byte_absolute;
            self.file.read(buf)
        }
    }
}

impl<T: Seek + Read> Seek for Partition<T> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_real_pos = match pos {
            SeekFrom::Start(nb) => self.start + nb as usize,
            SeekFrom::End(nb) => (self.end as i64 + nb) as usize,
            SeekFrom::Current(nb) => ((self.start + self.pointer) as i64 + nb) as usize,
        };
        if new_real_pos < self.start {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "can't seek before the beggining of the partition",
            ));
        };
        // do not block seeking post-partition, as it will be caught by read
        self.file.seek(SeekFrom::Start(new_real_pos as u64))?;
        self.pointer = new_real_pos;
        Ok(self.pointer as u64 - self.start as u64)
    }
}

#[derive(Debug)]
pub struct PartitionMutex<T: Read + Seek> {
    file: Arc<Mutex<T>>,
    start: usize,
    pointer: usize,
    end: usize,
}

impl<T: Read + Seek> PartitionMutex<T> {
    pub fn new(file: Arc<Mutex<T>>, start: usize, lenght: usize) -> io::Result<PartitionMutex<T>> {
        let mut result = PartitionMutex {
            file,
            start,
            pointer: start,
            end: start + lenght,
        };
        result.seek(SeekFrom::Start(0))?;
        Ok(result)
    }
}

impl<T: Read + Seek> Read for PartitionMutex<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut file = match self.file.lock() {
            Ok(value) => value,
            Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "the fie mutex is poisoned",
                ))
            }
        };
        let result = partition_read(buf, &mut *file, self.start, self.end, self.pointer);
        self.pointer = result.0;
        result.1
    }
}

impl<T: Read + Seek> Seek for PartitionMutex<T> {
    fn seek(&mut self, target: SeekFrom) -> io::Result<u64> {
        let mut file = match self.file.lock() {
            Ok(value) => value,
            Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "the fie mutex is poisoned",
                ))
            }
        };
        let result = partition_seek(&mut *file, self.start, self.end, self.pointer, target);
        self.pointer = result.0;
        result.1
    }
}

impl<T: Read + Seek> Write for PartitionMutex<T> {
    /// Do not use this write function. It is just here to make ``vfs::VFile`` happy. It will always return an error.
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }

    /// Always suceed. It is useless to call it
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

//TODO: make a test, and find a type that implement Read + Seek for in memory data
/*
#[test]
fn test_partition_read() {
    let mut partition = Partition::new(content, 4, 6);
    let mut buf = [0; 4];
    partition.read(&mut buf);
    assert_eq!(buf, [4,5,6,7]);

    buf = [0; 4];
    partition.read(&mut buf);
    assert_eq!(buf, [8,9,0,0]);

    buf = [0; 4];
    partition.read(&mut buf);
    assert_eq!(buf, [0,0,0,0]);
}
*/
