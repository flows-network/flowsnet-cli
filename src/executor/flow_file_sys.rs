use std::{
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use slab::Slab;
use wasmedge_sdk::r#async::wasi::async_wasi::snapshots::env::{vfs::*, wasi_types, Errno};

fn get_file_ino(metadata: &std::fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::prelude::MetadataExt;
        metadata.ino()
    }
    #[cfg(not(unix))]
    {
        0
    }
}

fn get_file_nlink(metadata: &std::fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::prelude::MetadataExt;
        metadata.nlink()
    }
    #[cfg(not(unix))]
    {
        1
    }
}

fn systimespec(
    set: bool,
    ts: wasi_types::__wasi_timestamp_t,
    now: bool,
) -> Result<Option<SystemTimeSpec>, Errno> {
    if set && now {
        Err(Errno::__WASI_ERRNO_INVAL)
    } else if set {
        Ok(Some(SystemTimeSpec::Absolute(Duration::from_nanos(ts))))
    } else if now {
        Ok(Some(SystemTimeSpec::SymbolicNow))
    } else {
        Ok(None)
    }
}

#[derive(Debug)]
pub struct DiskDir {
    // absolutize
    pub real_path: PathBuf,
    pub dir_rights: WASIRights,
    pub file_rights: WASIRights,
}

impl DiskDir {
    pub fn get_absolutize_path<P: AsRef<Path>>(&self, sub_path: &P) -> Result<PathBuf, Errno> {
        use path_absolutize::*;
        let new_path = self.real_path.join(sub_path);
        let absolutize = new_path
            .absolutize_virtually(&self.real_path)
            .or(Err(Errno::__WASI_ERRNO_NOENT))?;
        Ok(absolutize.to_path_buf())
    }
}

impl WasiNode for DiskDir {
    fn fd_fdstat_get(&self) -> Result<FdStat, Errno> {
        Ok(FdStat {
            filetype: FileType::DIRECTORY,
            fs_rights_base: self.dir_rights.clone(),
            fs_rights_inheriting: self.file_rights.clone(),
            flags: FdFlags::empty(),
        })
    }

    fn fd_fdstat_set_flags(&mut self, _flags: FdFlags) -> Result<(), Errno> {
        Err(Errno::__WASI_ERRNO_BADF)
    }

    fn fd_fdstat_set_rights(
        &mut self,
        fs_rights_base: WASIRights,
        fs_rights_inheriting: WASIRights,
    ) -> Result<(), Errno> {
        self.dir_rights.can(fs_rights_base.clone())?;
        self.file_rights.can(fs_rights_inheriting.clone())?;

        self.dir_rights = fs_rights_base;
        self.file_rights = fs_rights_inheriting;

        Ok(())
    }

    fn fd_filestat_get(&self) -> Result<Filestat, Errno> {
        self.dir_rights.can(WASIRights::FD_FILESTAT_GET)?;
        let meta = std::fs::metadata(&self.real_path)?;
        let filetype = if meta.is_symlink() {
            FileType::SYMBOLIC_LINK
        } else {
            FileType::DIRECTORY
        };

        let nlink = get_file_nlink(&meta);
        let inode = get_file_ino(&meta);

        Ok(Filestat {
            filetype,
            nlink,
            inode,
            size: meta.len(),
            atim: meta.accessed().ok(),
            mtim: meta.modified().ok(),
            ctim: meta.created().ok(),
        })
    }

    fn fd_filestat_set_size(&mut self, _size: wasi_types::__wasi_filesize_t) -> Result<(), Errno> {
        Err(Errno::__WASI_ERRNO_BADF)
    }

    fn fd_filestat_set_times(
        &mut self,
        _atim: wasi_types::__wasi_timestamp_t,
        _mtim: wasi_types::__wasi_timestamp_t,
        _fst_flags: wasi_types::__wasi_fstflags_t::Type,
    ) -> Result<(), Errno> {
        self.dir_rights.can(WASIRights::FD_FILESTAT_SET_TIMES)?;
        Err(Errno::__WASI_ERRNO_NOSYS)
    }
}

