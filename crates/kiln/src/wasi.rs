//! A WASI preview 1 (`wasi_snapshot_preview1`) host over [`Vfs`].
//!
//! The host is plain Rust: [`Process::call`] takes an import's name, its
//! arguments as `i64`, and the guest's linear memory through [`Memory`].
//! The browser worker binds every import of a module to it; the tests drive
//! it with a byte vector.
//!
//! Supported: arguments and environment; standard input from a buffer;
//! standard output and error into buffers; files and directories (open,
//! read, write, seek, tell, close, readdir, stat, create and remove
//! directories, unlink, rename, truncate); the realtime and monotonic
//! clocks; random bytes; and `proc_exit`. Everything else returns `ENOSYS`.
//! Preopens are `/` (fd 3) and `.` (fd 4), both the workspace root.

use std::collections::BTreeMap;

use crate::vfs::{self, FsError, Vfs};

/// The guest's linear memory.
pub trait Memory {
    /// Copy `len` bytes from `ptr`.
    fn read(&self, ptr: u32, len: u32) -> Vec<u8>;
    /// Copy `data` to `ptr`.
    fn write(&mut self, ptr: u32, data: &[u8]);
}

impl Memory for Vec<u8> {
    fn read(&self, ptr: u32, len: u32) -> Vec<u8> {
        self[ptr as usize..(ptr + len) as usize].to_vec()
    }
    fn write(&mut self, ptr: u32, data: &[u8]) {
        self[ptr as usize..ptr as usize + data.len()].copy_from_slice(data);
    }
}

/// The errno values this host returns.
pub mod errno {
    pub const SUCCESS: i32 = 0;
    pub const BADF: i32 = 8;
    pub const EXIST: i32 = 20;
    pub const INVAL: i32 = 28;
    pub const ISDIR: i32 = 31;
    pub const NOENT: i32 = 44;
    pub const NOSYS: i32 = 52;
    pub const NOTDIR: i32 = 54;
    pub const NOTEMPTY: i32 = 55;
    pub const SPIPE: i32 = 70;
}

const FILETYPE_CHAR: u8 = 2;
const FILETYPE_DIR: u8 = 3;
const FILETYPE_FILE: u8 = 4;
const OFLAG_CREAT: i64 = 1;
const OFLAG_DIRECTORY: i64 = 2;
const OFLAG_EXCL: i64 = 4;
const OFLAG_TRUNC: i64 = 8;
const FDFLAG_APPEND: i64 = 1;
const RIGHTS_ALL: u64 = u64::MAX >> 35;

fn code(error: FsError) -> i32 {
    match error {
        FsError::NotFound => errno::NOENT,
        FsError::Exists => errno::EXIST,
        FsError::NotDir => errno::NOTDIR,
        FsError::IsDir => errno::ISDIR,
        FsError::NotEmpty => errno::NOTEMPTY,
        FsError::Invalid => errno::INVAL,
    }
}

/// What a call did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Return this errno to the guest.
    Return(i32),
    /// The guest called `proc_exit` with this code.
    Exit(i32),
}

#[derive(Debug, Clone)]
enum Fd {
    Stdin,
    Stdout,
    Stderr,
    /// A directory, preopened under `name` or opened by path.
    Dir {
        path: String,
        name: Option<String>,
    },
    File {
        path: String,
        pos: u64,
        append: bool,
    },
}

/// Clocks and randomness, injected so tests are deterministic.
pub trait Platform {
    /// Nanoseconds since the Unix epoch.
    fn realtime_ns(&self) -> u64;
    /// Nanoseconds on a monotonic clock.
    fn monotonic_ns(&self) -> u64;
    /// Fill `buf` with random bytes.
    fn random(&self, buf: &mut [u8]);
}

/// One running program's view: its arguments, environment, descriptors,
/// standard streams, and the workspace it changes.
pub struct Process<P: Platform> {
    pub vfs: Vfs,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// The most bytes kept from each output stream.
    pub output_cap: usize,
    args: Vec<String>,
    env: Vec<String>,
    stdin: Vec<u8>,
    stdin_pos: usize,
    fds: BTreeMap<u32, Fd>,
    next_fd: u32,
    platform: P,
    /// Calls by import name, for the findings.
    pub counts: BTreeMap<String, u64>,
}

fn u32_at(args: &[i64], i: usize) -> u32 {
    args.get(i).copied().unwrap_or(0) as u32
}

