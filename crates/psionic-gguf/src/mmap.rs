use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

use crate::format::{GgufMeta, TensorInfo};

pub struct TensorView {
    pub info: TensorInfo,
    pub data: *const u8,
    pub len: usize,
}

// Safety: views borrow the mmap held in MappedWeights.
unsafe impl Send for TensorView {}
unsafe impl Sync for TensorView {}

pub struct MappedWeights {
    pub mmap: Mmap,
    pub file_size: u64,
    pub data_offset: u64,
    pub tensors: HashMap<String, TensorView>,
}

impl MappedWeights {
    /// Pages of this mapping currently resident. Shared Metal wrap does not
    /// add a second copy; this is the mmap itself.
    pub fn resident_bytes(&self) -> u64 {
        resident_of(self.mmap.as_ptr(), self.mmap.len())
    }
}

fn resident_of(ptr: *const u8, len: usize) -> u64 {
    if len == 0 {
        return 0;
    }
    #[cfg(unix)]
    {
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
        if page == 0 {
            return 0;
        }
        let start = (ptr as usize) & !(page - 1);
        let end = (ptr as usize + len + page - 1) & !(page - 1);
        let span = end.saturating_sub(start);
        let n_pages = span / page;
        let mut vec = vec![0u8; n_pages];
        let rc =
            unsafe { libc::mincore(start as *mut libc::c_void, span, vec.as_mut_ptr() as *mut _) };
        if rc != 0 {
            return 0;
        }
        let pages = vec.iter().filter(|byte| *byte & 1 != 0).count();
        (pages * page) as u64
    }
    #[cfg(not(unix))]
    {
        let _ = ptr;
        len as u64
    }
}

pub fn map_file(path: &Path, meta: &GgufMeta) -> Result<MappedWeights, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    // Safety: the file is not mutated while mapped; GGUF loads are read-only.
    let mmap = unsafe { Mmap::map(&file).map_err(|e| e.to_string())? };
    if mmap.len() as u64 != meta.file_size && meta.file_size != 0 {
        // Length can differ if the file grew; still bind within this map.
    }
    let mut tensors = HashMap::new();
    let base = mmap.as_ptr();
    let map_len = mmap.len();
    for info in &meta.tensors {
        let start = meta
            .data_offset
            .checked_add(info.offset)
            .ok_or_else(|| String::from("tensor offset overflow"))?;
        let len = info.nbytes().unwrap_or(0) as usize;
        if start as usize + len > map_len {
            return Err(format!("tensor {} extends past the mapped file", info.name));
        }
        let data = unsafe { base.add(start as usize) };
        tensors.insert(
            info.name.clone(),
            TensorView {
                info: info.clone(),
                data,
                len,
            },
        );
    }
    Ok(MappedWeights {
        file_size: map_len as u64,
        data_offset: meta.data_offset,
        mmap,
        tensors,
    })
}
