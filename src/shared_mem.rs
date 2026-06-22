// Named shared memory for desktop snapshots, passed from the --daemon
// process to /s screensaver instances.
//
// Layout (little-endian, packed):
//   u32  magic       = 0xB1AC_C001
//   u32  version     = 1
//   u32  width       physical pixels
//   u32  height      physical pixels
//   u64  written_ns  monotonic write timestamp (so the reader can tell
//                    "stale" from "fresh" at a glance; not strictly
//                    needed -- a hash bump would also work)
//   [u8] bgra        width * height * 4 bytes
//
// The mapping is sized to the largest snapshot we expect to see (8K),
// so growing displays don't need to recreate it.
//
// The Local\\ prefix scopes the name to the current logon session, which
// is what we want: a logged-out user shouldn't be able to read the
// desktop of whoever logs in next.

#![cfg(windows)]

#![allow(unused_imports)]

use anyhow::{anyhow, Result};
use std::ffi::c_void;
use std::ptr::NonNull;

pub const MAGIC: u32 = 0xB1AC_C001;
pub const VERSION: u32 = 1;
pub const HEADER_SIZE: usize = 4 + 4 + 4 + 4 + 8;
// Enough for an 8K (7680x4320) BGRA frame plus header. Reservation does
// not commit physical RAM on Windows; the OS pages in only what's touched.
pub const MAPPING_SIZE: usize = HEADER_SIZE + 7680 * 4320 * 4;

pub const MAPPING_NAME: &str = "Local\\BlackholeScreensaver_DesktopSnapshot_v1";

pub struct SharedMem {
    handle: windows::Win32::Foundation::HANDLE,
    ptr: NonNull<u8>,
    len: usize,
}

unsafe impl Send for SharedMem {}
unsafe impl Sync for SharedMem {}

impl SharedMem {
    /// Create-or-open the mapping (daemon side).
    pub fn create() -> Result<Self> {
        use windows::core::HSTRING;
        use windows::Win32::System::Memory::{CreateFileMappingW, PAGE_READWRITE};

        let name = HSTRING::from(MAPPING_NAME);
        let size_hi = (MAPPING_SIZE >> 32) as u32;
        let size_lo = (MAPPING_SIZE & 0xFFFF_FFFF) as u32;
        unsafe {
            let handle = CreateFileMappingW(
                windows::Win32::Foundation::INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                size_hi,
                size_lo,
                &name,
            )?;
            if handle.is_invalid() {
                return Err(anyhow!("CreateFileMappingW returned invalid handle"));
            }
            map(handle)
        }
    }

    /// Open an existing mapping read-only (screensaver side).
    pub fn open_existing() -> Result<Self> {
        use windows::core::HSTRING;
        use windows::Win32::System::Memory::{OpenFileMappingW, FILE_MAP_READ};
        let name = HSTRING::from(MAPPING_NAME);
        unsafe {
            let handle = OpenFileMappingW(FILE_MAP_READ.0, false, &name)?;
            if handle.is_invalid() {
                return Err(anyhow!("OpenFileMappingW returned invalid handle"));
            }
            map_read(handle)
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for SharedMem {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Memory::{UnmapViewOfFile, MEMORY_MAPPED_VIEW_ADDRESS};
        unsafe {
            let view = MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.ptr.as_ptr() as *mut c_void,
            };
            let _ = UnmapViewOfFile(view);
            let _ = CloseHandle(self.handle);
        }
    }
}

unsafe fn map(handle: windows::Win32::Foundation::HANDLE) -> Result<SharedMem> {
    use windows::Win32::System::Memory::{MapViewOfFile, FILE_MAP_ALL_ACCESS};
    let view = MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, MAPPING_SIZE);
    if view.Value.is_null() {
        return Err(anyhow!("MapViewOfFile failed"));
    }
    Ok(SharedMem {
        handle,
        ptr: NonNull::new(view.Value as *mut u8).unwrap(),
        len: MAPPING_SIZE,
    })
}

unsafe fn map_read(handle: windows::Win32::Foundation::HANDLE) -> Result<SharedMem> {
    use windows::Win32::System::Memory::{MapViewOfFile, FILE_MAP_READ};
    let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, MAPPING_SIZE);
    if view.Value.is_null() {
        return Err(anyhow!("MapViewOfFile (read) failed"));
    }
    Ok(SharedMem {
        handle,
        ptr: NonNull::new(view.Value as *mut u8).unwrap(),
        len: MAPPING_SIZE,
    })
}

// ---- header helpers ------------------------------------------------------

pub fn write_header(buf: &mut [u8], width: u32, height: u32, written_ns: u64) {
    buf[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    buf[4..8].copy_from_slice(&VERSION.to_le_bytes());
    buf[8..12].copy_from_slice(&width.to_le_bytes());
    buf[12..16].copy_from_slice(&height.to_le_bytes());
    buf[16..24].copy_from_slice(&written_ns.to_le_bytes());
}

pub struct Snapshot {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

pub fn read_snapshot(buf: &[u8]) -> Option<Snapshot> {
    if buf.len() < HEADER_SIZE {
        return None;
    }
    let magic = u32::from_le_bytes(buf[0..4].try_into().ok()?);
    if magic != MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(buf[4..8].try_into().ok()?);
    if version != VERSION {
        return None;
    }
    let width = u32::from_le_bytes(buf[8..12].try_into().ok()?);
    let height = u32::from_le_bytes(buf[12..16].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    let pixels = (width as usize).checked_mul(height as usize)?;
    let bytes = pixels.checked_mul(4)?;
    if HEADER_SIZE + bytes > buf.len() {
        return None;
    }
    Some(Snapshot {
        width,
        height,
        bgra: buf[HEADER_SIZE..HEADER_SIZE + bytes].to_vec(),
    })
}
