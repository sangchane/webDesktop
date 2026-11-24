//! Windows OS 호출 — 키 보호(DPAPI)·제한된 파일 쓰기 (04 「코드 구조」 platform/windows).
use std::{fs::File, io::Write, os::windows::io::FromRawHandle, path::Path, ptr};
use windows_sys::Win32::{
    Foundation::{LocalFree, INVALID_HANDLE_VALUE},
    Security::{
        Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1},
        Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_LOCAL_MACHINE,
            CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
        PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
    },
    Storage::FileSystem::{
        CreateDirectoryW, CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_WRITE,
    },
};

/// 자료 폴더·키 파일 ACL — 상속 끊고 SYSTEM·Administrators·주인만, 폴더 안 파일도 물려받음(OICI).
/// 기계 범위 DPAPI 라 첫 작업 2 의 SYSTEM 서비스도 같은 키를 읽는다(재등록 없음 — 목표 1).
const PRIVATE_SDDL: &str = "D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;OW)";

/// PRIVATE_SDDL 보안 속성을 만들어 f 에 빌려주고 돌려받는다.
fn with_private<R>(f: impl FnOnce(&SECURITY_ATTRIBUTES) -> R) -> std::io::Result<R> {
    let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: 문자열은 널 끝, sd 는 OS 가 할당하고 f 가 끝난 뒤 LocalFree.
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide(PRIVATE_SDDL).as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            ptr::null_mut(),
        ) == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let sa = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        let r = f(&sa);
        LocalFree(sd as _);
        Ok(r)
    }
}

/// 자료 폴더를 보호 ACL 로 만든다. 이미 있으면 그대로 둔다.
/// shortcut: 남이 미리 만든 폴더(느슨한 ACL)를 거부하지 않음 — 설치 프로그램이 관리자로 폴더를 만드는 첫 작업 2 에서 주인 확인을 더한다.
pub fn create_private_dir(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    // SAFETY: 경로는 널 끝, sa 는 호출 동안 살아 있다.
    // 오류 번호는 LocalFree 전에 읽는다.
    with_private(|sa| unsafe {
        if CreateDirectoryW(wide(&path.to_string_lossy()).as_ptr(), sa) == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    })?
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn blob(b: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: b.len() as u32,
        pbData: b.as_ptr() as *mut u8,
    }
}

/// DPAPI 결과를 복사하고 OS 버퍼를 돌려준다.
unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
    LocalFree(out.pbData as _);
    v
}

pub fn protect(plain: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    // SAFETY: 입력 blob 은 plain 을 빌려 이 호출 동안만 쓰고, out 은 OS 가 채운다.
    unsafe {
        if CryptProtectData(
            &blob(plain),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        ) == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(take(out))
    }
}

pub fn unprotect(sealed: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    // SAFETY: protect 와 같다.
    unsafe {
        if CryptUnprotectData(
            &blob(sealed),
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        ) == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(take(out))
    }
}

/// 앞 파일을 지우고 PRIVATE_SDDL 로 새로 만든다 — 남이 미리 만든 느슨한 파일을 이어 쓰지 않는다.
pub fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    // SAFETY: 경로는 널 끝, sa 는 호출 동안 살아 있다.
    let h = with_private(|sa| unsafe {
        let h = CreateFileW(
            wide(&path.to_string_lossy()).as_ptr(),
            FILE_GENERIC_WRITE,
            0,
            sa,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        );
        // 오류 번호는 LocalFree 전에 읽는다.
        if h == INVALID_HANDLE_VALUE {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(h)
        }
    })??;
    // SAFETY: 방금 연 유효한 핸들 — File 이 소유하고 닫는다.
    unsafe {
        let mut f = File::from_raw_handle(h as _);
        f.write_all(data)?;
        f.sync_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protect_round_trip() {
        let sealed = protect(b"seed-32-bytes").unwrap();
        assert_ne!(sealed, b"seed-32-bytes");
        assert_eq!(unprotect(&sealed).unwrap(), b"seed-32-bytes");
    }

    #[test]
    fn private_dir_twice() {
        let d = std::env::temp_dir().join(format!("wd-dir-{}", std::process::id()));
        create_private_dir(&d).unwrap();
        create_private_dir(&d).unwrap();
        write_private(&d.join("k"), b"x").unwrap();
        assert_eq!(std::fs::read(d.join("k")).unwrap(), b"x");
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn private_file_overwrites() {
        let p = std::env::temp_dir().join(format!("wd-key-{}", std::process::id()));
        write_private(&p, b"one").unwrap();
        write_private(&p, b"two").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"two");
        std::fs::remove_file(p).unwrap();
    }
}