impl WasiDir for DiskDir {
    fn get_readdir(&self, mut index: u64) -> Result<Vec<(String, u64, FileType)>, Errno> {
        self.dir_rights.can(WASIRights::FD_READDIR)?;

        let mut dirs = vec![];
        if index == 0 {
            let dir_meta = std::fs::metadata(&self.real_path)?;
            let dir_ino = get_file_ino(&dir_meta);
            dirs.push((".".to_string(), dir_ino, FileType::DIRECTORY));
            index += 1;
        }

        if index == 1 {
            let dir_ino = if let Some(parent) = self.real_path.parent() {
                let dir_meta = std::fs::metadata(parent)?;
                get_file_ino(&dir_meta)
            } else {
                0
            };
            dirs.push(("..".to_string(), dir_ino, FileType::DIRECTORY));
            index += 1;
        }

        let read_dir = self.real_path.read_dir()?;

        for dir_entity in read_dir.into_iter().skip((index - 2) as usize) {
            let dir_entity = dir_entity?;
            let name = dir_entity
                .file_name()
                .into_string()
                .map_err(|_| Errno::__WASI_ERRNO_ILSEQ)?;
            let metadata = dir_entity.metadata()?;
            let inode = get_file_ino(&metadata);

            let filetype = if metadata.is_dir() {
                FileType::DIRECTORY
            } else if metadata.is_symlink() {
                FileType::SYMBOLIC_LINK
            } else {
                FileType::REGULAR_FILE
            };

            dirs.push((name, inode, filetype));
        }

        Ok(dirs)
    }
}

const FILE_WRITE_LIMIT: u64 = 300 * 1024 * 1024;

#[derive(Debug)]
pub struct DiskFile {
    pub fd: std::fs::File,
    pub flags: FdFlags,
    pub right: WASIRights,
}

impl WasiNode for DiskFile {
    fn fd_fdstat_get(&self) -> Result<FdStat, Errno> {
        let meta = self.fd.metadata()?;
        let fd_flags = FdStat {
            filetype: if meta.is_symlink() {
                FileType::SYMBOLIC_LINK
            } else {
                FileType::REGULAR_FILE
            },
            fs_rights_base: self.right.clone(),
            fs_rights_inheriting: WASIRights::empty(),
            flags: self.flags.clone(),
        };
        Ok(fd_flags)
    }

    fn fd_fdstat_set_flags(&mut self, flags: FdFlags) -> Result<(), Errno> {
        self.right.can(WASIRights::FD_FDSTAT_SET_FLAGS)?;
        if flags.contains(FdFlags::NONBLOCK)
            && flags.intersects(FdFlags::DSYNC | FdFlags::SYNC | FdFlags::RSYNC)
        {
            return Err(Errno::__WASI_ERRNO_INVAL);
        }
        if flags.contains(FdFlags::APPEND) {
            return Err(Errno::__WASI_ERRNO_NOSYS);
        }
        self.flags = flags;
        Ok(())
    }

    fn fd_fdstat_set_rights(
        &mut self,
        fs_rights_base: WASIRights,
        _fs_rights_inheriting: WASIRights,
    ) -> Result<(), Errno> {
        self.right.can(fs_rights_base.clone())?;
        self.right = fs_rights_base;
        Ok(())
    }

    fn fd_filestat_get(&self) -> Result<Filestat, Errno> {
        self.right.can(WASIRights::FD_FILESTAT_GET)?;
        let meta = self.fd.metadata()?;
        let filetype = if meta.is_symlink() {
            FileType::SYMBOLIC_LINK
        } else {
            FileType::REGULAR_FILE
        };

        let nlink = get_file_nlink(&meta);
        let inode = get_file_ino(&meta);

        Ok(Filestat {
            filetype,
            nlink,
            inode,
            size: meta.len(),
            atim: meta.accessed().ok(),
            mtim: meta.modified().ok(),
            ctim: meta.created().ok(),
        })
    }

