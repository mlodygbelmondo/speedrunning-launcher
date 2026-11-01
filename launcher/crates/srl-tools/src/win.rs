//! Windows-only helpers: injecting Toolscreen's DLLs into the game's JVM and finding a Java
//! window by its title.
//!
//! `inject` follows EasyInjectBundled (MIT, jojoe77777, commit f59501e),
//! `src/main/java/com/easyinject/DllInjector.java` (`injectDllToProcessWithResult`): write the
//! DLL's UTF-16 path into the target, start a remote thread at `kernel32!LoadLibraryW` with it,
//! wait for the thread, free the memory. See THIRD_PARTY_NOTICES.md. Changes: Rust over
//! windows-sys instead of Java over JNA, the reduced access mask, and the thread's exit code is
//! checked (a zero module handle means `LoadLibraryW` failed in the game).
use std::{ffi::c_void, os::windows::ffi::OsStrExt as _, path::Path};

use anyhow::{Result, bail};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, HANDLE, HWND, LPARAM, MAX_PATH, WAIT_OBJECT_0},
    System::{
        Diagnostics::Debug::WriteProcessMemory,
        LibraryLoader::{GetModuleHandleW, GetProcAddress},
        Memory::{
            MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE, VirtualAllocEx, VirtualFreeEx,
        },
        Threading::{
            CreateRemoteThread, GetExitCodeThread, OpenProcess, PROCESS_CREATE_THREAD,
            PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE, QueryFullProcessImageNameW,
            WaitForSingleObject,
        },
    },
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    },
};

