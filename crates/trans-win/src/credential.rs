//! Windows 凭据管理器。API Key 只存这里，不写进设置文件，也不出现在日志里。

use trans_application::ports::{SecretError, SecretStore};
use windows::core::{HSTRING, PWSTR};
use windows::Win32::Foundation::ERROR_NOT_FOUND;
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

#[derive(Debug, Clone)]
pub struct CredentialStore {
    prefix: String,
}

impl CredentialStore {
    /// 凭据名为 `<app_id>/<key>`，在"控制面板 → 凭据管理器 → Windows 凭据"里可见、可手动删除。
    pub fn new(app_id: &str) -> Self {
        Self { prefix: format!("{app_id}/") }
    }

    fn target(&self, key: &str) -> HSTRING {
        HSTRING::from(format!("{}{key}", self.prefix))
    }
}

impl SecretStore for CredentialStore {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        let target = self.target(key);
        let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `target` 是以 NUL 结尾的宽字符串；成功时系统分配 `credential`，下面用 CredFree 释放。
        let result = unsafe { CredReadW(&target, CRED_TYPE_GENERIC, None, &mut credential) };
        match result {
            Ok(()) => {
                // SAFETY: CredReadW 成功，`credential` 在 CredFree 之前有效；长度为 0 时 blob 可能是空指针。
                let bytes = unsafe {
                    let c = &*credential;
                    let bytes = if c.CredentialBlobSize == 0 || c.CredentialBlob.is_null() {
                        Vec::new()
                    } else {
                        std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec()
                    };
                    CredFree(credential as *const _);
                    bytes
                };
                String::from_utf8(bytes).map(Some).map_err(|_| SecretError("凭据内容不是 UTF-8 文本".into()))
            }
            Err(error) if error.code() == ERROR_NOT_FOUND.to_hresult() => Ok(None),
            Err(error) => Err(SecretError(error.to_string())),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        let target = self.target(key);
        let mut blob = value.as_bytes().to_vec();
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_ptr().cast_mut()),
            CredentialBlobSize: u32::try_from(blob.len()).map_err(|_| SecretError("凭据内容过长".into()))?,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        // SAFETY: `credential` 里的指针借用 `target` 和 `blob`，二者都活到调用结束之后。
        unsafe { CredWriteW(&credential, 0) }.map_err(|e| SecretError(e.to_string()))
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        let target = self.target(key);
        // SAFETY: `target` 是以 NUL 结尾的宽字符串。
        match unsafe { CredDeleteW(&target, CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if error.code() == ERROR_NOT_FOUND.to_hresult() => Ok(()),
            Err(error) => Err(SecretError(error.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 会真实写入当前用户的凭据管理器（写完即删），默认不跑：`cargo test -p trans-win -- --ignored`。
    #[test]
    #[ignore = "touches the real Windows Credential Manager"]
    fn round_trips_a_secret_through_the_real_credential_manager() {
        let store = CredentialStore::new("trans-selftest");
        let key = "roundtrip";
        store.delete(key).unwrap();
        assert_eq!(store.get(key).unwrap(), None);
        store.set(key, "sk-测试-value").unwrap();
        assert_eq!(store.get(key).unwrap().as_deref(), Some("sk-测试-value"));
        store.delete(key).unwrap();
        assert_eq!(store.get(key).unwrap(), None);
    }
}