    fn fd_filestat_set_size(&mut self, size: wasi_types::__wasi_filesize_t) -> Result<(), Errno> {
        self.right.can(WASIRights::FD_FILESTAT_SET_SIZE)?;
        self.fd.set_len(size)?;
        Ok(())
    }

    fn fd_filestat_set_times(
        &mut self,
        atim: wasi_types::__wasi_timestamp_t,
        mtim: wasi_types::__wasi_timestamp_t,
        fst_flags: wasi_types::__wasi_fstflags_t::Type,
    ) -> Result<(), Errno> {
        use wasi_types::__wasi_fstflags_t;

        self.right.can(WASIRights::FD_FILESTAT_SET_TIMES)?;

        let set_atim = (fst_flags & __wasi_fstflags_t::__WASI_FSTFLAGS_ATIM) > 0;
        let set_atim_now = (fst_flags & __wasi_fstflags_t::__WASI_FSTFLAGS_ATIM_NOW) > 0;
        let set_mtim = (fst_flags & __wasi_fstflags_t::__WASI_FSTFLAGS_MTIM) > 0;
        let set_mtim_now = (fst_flags & __wasi_fstflags_t::__WASI_FSTFLAGS_MTIM_NOW) > 0;

        let atim = systimespec(set_atim, atim, set_atim_now)?;
        let mtim = systimespec(set_mtim, mtim, set_mtim_now)?;

        #[cfg(unix)]
        {
            use std::os::unix::prelude::AsRawFd;
            let fd = self.fd.as_raw_fd();
            let times = [
                {
                    match atim {
                        Some(SystemTimeSpec::Absolute(atim)) => libc::timespec {
                            tv_sec: atim.as_secs() as i64,
                            tv_nsec: atim.subsec_nanos() as i64,
                        },
                        Some(SystemTimeSpec::SymbolicNow) => libc::timespec {
                            tv_sec: 0,
                            tv_nsec: libc::UTIME_NOW,
                        },
                        None => libc::timespec {
                            tv_sec: 0,
                            tv_nsec: libc::UTIME_OMIT,
                        },
                    }
                },
                {
                    match mtim {
                        Some(SystemTimeSpec::Absolute(mtim)) => libc::timespec {
                            tv_sec: mtim.as_secs() as i64,
                            tv_nsec: mtim.subsec_nanos() as i64,
                        },
                        Some(SystemTimeSpec::SymbolicNow) => libc::timespec {
                            tv_sec: 0,
                            tv_nsec: libc::UTIME_NOW,
                        },
                        None => libc::timespec {
                            tv_sec: 0,
                            tv_nsec: libc::UTIME_OMIT,
                        },
                    }
                },
            ];
            if unsafe { libc::futimens(fd, times.as_ptr()) } < 0 {
                Err(std::io::Error::last_os_error())?;
            }
            Ok(())
        }
        #[cfg(not(unix))]
        {
            Err(Errno::__WASI_ERRNO_NOSYS)
        }
    }
}

impl WasiFile for DiskFile {
    fn fd_advise(
        &mut self,
        _offset: wasi_types::__wasi_filesize_t,
        _len: wasi_types::__wasi_filesize_t,
        _advice: Advice,
    ) -> Result<(), Errno> {
        Ok(())
    }

    fn fd_allocate(
        &mut self,
        offset: wasi_types::__wasi_filesize_t,
        len: wasi_types::__wasi_filesize_t,
    ) -> Result<(), Errno> {
        self.right.can(WASIRights::FD_ALLOCATE)?;
        let f = &mut self.fd;
        let metadata = f.metadata()?;
        let file_len = metadata.len();
        let new_len = offset + len;
        if new_len > file_len {
            let old_seek = f.stream_position()?;
            f.set_len(new_len)?;
            f.seek(std::io::SeekFrom::Start(old_seek))?;
        }
        Ok(())
    }