/// How long the remote `LoadLibraryW` may take (EasyInject waits 10 s too).
const LOAD_TIMEOUT_MS: u32 = 10_000;

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenProcess/CreateRemoteThread and is closed once.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Loads `dll` into process `pid` (blocking, up to 10 s).
pub fn inject(pid: u32, dll: &Path) -> Result<()> {
    let wide: Vec<u16> = dll.as_os_str().encode_wide().chain(Some(0)).collect();
    let bytes = wide.len() * 2;
    // SAFETY: Win32 calls with checked results; every handle is wrapped and closed. The remote
    // memory is freed on every path after it was allocated, except when the remote thread may
    // still be reading it (see below).
    unsafe {
        let process = OpenProcess(
            PROCESS_CREATE_THREAD
                | PROCESS_QUERY_INFORMATION
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ,
            0,
            pid,
        );
        if process.is_null() {
            bail!("OpenProcess({pid}) failed: error {}", GetLastError());
        }
        let process = Handle(process);
        let remote = VirtualAllocEx(
            process.0,
            std::ptr::null(),
            bytes,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        if remote.is_null() {
            bail!("VirtualAllocEx failed: error {}", GetLastError());
        }
        // Set when the remote thread may still be inside LoadLibraryW, reading the path.
        let mut still_loading = false;
        let result = (|| -> Result<()> {
            let mut written = 0usize;
            if WriteProcessMemory(
                process.0,
                remote,
                wide.as_ptr() as *const c_void,
                bytes,
                &mut written,
            ) == 0
                || written != bytes
            {
                bail!("WriteProcessMemory failed: error {}", GetLastError());
            }
            let kernel32: Vec<u16> = "kernel32.dll\0".encode_utf16().collect();
            let module = GetModuleHandleW(kernel32.as_ptr());
            if module.is_null() {
                bail!(
                    "GetModuleHandleW(kernel32.dll) failed: error {}",
                    GetLastError()
                );
            }
            // kernel32 sits at the same address in every process of this boot, so our
            // LoadLibraryW is the game's.
            let Some(load_library) = GetProcAddress(module, c"LoadLibraryW".as_ptr().cast()) else {
                bail!(
                    "GetProcAddress(LoadLibraryW) failed: error {}",
                    GetLastError()
                );
            };
            let start: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(load_library);
            let mut thread_id = 0u32;
            let thread = CreateRemoteThread(
                process.0,
                std::ptr::null(),
                0,
                Some(start),
                remote,
                0,
                &mut thread_id,
            );
            if thread.is_null() {
                bail!("CreateRemoteThread failed: error {}", GetLastError());
            }
            let thread = Handle(thread);
            if WaitForSingleObject(thread.0, LOAD_TIMEOUT_MS) != WAIT_OBJECT_0 {
                // An antivirus scanning the DLL can hold LoadLibraryW up. Freeing the path
                // now would let the game's thread read freed memory, so it is left allocated
                // (a few hundred bytes, until the game exits).
                still_loading = true;
                bail!(
                    "LoadLibraryW in the game did not finish within 10 s (its path buffer is left \
                     allocated in the game, since the load may still be running)"
                );
            }
            let mut code = 0u32;
            GetExitCodeThread(thread.0, &mut code);
            // The low 32 bits of the module handle; zero means LoadLibraryW failed.
            if code == 0 {
                bail!("the game could not load {}", dll.display());
            }
            Ok(())
        })();
        if !still_loading {
            VirtualFreeEx(process.0, remote, 0, MEM_RELEASE);
        }
        result
    }
}

/// Pids of `java.exe`/`javaw.exe` processes with a visible window whose title starts with
/// `prefix`.
pub fn java_windows_titled(prefix: &str) -> Vec<u32> {
    struct Search {
        prefix: Vec<u16>,
        pids: Vec<u32>,
    }
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> i32 {
        // SAFETY: `data` is the &mut Search passed to EnumWindows below, alive for the call.
        let search = unsafe { &mut *(data as *mut Search) };
        let mut title = [0u16; 256];
        // SAFETY: plain Win32 calls on a window handle EnumWindows gave us.
        unsafe {
            if IsWindowVisible(hwnd) == 0 {
                return 1;
            }
            let n = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
            if n <= 0 || !title[..n as usize].starts_with(&search.prefix) {
                return 1;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid != 0 && is_java(pid) && !search.pids.contains(&pid) {
                search.pids.push(pid);
            }
        }
        1
    }
    let mut search = Search {
        prefix: prefix.encode_utf16().collect(),
        pids: Vec::new(),
    };
    // SAFETY: the callback only touches `search`, which outlives the call.
    unsafe {
        EnumWindows(Some(each), &mut search as *mut Search as LPARAM);
    }
    search.pids
}

/// Whether process `pid` has a visible top-level window with a title (EasyInject's
/// `getVisibleTopLevelWindowTitle`): the game has opened its window.
pub fn has_titled_window(pid: u32) -> bool {
    struct Search {
        pid: u32,
        found: bool,
    }
    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> i32 {
        // SAFETY: `data` is the &mut Search passed to EnumWindows below, alive for the call.
        let search = unsafe { &mut *(data as *mut Search) };
        let mut title = [0u16; 8];
        // SAFETY: plain Win32 calls on a window handle EnumWindows gave us.
        unsafe {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid != search.pid || IsWindowVisible(hwnd) == 0 {
                return 1;
            }
            if GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32) > 0 {
                search.found = true;
                return 0;
            }
        }
        1
    }
    let mut search = Search { pid, found: false };
    // SAFETY: the callback only touches `search`, which outlives the call.
    unsafe {
        EnumWindows(Some(each), &mut search as *mut Search as LPARAM);
    }
    search.found
}

fn is_java(pid: u32) -> bool {
    // SAFETY: Win32 calls with checked results; the handle is closed by `Handle`.
    unsafe {
        let p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if p.is_null() {
            return false;
        }
        let p = Handle(p);
        let mut buf = [0u16; MAX_PATH as usize];
        let mut len = buf.len() as u32;
        if QueryFullProcessImageNameW(p.0, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) == 0 {
            return false;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]).to_ascii_lowercase();
        path.ends_with("\\java.exe") || path.ends_with("\\javaw.exe")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opt-in (needs a test DLL): `SRL_TEST_DLL=<dll>` that writes `C:\srl-inject-marker.txt`
    /// when it loads. Starts notepad, injects the DLL, checks the marker names notepad's pid.
    #[test]
    #[ignore]
    fn injects_into_a_child() {
        let dll = std::env::var("SRL_TEST_DLL").expect("set SRL_TEST_DLL");
        let marker = Path::new(r"C:\srl-inject-marker.txt");
        let _ = std::fs::remove_file(marker);
        let mut child = std::process::Command::new(r"C:\windows\system32\notepad.exe")
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let result = inject(child.id(), Path::new(&dll));
        let text = std::fs::read_to_string(marker).unwrap_or_default();
        let _ = child.kill();
        let _ = child.wait();
        result.unwrap();
        assert_eq!(text.trim(), format!("loaded into pid {}", child.id()));
        // A DLL that doesn't exist makes LoadLibraryW fail in the target, and we say so.
        let mut child = std::process::Command::new(r"C:\windows\system32\notepad.exe")
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let err = inject(child.id(), Path::new(r"C:\no\such.dll")).unwrap_err();
        let _ = child.kill();
        let _ = child.wait();
        assert!(err.to_string().contains("could not load"), "{err}");
    }

    #[test]
    fn no_java_window_named_like_this() {
        assert!(java_windows_titled("srl-tools test: no such window").is_empty());
    }
}
