//! Windows: a generic credential in Credential Manager, and DPAPI for the fallback key file.
//! The Credential Manager part is adapted from Pandora Launcher's `crates/auth/src/secret.rs`
//! (MIT), ported from the `windows` crate to `windows-sys`.
use std::{io, ptr};

use windows_sys::Win32::{
    Foundation::{ERROR_NOT_FOUND, GetLastError, LocalFree},
    Security::{
        Credentials::{
            CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree,
            CredReadW, CredWriteW,
        },
        Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    },
};
use zeroize::{Zeroize, Zeroizing};

use super::SERVICE;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn target(name: &str) -> Vec<u16> {
    wide(&format!("{SERVICE}/{name}"))
}

fn last_error(what: &str) -> String {
    format!("{what} failed: {}", io::Error::last_os_error())
}

/// Credential Manager calls are blocking Win32 I/O: keep them off the async worker threads.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

pub async fn get(name: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
    let target = target(name);
    blocking(move || {
        let mut cred: *mut CREDENTIALW = ptr::null_mut();
        // SAFETY: `target` is NUL-terminated and outlives the call; `cred` is freed with CredFree.
        unsafe {
            if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut cred) == 0 {
                if GetLastError() == ERROR_NOT_FOUND {
                    return Ok(None);
                }
                return Err(last_error("CredReadW"));
            }
            let c = &*cred;
            let size = c.CredentialBlobSize as usize;
            let blob = if size == 0 || c.CredentialBlob.is_null() {
                Vec::new()
            } else {
                let slice = std::slice::from_raw_parts_mut(c.CredentialBlob, size);
                let copied = slice.to_vec();
                slice.zeroize();
                copied
            };
            CredFree(cred as *const _);
            Ok(Some(Zeroizing::new(blob)))
        }
    })
    .await
}

pub async fn set(name: &str, value: &[u8]) -> Result<(), String> {
    let mut target = target(name);
    let mut user = wide(SERVICE);
    let mut blob = Zeroizing::new(value.to_vec());
    let blob_len =
        u32::try_from(blob.len()).map_err(|_| "credential blob too large".to_string())?;
    blocking(move || {
        // SAFETY: every pointer in `cred` points into a buffer that lives until after CredWriteW.
        unsafe {
            let mut cred: CREDENTIALW = std::mem::zeroed();
            cred.Type = CRED_TYPE_GENERIC;
            cred.TargetName = target.as_mut_ptr();
            cred.UserName = user.as_mut_ptr();
            cred.CredentialBlobSize = blob_len;
            cred.CredentialBlob = blob.as_mut_ptr();
            cred.Persist = CRED_PERSIST_LOCAL_MACHINE;
            if CredWriteW(&cred, 0) == 0 {
                return Err(last_error("CredWriteW"));
            }
        }
        Ok(())
    })
    .await
}

pub async fn delete(name: &str) -> Result<(), String> {
    let target = target(name);
    blocking(move || {
        // SAFETY: `target` is NUL-terminated and outlives the call.
        unsafe {
            if CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) == 0
                && GetLastError() != ERROR_NOT_FOUND
            {
                return Err(last_error("CredDeleteW"));
            }
        }
        Ok(())
    })
    .await
}

/// DPAPI, current-user scope: only this Windows user on this machine can unprotect it.
pub fn dpapi_protect(data: &[u8]) -> io::Result<Vec<u8>> {
    dpapi(data, true)
}

pub fn dpapi_unprotect(data: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
    dpapi(data, false).map(Zeroizing::new)
}

fn dpapi(data: &[u8], protect: bool) -> io::Result<Vec<u8>> {
    let cb_data = u32::try_from(data.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "DPAPI input too large"))?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: cb_data,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    // SAFETY: `input` borrows `data` for the call; DPAPI allocates `output` with LocalAlloc and
    // we zeroize and free it with LocalFree after copying it out.
    unsafe {
        let ok = if protect {
            CryptProtectData(
                &input,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                ptr::null_mut(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let size = output.cbData as usize;
        let out = if size == 0 || output.pbData.is_null() {
            Vec::new()
        } else {
            let slice = std::slice::from_raw_parts_mut(output.pbData, size);
            let copied = slice.to_vec();
            slice.zeroize();
            copied
        };
        if !output.pbData.is_null() {
            LocalFree(output.pbData as _);
        }
        Ok(out)
    }
}