impl<P: Platform> Process<P> {
    /// A process that runs `args` (the program name first) over `vfs`.
    #[must_use]
    pub fn new(vfs: Vfs, args: Vec<String>, env: Vec<String>, stdin: Vec<u8>, platform: P) -> Self {
        let mut fds = BTreeMap::new();
        fds.insert(0, Fd::Stdin);
        fds.insert(1, Fd::Stdout);
        fds.insert(2, Fd::Stderr);
        fds.insert(
            3,
            Fd::Dir {
                path: String::new(),
                name: Some("/".into()),
            },
        );
        fds.insert(
            4,
            Fd::Dir {
                path: String::new(),
                name: Some(".".into()),
            },
        );
        Self {
            vfs,
            stdout: Vec::new(),
            stderr: Vec::new(),
            output_cap: 256 * 1024,
            args,
            env,
            stdin,
            stdin_pos: 0,
            fds,
            next_fd: 5,
            platform,
            counts: BTreeMap::new(),
        }
    }

    fn now(&self) -> u64 {
        self.platform.realtime_ns()
    }

    /// Run one `wasi_snapshot_preview1` import.
    pub fn call(&mut self, name: &str, args: &[i64], mem: &mut dyn Memory) -> Outcome {
        *self.counts.entry(name.to_string()).or_default() += 1;
        let a = |i| u32_at(args, i);
        let r = match name {
            "args_sizes_get" => Self::sizes(&self.args, a(0), a(1), mem),
            "args_get" => Self::strings(&self.args, a(0), a(1), mem),
            "environ_sizes_get" => Self::sizes(&self.env, a(0), a(1), mem),
            "environ_get" => Self::strings(&self.env, a(0), a(1), mem),
            "clock_res_get" => {
                mem.write(a(1), &1_000u64.to_le_bytes());
                errno::SUCCESS
            }
            "clock_time_get" => {
                let ns = if a(0) == 0 {
                    self.platform.realtime_ns()
                } else {
                    self.platform.monotonic_ns()
                };
                mem.write(a(2), &ns.to_le_bytes());
                errno::SUCCESS
            }
            "random_get" => {
                let mut buf = vec![0; a(1) as usize];
                self.platform.random(&mut buf);
                mem.write(a(0), &buf);
                errno::SUCCESS
            }
            "proc_exit" => return Outcome::Exit(args.first().copied().unwrap_or(0) as i32),
            "sched_yield" => errno::SUCCESS,
            "fd_prestat_get" => match self.fds.get(&a(0)) {
                Some(Fd::Dir {
                    name: Some(name), ..
                }) => {
                    let mut out = [0u8; 8];
                    out[4..].copy_from_slice(&(name.len() as u32).to_le_bytes());
                    mem.write(a(1), &out);
                    errno::SUCCESS
                }
                _ => errno::BADF,
            },
            "fd_prestat_dir_name" => match self.fds.get(&a(0)) {
                Some(Fd::Dir {
                    name: Some(name), ..
                }) => {
                    let bytes = name.as_bytes();
                    mem.write(a(1), &bytes[..bytes.len().min(a(2) as usize)]);
                    errno::SUCCESS
                }
                _ => errno::BADF,
            },
            "fd_fdstat_get" => self.fdstat(a(0), a(1), mem),
            "fd_fdstat_set_flags" | "fd_sync" | "fd_datasync" | "fd_advise" => {
                if self.fds.contains_key(&a(0)) {
                    errno::SUCCESS
                } else {
                    errno::BADF
                }
            }
            "fd_close" => {
                if a(0) > 4 && self.fds.remove(&a(0)).is_some() {
                    errno::SUCCESS
                } else if a(0) <= 4 {
                    errno::SUCCESS
                } else {
                    errno::BADF
                }
            }
            "fd_write" => self.fd_write(a(0), a(1), a(2), a(3), mem),
            "fd_read" => self.fd_read(a(0), a(1), a(2), a(3), mem),
            "fd_seek" => self.fd_seek(a(0), args.get(1).copied().unwrap_or(0), a(2), a(3), mem),
            "fd_tell" => match self.fds.get(&a(0)) {
                Some(Fd::File { pos, .. }) => {
                    mem.write(a(1), &pos.to_le_bytes());
                    errno::SUCCESS
                }
                Some(_) => errno::SPIPE,
                None => errno::BADF,
            },
            "fd_filestat_get" => match self.fds.get(&a(0)).cloned() {
                Some(Fd::File { path, .. } | Fd::Dir { path, .. }) => {
                    self.filestat(&path, a(1), mem)
                }
                Some(_) => {
                    Self::write_filestat(mem, a(1), FILETYPE_CHAR, 0, 0, 0);
                    errno::SUCCESS
                }
                None => errno::BADF,
            },
            "fd_filestat_set_size" => match self.fds.get(&a(0)).cloned() {
                Some(Fd::File { path, .. }) => self
                    .vfs
                    .truncate(&path, args.get(1).copied().unwrap_or(0) as u64)
                    .map_or_else(code, |()| errno::SUCCESS),
                _ => errno::BADF,
            },
            "fd_filestat_set_times" | "path_filestat_set_times" => errno::SUCCESS,
            "fd_readdir" => self.fd_readdir(
                a(0),
                a(1),
                a(2),
                args.get(3).copied().unwrap_or(0) as u64,
                a(4),
                mem,
            ),
            "path_open" => self.path_open(args, mem),
            "path_filestat_get" => match self.path(a(0), a(2), a(3), mem) {
                Ok(path) => self.filestat(&path, a(4), mem),
                Err(e) => e,
            },
            "path_create_directory" => match self.path(a(0), a(1), a(2), mem) {
                Ok(path) => {
                    let now = self.now();
                    self.vfs
                        .mkdir(&path, now)
                        .map_or_else(code, |()| errno::SUCCESS)
                }
                Err(e) => e,
            },
            "path_remove_directory" => match self.path(a(0), a(1), a(2), mem) {
                Ok(path) => self.vfs.rmdir(&path).map_or_else(code, |()| errno::SUCCESS),
                Err(e) => e,
            },
            "path_unlink_file" => match self.path(a(0), a(1), a(2), mem) {
                Ok(path) => self
                    .vfs
                    .unlink(&path)
                    .map_or_else(code, |()| errno::SUCCESS),
                Err(e) => e,
            },
            "path_rename" => match (
                self.path(a(0), a(1), a(2), mem),
                self.path(a(3), a(4), a(5), mem),
            ) {
                (Ok(from), Ok(to)) => self
                    .vfs
                    .rename(&from, &to)
                    .map_or_else(code, |()| errno::SUCCESS),
                (Err(e), _) | (_, Err(e)) => e,
            },
            "path_readlink" => errno::INVAL,
            _ => errno::NOSYS,
        };
        Outcome::Return(r)
    }