    fn fd_datasync(&mut self) -> Result<(), Errno> {
        self.right.can(WASIRights::FD_DATASYNC)?;
        self.fd.sync_data()?;
        Ok(())
    }

    fn fd_sync(&mut self) -> Result<(), Errno> {
        self.right.can(WASIRights::FD_SYNC)?;
        self.fd.sync_all()?;
        Ok(())
    }

    fn fd_read(&mut self, bufs: &mut [std::io::IoSliceMut<'_>]) -> Result<usize, Errno> {
        self.right.can(WASIRights::FD_READ)?;
        Ok(self.fd.read_vectored(bufs)?)
    }

    fn fd_pread(
        &mut self,
        bufs: &mut [std::io::IoSliceMut<'_>],
        offset: wasi_types::__wasi_filesize_t,
    ) -> Result<usize, Errno> {
        use std::io::SeekFrom;

        self.right.can(WASIRights::FD_READ | WASIRights::FD_SEEK)?;

        let old_seek = self.fd.stream_position()?;
        self.fd.seek(SeekFrom::Start(offset))?;
        let r = self.fd.read_vectored(bufs);
        self.fd.seek(SeekFrom::Start(old_seek))?;
        Ok(r?)
    }

    fn fd_write(&mut self, bufs: &[std::io::IoSlice<'_>]) -> Result<usize, Errno> {
        self.right.can(WASIRights::FD_WRITE)?;
        let buf_len = bufs
            .iter()
            .map(|b| b.len())
            .reduce(|x, y| x + y)
            .unwrap_or_default();
        if self.fd.metadata()?.len() + buf_len as u64 >= FILE_WRITE_LIMIT {
            return Err(Errno::__WASI_ERRNO_NOSPC);
        }
        Ok(self.fd.write_vectored(bufs)?)
    }

    fn fd_pwrite(
        &mut self,
        bufs: &[std::io::IoSlice<'_>],
        offset: wasi_types::__wasi_filesize_t,
    ) -> Result<usize, Errno> {
        use std::io::SeekFrom;

        self.right.can(WASIRights::FD_WRITE | WASIRights::FD_SEEK)?;

        let buf_len = bufs
            .iter()
            .map(|b| b.len())
            .reduce(|x, y| x + y)
            .unwrap_or_default();
        if offset + buf_len as u64 >= FILE_WRITE_LIMIT {
            return Err(Errno::__WASI_ERRNO_NOSPC);
        }

        let old_seek = self.fd.stream_position()?;
        self.fd.seek(SeekFrom::Start(offset))?;
        let r = self.fd.write_vectored(bufs);
        self.fd.seek(SeekFrom::Start(old_seek))?;
        Ok(r?)
    }

    fn fd_seek(
        &mut self,
        offset: wasi_types::__wasi_filedelta_t,
        whence: wasi_types::__wasi_whence_t::Type,
    ) -> Result<wasi_types::__wasi_filesize_t, Errno> {
        use std::io::SeekFrom;

        let required_rigth =
            if offset == 0 && whence == wasi_types::__wasi_whence_t::__WASI_WHENCE_CUR {
                WASIRights::FD_TELL
            } else {
                WASIRights::FD_TELL | WASIRights::FD_SEEK
            };

        self.right.can(required_rigth)?;

        let pos = match whence {
            wasi_types::__wasi_whence_t::__WASI_WHENCE_CUR => SeekFrom::Current(offset),
            wasi_types::__wasi_whence_t::__WASI_WHENCE_END => SeekFrom::End(offset),
            wasi_types::__wasi_whence_t::__WASI_WHENCE_SET => SeekFrom::Start(offset as u64),
            _ => return Err(Errno::__WASI_ERRNO_INVAL),
        };

        Ok(self.fd.seek(pos)?)
    }

    fn fd_tell(&mut self) -> Result<wasi_types::__wasi_filesize_t, Errno> {
        self.right.can(WASIRights::FD_TELL)?;
        Ok(self.fd.stream_position()?)
    }
}

pub enum DiskInode {
    Dir(DiskDir),
    File(DiskFile),
}

impl WasiNode for DiskInode {
    fn fd_fdstat_get(&self) -> Result<FdStat, Errno> {
        match self {
            DiskInode::Dir(inode) => inode.fd_fdstat_get(),
            DiskInode::File(inode) => inode.fd_fdstat_get(),
        }
    }

    fn fd_fdstat_set_flags(&mut self, flags: FdFlags) -> Result<(), Errno> {
        match self {
            DiskInode::Dir(inode) => inode.fd_fdstat_set_flags(flags),
            DiskInode::File(inode) => inode.fd_fdstat_set_flags(flags),
        }
    }

    fn fd_fdstat_set_rights(
        &mut self,
        fs_rights_base: WASIRights,
        fs_rights_inheriting: WASIRights,
    ) -> Result<(), Errno> {
        match self {
            DiskInode::Dir(inode) => {
                inode.fd_fdstat_set_rights(fs_rights_base, fs_rights_inheriting)
            }
            DiskInode::File(inode) => {
                inode.fd_fdstat_set_rights(fs_rights_base, fs_rights_inheriting)
            }
        }
    }

    fn fd_filestat_get(&self) -> Result<Filestat, Errno> {
        match self {
            DiskInode::Dir(inode) => inode.fd_filestat_get(),
            DiskInode::File(inode) => inode.fd_filestat_get(),
        }
    }

    fn fd_filestat_set_size(&mut self, size: wasi_types::__wasi_filesize_t) -> Result<(), Errno> {
        match self {
            DiskInode::Dir(inode) => inode.fd_filestat_set_size(size),
            DiskInode::File(inode) => inode.fd_filestat_set_size(size),
        }
    }

    fn fd_filestat_set_times(
        &mut self,
        atim: wasi_types::__wasi_timestamp_t,
        mtim: wasi_types::__wasi_timestamp_t,
        fst_flags: wasi_types::__wasi_fstflags_t::Type,
    ) -> Result<(), Errno> {
        match self {
            DiskInode::Dir(inode) => inode.fd_filestat_set_times(atim, mtim, fst_flags),
            DiskInode::File(inode) => inode.fd_filestat_set_times(atim, mtim, fst_flags),
        }
    }
}

pub struct DiskFileSys {
    real_path: PathBuf,
    inodes: Slab<DiskInode>,
    dir_rights: WASIRights,
    file_rights: WASIRights,
}

impl DiskFileSys {
    pub fn new(host_path: PathBuf) -> std::io::Result<Self> {
        let host_path = host_path.canonicalize()?;
        let mut inodes = Slab::new();

        inodes.insert(DiskInode::Dir(DiskDir {
            real_path: host_path.clone(),
            dir_rights: WASIRights::dir_all(),
            file_rights: WASIRights::fd_all(),
        }));

        Ok(DiskFileSys {
            inodes,
            real_path: host_path,
            dir_rights: WASIRights::dir_all(),
            file_rights: WASIRights::fd_all(),
        })
    }
}

impl WasiFileSys for DiskFileSys {
    type Index = usize;

    fn path_open(
        &mut self,
        dir_ino: Self::Index,
        path: &str,
        oflags: OFlags,
        fs_rights_base: WASIRights,
        fs_rights_inheriting: WASIRights,
        fdflags: FdFlags,
    ) -> Result<Self::Index, Errno> {
        if fdflags.intersects(FdFlags::DSYNC | FdFlags::SYNC | FdFlags::RSYNC) {
            return Err(Errno::__WASI_ERRNO_NOSYS);
        }

        if oflags.intersects(OFlags::DIRECTORY)
            && (oflags.contains(OFlags::CREATE)
                || oflags.contains(OFlags::EXCLUSIVE)
                || oflags.contains(OFlags::TRUNCATE))
        {
            return Err(Errno::__WASI_ERRNO_INVAL);
        }

        let parent_dir = match self.inodes.get(dir_ino).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::Dir(dir) => dir,
            _ => return Err(Errno::__WASI_ERRNO_NOTDIR),
        };

        let path = parent_dir.get_absolutize_path(&path)?;
        if path == self.real_path {
            return Ok(0);
        }

        log::trace!("DiskFileSys path_open({path:?},{oflags:?})");

        let path_meta = std::fs::metadata(&path).ok();
        match path_meta {
            Some(meta) if meta.is_dir() => {
                let dir_rights = self.dir_rights.clone() & fs_rights_base;
                let file_rights = self.file_rights.clone() & fs_rights_inheriting;
                let ino = self.inodes.insert(DiskInode::Dir(DiskDir {
                    real_path: path,
                    dir_rights,
                    file_rights,
                }));
                return Ok(ino);
            }
            _ => {
                if oflags.contains(OFlags::DIRECTORY) {
                    return Err(Errno::__WASI_ERRNO_NOTDIR);
                }
            }
        }

        let read = fs_rights_base.contains(WASIRights::FD_READ);
        let write = fs_rights_base.contains(WASIRights::FD_WRITE);

        let mut opts = std::fs::OpenOptions::new();

        if oflags.contains(OFlags::CREATE | OFlags::EXCLUSIVE) {
            opts.create_new(true);
            opts.write(true);
        } else if oflags.contains(OFlags::CREATE) {
            opts.create(true);
            opts.write(true);
        }

        if oflags.contains(OFlags::TRUNCATE) {
            opts.truncate(true);
        }
        if read {
            opts.read(true);
        }

        if write {
            opts.write(true);
        } else {
            opts.read(true);
        }

        if fdflags.contains(FdFlags::APPEND) {
            opts.append(true);
        }

        let fd = opts.open(path)?;

        let ino = self.inodes.insert(DiskInode::File(DiskFile {
            fd,
            flags: fdflags,
            right: fs_rights_base,
        }));

        Ok(ino)
    }

    fn path_rename(
        &mut self,
        old_dir: usize,
        old_path: &str,
        new_dir: usize,
        new_path: &str,
    ) -> Result<(), Errno> {
        let old_parent_dir = match self.inodes.get(old_dir).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::Dir(dir) => dir,
            _ => return Err(Errno::__WASI_ERRNO_NOTDIR),
        };

        let old_path = old_parent_dir.get_absolutize_path(&old_path)?;

        let new_parent_dir = match self.inodes.get(new_dir).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::Dir(dir) => dir,
            _ => return Err(Errno::__WASI_ERRNO_NOTDIR),
        };
        let new_path = new_parent_dir.get_absolutize_path(&new_path)?;

        Ok(std::fs::rename(old_path, new_path)?)
    }

    fn path_create_directory(&mut self, dir_ino: Self::Index, path: &str) -> Result<(), Errno> {
        self.dir_rights.can(WASIRights::PATH_CREATE_DIRECTORY)?;
        let parent_dir = match self.inodes.get(dir_ino).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::Dir(dir) => dir,
            _ => return Err(Errno::__WASI_ERRNO_NOTDIR),
        };
        let new_path = parent_dir.get_absolutize_path(&path)?;
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(new_path)?;
        Ok(())
    }

