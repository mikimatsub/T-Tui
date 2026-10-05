//! Windows DPAPI protects session fields for the current OS user.
#[cfg(windows)]
fn transform(bytes: &[u8], protect: bool) -> std::io::Result<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes
            .len()
            .try_into()
            .map_err(|_| std::io::Error::other("Credentials are too large"))?,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: DPAPI reads input for the duration of this call. Output is an
    // OS allocation, copied before LocalFree. UI and machine-wide access are disabled.
    let success = unsafe {
        if protect {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: successful DPAPI calls return cbData initialized bytes at pbData.
    let copied =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    // SAFETY: this buffer was allocated by DPAPI and is released exactly once.
    unsafe {
        LocalFree(output.pbData.cast());
    }
    Ok(copied)
}

pub fn encode(mut value: serde_json::Value, mock: bool) -> std::io::Result<Vec<u8>> {
    #[cfg(windows)]
    if !mock {
        let object = value
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("Invalid config"))?;
        let mut credentials = serde_json::Map::new();
        for key in ["auth_token", "device_id", "refresh_token"] {
            if let Some(value) = object.remove(key) {
                credentials.insert(key.into(), value);
            }
        }
        if credentials.values().any(|value| !value.is_null()) {
            let bytes = serde_json::to_vec(&credentials).map_err(std::io::Error::other)?;
            object.insert(
                "windows_credentials".into(),
                transform(&bytes, true)?.into(),
            );
        }
    }
    #[cfg(not(windows))]
    let _ = (&mut value, mock);
    serde_json::to_vec_pretty(&value).map_err(std::io::Error::other)
}

pub fn decode(bytes: &[u8]) -> std::io::Result<serde_json::Value> {
    let mut value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(std::io::Error::other)?;
    if let Some(encrypted) = value
        .as_object_mut()
        .and_then(|map| map.remove("windows_credentials"))
    {
        #[cfg(windows)]
        {
            let encrypted: Vec<u8> =
                serde_json::from_value(encrypted).map_err(std::io::Error::other)?;
            let decrypted = transform(&encrypted, false).map_err(|_| std::io::Error::other(
                "Cannot unlock the Windows session. Open this config as its original Windows user; the file has been preserved."))?;
            let credentials: serde_json::Map<String, serde_json::Value> =
                serde_json::from_slice(&decrypted).map_err(std::io::Error::other)?;
            let object = value
                .as_object_mut()
                .ok_or_else(|| std::io::Error::other("Invalid config"))?;
            for key in ["auth_token", "device_id", "refresh_token"] {
                object.insert(
                    key.into(),
                    credentials.get(key).cloned().unwrap_or_default(),
                );
            }
        }
        #[cfg(not(windows))]
        {
            let _ = encrypted;
            return Err(std::io::Error::other(
                "This config contains a Windows-protected session. Use a separate config on Linux.",
            ));
        }
    }
    Ok(value)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn credentials_are_protected_and_tampering_is_rejected() {
        let original = serde_json::json!({"auth_token":"private-auth", "refresh_token":"private-refresh", "device_id":"device", "theme":"Dark"});
        let encoded = encode(original.clone(), false).unwrap();
        let text = String::from_utf8_lossy(&encoded);
        assert!(!text.contains("private-auth"));
        assert!(!text.contains("private-refresh"));
        assert_eq!(decode(&encoded).unwrap(), original);
        let mut stored: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        stored["windows_credentials"][0] = 255.into();
        assert!(decode(&serde_json::to_vec(&stored).unwrap()).is_err());
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn legacy_plaintext_and_demo_configs_remain_readable() {
        let original = serde_json::json!({
            "auth_token": "synthetic-auth", "refresh_token": "synthetic-refresh",
            "device_id": "synthetic-device", "drafts": {"chat": "A quote: \"日本語\""}
        });
        let legacy = serde_json::to_vec(&original).unwrap();
        assert_eq!(decode(&legacy).unwrap()["auth_token"], "synthetic-auth");
        let demo = encode(original, true).unwrap();
        let stored: serde_json::Value = serde_json::from_slice(&demo).unwrap();
        assert!(stored.get("windows_credentials").is_none());
        assert_eq!(stored["auth_token"], "synthetic-auth");
        let decoded = decode(&demo).unwrap();
        assert_eq!(decoded["refresh_token"], "synthetic-refresh");
        assert_eq!(decoded["drafts"]["chat"], "A quote: \"日本語\"");
        assert!(decoded.get("windows_credentials").is_none());
    }

    #[test]
    fn signed_out_config_does_not_contain_a_protected_session() {
        let encoded = encode(
            serde_json::json!({
                "auth_token": null, "refresh_token": null, "device_id": null,
                "theme": "Light"
            }),
            false,
        )
        .unwrap();
        let stored: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert!(stored.get("windows_credentials").is_none());
        let decoded = decode(&encoded).unwrap();
        assert!(decoded.get("windows_credentials").is_none());
        assert!(decoded["auth_token"].is_null());
        assert!(decoded["refresh_token"].is_null());
        assert!(decoded["device_id"].is_null());
        assert_eq!(decoded["theme"], "Light");
    }
}