    fn sizes(list: &[String], count_ptr: u32, size_ptr: u32, mem: &mut dyn Memory) -> i32 {
        let size: usize = list.iter().map(|s| s.len() + 1).sum();
        mem.write(count_ptr, &(list.len() as u32).to_le_bytes());
        mem.write(size_ptr, &(size as u32).to_le_bytes());
        errno::SUCCESS
    }

    fn strings(list: &[String], ptrs: u32, buf: u32, mem: &mut dyn Memory) -> i32 {
        let mut at = buf;
        for (i, s) in list.iter().enumerate() {
            mem.write(ptrs + 4 * i as u32, &at.to_le_bytes());
            let mut bytes = s.as_bytes().to_vec();
            bytes.push(0);
            mem.write(at, &bytes);
            at += bytes.len() as u32;
        }
        errno::SUCCESS
    }

    fn fdstat(&self, fd: u32, ptr: u32, mem: &mut dyn Memory) -> i32 {
        let (filetype, flags) = match self.fds.get(&fd) {
            None => return errno::BADF,
            Some(Fd::Dir { .. }) => (FILETYPE_DIR, 0u16),
            Some(Fd::File { append, .. }) => (FILETYPE_FILE, u16::from(*append)),
            Some(_) => (FILETYPE_CHAR, 0),
        };
        let mut out = [0u8; 24];
        out[0] = filetype;
        out[2..4].copy_from_slice(&flags.to_le_bytes());
        out[8..16].copy_from_slice(&RIGHTS_ALL.to_le_bytes());
        out[16..24].copy_from_slice(&RIGHTS_ALL.to_le_bytes());
        mem.write(ptr, &out);
        errno::SUCCESS
    }

    fn write_filestat(
        mem: &mut dyn Memory,
        ptr: u32,
        filetype: u8,
        ino: u64,
        size: u64,
        time: u64,
    ) {
        let mut out = [0u8; 64];
        out[8..16].copy_from_slice(&ino.to_le_bytes());
        out[16] = filetype;
        out[24..32].copy_from_slice(&1u64.to_le_bytes());
        out[32..40].copy_from_slice(&size.to_le_bytes());
        for at in [40, 48, 56] {
            out[at..at + 8].copy_from_slice(&time.to_le_bytes());
        }
        mem.write(ptr, &out);
    }