    fn path_remove_directory(&mut self, dir_ino: Self::Index, path: &str) -> Result<(), Errno> {
        self.dir_rights.can(WASIRights::PATH_REMOVE_DIRECTORY)?;
        let parent_dir = match self.inodes.get(dir_ino).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::Dir(dir) => dir,
            _ => return Err(Errno::__WASI_ERRNO_NOTDIR),
        };
        let new_path = parent_dir.get_absolutize_path(&path)?;
        log::trace!("DiskFileSys path_remove_directory {new_path:?}");
        std::fs::remove_dir(new_path)?;
        Ok(())
    }

    fn path_unlink_file(&mut self, dir_ino: Self::Index, path: &str) -> Result<(), Errno> {
        self.dir_rights.can(WASIRights::PATH_REMOVE_DIRECTORY)?;
        let parent_dir = match self.inodes.get(dir_ino).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::Dir(dir) => dir,
            _ => return Err(Errno::__WASI_ERRNO_NOTDIR),
        };
        let new_path = parent_dir.get_absolutize_path(&path)?;
        std::fs::remove_file(new_path)?;
        Ok(())
    }

    fn path_link_file(
        &mut self,
        _old_dir: Self::Index,
        _old_path: &str,
        _new_dir: Self::Index,
        _new_path: &str,
    ) -> Result<(), Errno> {
        Err(Errno::__WASI_ERRNO_NOSYS)
    }

    fn path_filestat_get(
        &self,
        dir_ino: Self::Index,
        path: &str,
        follow_symlinks: bool,
    ) -> Result<Filestat, Errno> {
        self.dir_rights.can(WASIRights::PATH_FILESTAT_GET)?;

        let parent_dir = match self.inodes.get(dir_ino).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::Dir(dir) => dir,
            _ => return Err(Errno::__WASI_ERRNO_NOTDIR),
        };
        let new_path = parent_dir.get_absolutize_path(&path)?;

        let meta = if follow_symlinks {
            std::fs::metadata(new_path)?
        } else {
            std::fs::symlink_metadata(new_path)?
        };

        let filetype = if meta.is_symlink() {
            FileType::SYMBOLIC_LINK
        } else if meta.is_dir() {
            FileType::DIRECTORY
        } else {
            FileType::REGULAR_FILE
        };

        let nlink = get_file_nlink(&meta);
        let inode = get_file_ino(&meta);

        Ok(Filestat {
            filetype,
            inode,
            nlink,
            size: meta.len(),
            atim: meta.accessed().ok(),
            mtim: meta.modified().ok(),
            ctim: meta.created().ok(),
        })
    }

    fn fclose(&mut self, ino: Self::Index) -> Result<(), Errno> {
        self.inodes.try_remove(ino);
        Ok(())
    }

    fn get_mut_inode(&mut self, ino: usize) -> Result<&mut dyn WasiNode, Errno> {
        Ok(self.inodes.get_mut(ino).ok_or(Errno::__WASI_ERRNO_BADF)?)
    }

    fn get_inode(&self, ino: usize) -> Result<&dyn WasiNode, Errno> {
        Ok(self.inodes.get(ino).ok_or(Errno::__WASI_ERRNO_BADF)?)
    }

    fn get_mut_file(&mut self, ino: usize) -> Result<&mut dyn WasiFile, Errno> {
        match self.inodes.get_mut(ino).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::File(f) => Ok(f),
            _ => Err(Errno::__WASI_ERRNO_ISDIR),
        }
    }

    fn get_file(&self, ino: usize) -> Result<&dyn WasiFile, Errno> {
        match self.inodes.get(ino).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::File(f) => Ok(f),
            _ => Err(Errno::__WASI_ERRNO_ISDIR),
        }
    }

    fn get_mut_dir(&mut self, ino: usize) -> Result<&mut dyn WasiDir, Errno> {
        match self.inodes.get_mut(ino).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::Dir(dir) => Ok(dir),
            _ => Err(Errno::__WASI_ERRNO_NOTDIR),
        }
    }

    fn get_dir(&self, ino: usize) -> Result<&dyn WasiDir, Errno> {
        match self.inodes.get(ino).ok_or(Errno::__WASI_ERRNO_BADF)? {
            DiskInode::Dir(dir) => Ok(dir),
            _ => Err(Errno::__WASI_ERRNO_NOTDIR),
        }
    }
}
