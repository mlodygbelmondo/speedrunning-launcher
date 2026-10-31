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

pub async fn get(name: &str) -> Result<Option<Vec<u8>>, String> {
    let target = target(name);
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
        let blob =
            std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec();
        CredFree(cred as *const _);
        Ok(Some(blob))
    }
}

pub async fn set(name: &str, value: &[u8]) -> Result<(), String> {
    let mut target = target(name);
    let mut user = wide(SERVICE);
    let mut blob = zeroize::Zeroizing::new(value.to_vec());
    // SAFETY: every pointer in `cred` points into a buffer that lives until after CredWriteW.
    unsafe {
        let mut cred: CREDENTIALW = std::mem::zeroed();
        cred.Type = CRED_TYPE_GENERIC;
        cred.TargetName = target.as_mut_ptr();
        cred.UserName = user.as_mut_ptr();
        cred.CredentialBlobSize = blob.len() as u32;
        cred.CredentialBlob = blob.as_mut_ptr();
        cred.Persist = CRED_PERSIST_LOCAL_MACHINE;
        if CredWriteW(&cred, 0) == 0 {
            return Err(last_error("CredWriteW"));
        }
    }
    Ok(())
}

pub async fn delete(name: &str) -> Result<(), String> {
    let target = target(name);
    // SAFETY: `target` is NUL-terminated and outlives the call.
    unsafe {
        if CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) == 0
            && GetLastError() != ERROR_NOT_FOUND
        {
            return Err(last_error("CredDeleteW"));
        }
    }
    Ok(())
}

/// DPAPI, current-user scope: only this Windows user on this machine can unprotect it.
pub fn dpapi_protect(data: &[u8]) -> io::Result<Vec<u8>> {
    dpapi(data, true)
}

pub fn dpapi_unprotect(data: &[u8]) -> io::Result<Vec<u8>> {
    dpapi(data, false)
}

fn dpapi(data: &[u8], protect: bool) -> io::Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    // SAFETY: `input` borrows `data` for the call; DPAPI allocates `output` with LocalAlloc and
    // we free it with LocalFree after copying it out.
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
        let out = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as _);
        Ok(out)
    }
}