    fn filestat(&self, path: &str, ptr: u32, mem: &mut dyn Memory) -> i32 {
        match self.vfs.get(path) {
            Some(node) => {
                let ino = path.bytes().fold(1469598103934665603u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(1099511628211)
                });
                let kind = if node.is_dir() {
                    FILETYPE_DIR
                } else {
                    FILETYPE_FILE
                };
                Self::write_filestat(mem, ptr, kind, ino, node.size(), node.mtime());
                errno::SUCCESS
            }
            None => errno::NOENT,
        }
    }

    /// The workspace path a `(dirfd, path)` pair names.
    fn path(&self, dirfd: u32, ptr: u32, len: u32, mem: &dyn Memory) -> Result<String, i32> {
        let base = match self.fds.get(&dirfd) {
            Some(Fd::Dir { path, .. }) => path.clone(),
            Some(_) => return Err(errno::NOTDIR),
            None => return Err(errno::BADF),
        };
        let raw = String::from_utf8(mem.read(ptr, len)).map_err(|_| errno::INVAL)?;
        vfs::resolve(&base, &raw).map_err(code)
    }

    fn iovecs(ptr: u32, count: u32, mem: &dyn Memory) -> Vec<(u32, u32)> {
        (0..count)
            .map(|i| {
                let raw = mem.read(ptr + 8 * i, 8);
                (
                    u32::from_le_bytes(raw[0..4].try_into().unwrap_or_default()),
                    u32::from_le_bytes(raw[4..8].try_into().unwrap_or_default()),
                )
            })
            .collect()
    }

    fn fd_write(
        &mut self,
        fd: u32,
        iovs: u32,
        count: u32,
        nwritten: u32,
        mem: &mut dyn Memory,
    ) -> i32 {
        let mut data = Vec::new();
        for (ptr, len) in Self::iovecs(iovs, count, mem) {
            data.extend(mem.read(ptr, len));
        }
        let cap = self.output_cap;
        match self.fds.get_mut(&fd) {
            Some(Fd::Stdout) => {
                let room = cap.saturating_sub(self.stdout.len());
                self.stdout.extend_from_slice(&data[..data.len().min(room)]);
            }
            Some(Fd::Stderr) => {
                let room = cap.saturating_sub(self.stderr.len());
                self.stderr.extend_from_slice(&data[..data.len().min(room)]);
            }
            Some(Fd::File { path, pos, append }) => {
                let path = path.clone();
                if *append {
                    *pos = self.vfs.get(&path).map_or(0, vfs::Node::size);
                }
                let at = *pos;
                *pos += data.len() as u64;
                let now = self.platform.realtime_ns();
                if let Err(e) = self.vfs.write_at(&path, at, &data, now) {
                    return code(e);
                }
            }
            Some(_) => return errno::BADF,
            None => return errno::BADF,
        }
        mem.write(nwritten, &(data.len() as u32).to_le_bytes());
        errno::SUCCESS
    }

    fn fd_read(&mut self, fd: u32, iovs: u32, count: u32, nread: u32, mem: &mut dyn Memory) -> i32 {
        let vecs = Self::iovecs(iovs, count, mem);
        let want: usize = vecs.iter().map(|(_, l)| *l as usize).sum();
        let data: Vec<u8> = match self.fds.get_mut(&fd) {
            Some(Fd::Stdin) => {
                let end = (self.stdin_pos + want).min(self.stdin.len());
                let out = self.stdin[self.stdin_pos..end].to_vec();
                self.stdin_pos = end;
                out
            }
            Some(Fd::File { path, pos, .. }) => match self.vfs.read(path) {
                Ok(bytes) => {
                    let start = (*pos as usize).min(bytes.len());
                    let end = (start + want).min(bytes.len());
                    *pos = end as u64;
                    bytes[start..end].to_vec()
                }
                Err(e) => return code(e),
            },
            Some(Fd::Dir { .. }) => return errno::ISDIR,
            Some(_) | None => return errno::BADF,
        };
        let mut at = 0;
        for (ptr, len) in vecs {
            let n = (len as usize).min(data.len() - at);
            mem.write(ptr, &data[at..at + n]);
            at += n;
            if at == data.len() {
                break;
            }
        }
        mem.write(nread, &(data.len() as u32).to_le_bytes());
        errno::SUCCESS
    }

    fn fd_seek(
        &mut self,
        fd: u32,
        offset: i64,
        whence: u32,
        out: u32,
        mem: &mut dyn Memory,
    ) -> i32 {
        let size = match self.fds.get(&fd) {
            Some(Fd::File { path, .. }) => self.vfs.get(path).map_or(0, vfs::Node::size),
            Some(_) => return errno::SPIPE,
            None => return errno::BADF,
        };
        let Some(Fd::File { pos, .. }) = self.fds.get_mut(&fd) else {
            return errno::BADF;
        };
        let base = match whence {
            0 => 0,
            1 => *pos as i64,
            2 => size as i64,
            _ => return errno::INVAL,
        };
        let next = base + offset;
        if next < 0 {
            return errno::INVAL;
        }
        *pos = next as u64;
        mem.write(out, &(next as u64).to_le_bytes());
        errno::SUCCESS
    }

    fn fd_readdir(
        &mut self,
        fd: u32,
        buf: u32,
        len: u32,
        cookie: u64,
        used: u32,
        mem: &mut dyn Memory,
    ) -> i32 {
        let path = match self.fds.get(&fd) {
            Some(Fd::Dir { path, .. }) => path.clone(),
            Some(_) => return errno::NOTDIR,
            None => return errno::BADF,
        };
        let entries = match self.vfs.list(&path) {
            Ok(entries) => entries,
            Err(e) => return code(e),
        };
        let mut out = Vec::new();
        for (i, (name, is_dir)) in entries.iter().enumerate().skip(cookie as usize) {
            let mut head = [0u8; 24];
            head[0..8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
            head[8..16].copy_from_slice(&(i as u64 + 1).to_le_bytes());
            head[16..20].copy_from_slice(&(name.len() as u32).to_le_bytes());
            head[20] = if *is_dir { FILETYPE_DIR } else { FILETYPE_FILE };
            out.extend_from_slice(&head);
            out.extend_from_slice(name.as_bytes());
            if out.len() >= len as usize {
                break;
            }
        }
        out.truncate(len as usize);
        mem.write(buf, &out);
        mem.write(used, &(out.len() as u32).to_le_bytes());
        errno::SUCCESS
    }

    fn path_open(&mut self, args: &[i64], mem: &mut dyn Memory) -> i32 {
        let a = |i| u32_at(args, i);
        let oflags = args.get(4).copied().unwrap_or(0);
        let fdflags = args.get(7).copied().unwrap_or(0);
        let path = match self.path(a(0), a(2), a(3), mem) {
            Ok(path) => path,
            Err(e) => return e,
        };
        let now = self.now();
        let fd = match self.vfs.get(&path) {
            Some(node) if node.is_dir() => {
                if oflags & (OFLAG_CREAT | OFLAG_TRUNC) != 0 && oflags & OFLAG_EXCL != 0 {
                    return errno::EXIST;
                }
                Fd::Dir { path, name: None }
            }
            Some(_) if oflags & OFLAG_DIRECTORY != 0 => return errno::NOTDIR,
            Some(_) if oflags & OFLAG_CREAT != 0 && oflags & OFLAG_EXCL != 0 => {
                return errno::EXIST;
            }
            Some(_) => {
                if oflags & OFLAG_TRUNC != 0
                    && let Err(e) = self.vfs.create(&path, true, now)
                {
                    return code(e);
                }
                Fd::File {
                    path,
                    pos: 0,
                    append: fdflags & FDFLAG_APPEND != 0,
                }
            }
            None if oflags & OFLAG_CREAT != 0 => {
                if let Err(e) = self.vfs.create(&path, false, now) {
                    return code(e);
                }
                Fd::File {
                    path,
                    pos: 0,
                    append: fdflags & FDFLAG_APPEND != 0,
                }
            }
            None => return errno::NOENT,
        };
        let n = self.next_fd;
        self.next_fd += 1;
        self.fds.insert(n, fd);
        mem.write(a(8), &n.to_le_bytes());
        errno::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;
    impl Platform for Fixed {
        fn realtime_ns(&self) -> u64 {
            7
        }
        fn monotonic_ns(&self) -> u64 {
            9
        }
        fn random(&self, buf: &mut [u8]) {
            buf.fill(4);
        }
    }

    fn put(mem: &mut Vec<u8>, at: u32, bytes: &[u8]) {
        mem.write(at, bytes);
    }

    fn u32_from(mem: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(mem[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn a_program_writes_a_file_and_reads_it_back() {
        let mut p = Process::new(Vfs::default(), vec!["t".into()], vec![], vec![], Fixed);
        let mut mem = vec![0u8; 4096];
        put(&mut mem, 100, b"out.txt");
        // path_open(dirfd 4, flags, path, len, CREAT|TRUNC, rights, rights, fdflags, fd_out)
        let open = [4, 0, 100, 7, 1 | 8, -1, -1, 0, 200];
        assert_eq!(p.call("path_open", &open, &mut mem), Outcome::Return(0));
        let fd = i64::from(u32_from(&mem, 200));
        put(&mut mem, 300, b"hello");
        put(&mut mem, 400, &300u32.to_le_bytes());
        put(&mut mem, 404, &5u32.to_le_bytes());
        assert_eq!(
            p.call("fd_write", &[fd, 400, 1, 500], &mut mem),
            Outcome::Return(0)
        );
        assert_eq!(u32_from(&mem, 500), 5);
        assert_eq!(p.vfs.read("out.txt").unwrap(), b"hello");
        assert_eq!(
            p.call("fd_seek", &[fd, 1, 0, 600], &mut mem),
            Outcome::Return(0)
        );
        put(&mut mem, 400, &700u32.to_le_bytes());
        put(&mut mem, 404, &10u32.to_le_bytes());
        assert_eq!(
            p.call("fd_read", &[fd, 400, 1, 500], &mut mem),
            Outcome::Return(0)
        );
        assert_eq!(u32_from(&mem, 500), 4);
        assert_eq!(&mem[700..704], b"ello");
        assert_eq!(p.call("fd_close", &[fd], &mut mem), Outcome::Return(0));
        assert_eq!(
            p.call("fd_close", &[fd], &mut mem),
            Outcome::Return(errno::BADF)
        );
    }

    #[test]
    fn standard_streams_args_and_exit() {
        let mut p = Process::new(
            Vfs::default(),
            vec!["cat".into(), "a".into()],
            vec!["K=V".into()],
            b"in".to_vec(),
            Fixed,
        );
        let mut mem = vec![0u8; 1024];
        p.call("args_sizes_get", &[0, 4], &mut mem);
        assert_eq!((u32_from(&mem, 0), u32_from(&mem, 4)), (2, 6));
        p.call("args_get", &[16, 64], &mut mem);
        assert_eq!(&mem[64..70], b"cat\0a\0");
        put(&mut mem, 200, b"hi");
        put(&mut mem, 300, &200u32.to_le_bytes());
        put(&mut mem, 304, &2u32.to_le_bytes());
        p.call("fd_write", &[2, 300, 1, 400], &mut mem);
        assert_eq!(p.stderr, b"hi");
        put(&mut mem, 304, &8u32.to_le_bytes());
        p.call("fd_read", &[0, 300, 1, 400], &mut mem);
        assert_eq!(u32_from(&mem, 400), 2);
        assert_eq!(p.call("proc_exit", &[3], &mut mem), Outcome::Exit(3));
        assert_eq!(
            p.call("sock_accept", &[], &mut mem),
            Outcome::Return(errno::NOSYS)
        );
    }

    #[test]
    fn paths_cannot_escape_and_readdir_lists() {
        let mut vfs = Vfs::default();
        vfs.put("src/a.rhai", b"1", 1).unwrap();
        vfs.put("src/b.rhai", b"22", 1).unwrap();
        let mut p = Process::new(vfs, vec![], vec![], vec![], Fixed);
        let mut mem = vec![0u8; 4096];
        put(&mut mem, 100, b"../x");
        let open = [4, 0, 100, 4, 0, -1, -1, 0, 200];
        assert_eq!(
            p.call("path_open", &open, &mut mem),
            Outcome::Return(errno::INVAL)
        );
        put(&mut mem, 100, b"src");
        let open = [4, 0, 100, 3, 2, -1, -1, 0, 200];
        assert_eq!(p.call("path_open", &open, &mut mem), Outcome::Return(0));
        let fd = i64::from(u32_from(&mem, 200));
        assert_eq!(
            p.call("fd_readdir", &[fd, 1000, 512, 0, 300], &mut mem),
            Outcome::Return(0)
        );
        assert_eq!(u32_from(&mem, 300), 2 * 24 + 12);
        assert_eq!(&mem[1024..1030], b"a.rhai");
    }
}
