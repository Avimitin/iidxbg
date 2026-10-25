//! Session material is never serialized into TOML or sent to media CDNs.
use super::{Api, err};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Runtime-only identity. A configuration and its stable user ID own one login.
/// The canonical configuration path is supplied by the config loader; display
/// names and card bindings may change without changing the account binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountKey([u8; 32]);
impl AccountKey {
    pub(crate) fn new(config_path: &Path, user: &str) -> Self {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(config_path.as_os_str().as_encoded_bytes());
        hash.update([0]);
        hash.update(user.as_bytes());
        Self(hash.finalize().into())
    }
    pub(super) fn cache_key(&self) -> String {
        hex::encode(self.0)
    }
    fn path_in(&self, root: &Path) -> PathBuf {
        root.join("bilibili-users")
            .join(format!("{}.dat", self.cache_key()))
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Session {
    cookies: BTreeMap<String, String>,
}
impl Session {
    pub(super) fn authenticated(&self) -> bool {
        self.cookies.contains_key("SESSDATA")
    }
    pub(super) fn cookie(&self) -> String {
        self.cookies
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
    fn accept(&mut self, header: &str) {
        let Some((key, value)) = header.split(';').next().and_then(|s| s.split_once('=')) else {
            return;
        };
        if [
            "SESSDATA",
            "bili_jct",
            "DedeUserID",
            "DedeUserID__ckMd5",
            "sid",
            "buvid3",
            "buvid4",
        ]
        .contains(&key)
            && value.len() <= 8192
            && value.bytes().all(|b| b.is_ascii_graphic() && b != b';')
        {
            self.cookies.insert(key.into(), value.into());
        }
    }
}
pub struct Qr {
    pub url: String,
    key: String,
}
pub enum Poll {
    Waiting,
    Scanned,
    Expired,
    Ready(Session),
}
impl Api {
    pub fn qr(&self) -> Result<Qr> {
        let data = Self::data(self.response(
            "passport.bilibili.com",
            "/x/passport-login/web/qrcode/generate",
            &[],
        )?)?;
        let url = data["url"]
            .as_str()
            .ok_or_else(|| err("登录二维码缺少地址"))?;
        let parsed = reqwest::Url::parse(url).map_err(|_| err("登录二维码地址无效"))?;
        if parsed.scheme() != "https"
            || !matches!(
                parsed.host_str(),
                Some("passport.bilibili.com" | "account.bilibili.com")
            )
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port_or_known_default() != Some(443)
        {
            return Err(err("登录二维码地址不受支持"));
        }
        Ok(Qr {
            url: url.into(),
            key: data["qrcode_key"]
                .as_str()
                .ok_or_else(|| err("登录二维码缺少密钥"))?
                .into(),
        })
    }
    pub fn poll(&self, qr: &Qr) -> Result<Poll> {
        let response = self.response(
            "passport.bilibili.com",
            "/x/passport-login/web/qrcode/poll",
            &[("qrcode_key", qr.key.clone())],
        )?;
        let mut session = Session::default();
        for cookie in response.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Ok(cookie) = cookie.to_str() {
                session.accept(cookie);
            }
        }
        let data = Self::data(response)?;
        match data["code"].as_i64() {
            Some(86101) => Ok(Poll::Waiting),
            Some(86090) => Ok(Poll::Scanned),
            Some(86038) => Ok(Poll::Expired),
            Some(0) if session.cookies.contains_key("SESSDATA") => Ok(Poll::Ready(session)),
            _ => Err(err("登录响应无效，请重新获取二维码")),
        }
    }
}
fn session_root() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA").ok_or_else(|| err("找不到 Windows 用户数据目录"))?,
    )
    .join("iidxbg"))
}
pub fn load(account: &AccountKey) -> Result<Session> {
    load_in(&session_root()?, account)
}
fn load_in(root: &Path, account: &AccountKey) -> Result<Session> {
    let path = account.path_in(root);
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Session::default()),
        Err(e) => return Err(e.into()),
    };
    let encrypted = crate::pixels::read_bounded(file, 64 * 1024)?;
    let bytes = protect(&encrypted, false, &account.0)?;
    let session: Session =
        serde_json::from_slice(&bytes).map_err(|_| err("保存的登录信息损坏，请重新登录"))?;
    // Revalidate before constructing any HTTP headers.
    let mut checked = Session::default();
    for (k, v) in session.cookies {
        checked.accept(&format!("{k}={v}"));
    }
    Ok(checked)
}
/// Store a session already verified by the account worker.
pub fn save(account: &AccountKey, session: &Session) -> Result<()> {
    save_in(&session_root()?, account, session)
}
fn save_in(root: &Path, account: &AccountKey, session: &Session) -> Result<()> {
    let bytes = serde_json::to_vec(&session)?;
    let encrypted = protect(&bytes, true, &account.0)?;
    let path = account.path_in(root);
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir)?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.write_all(&encrypted)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
pub fn logout(account: &AccountKey) -> Result<()> {
    logout_in(&session_root()?, account)
}
fn logout_in(root: &Path, account: &AccountKey) -> Result<()> {
    match fs::remove_file(account.path_in(root)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
#[cfg(windows)]
fn protect(data: &[u8], encrypt: bool, account: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: account.len() as u32,
        pbData: account.as_ptr() as *mut u8,
    };
    // DPAPI defaults to current-user protection, deliberately not LOCAL_MACHINE.
    unsafe {
        let ok = if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(err(
                "Windows 无法解密或保存登录信息；请在游戏所在机器上重新登录",
            ));
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
        LocalFree(output.pbData.cast());
        Ok(bytes)
    }
}
#[cfg(not(windows))]
fn protect(_: &[u8], _: bool, _: &[u8]) -> Result<Vec<u8>> {
    Err(err("登录信息存储需要 Windows DPAPI"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires Bilibili network access; creates an unconfirmed QR session"]
    fn live_qr_creation_and_waiting() {
        let api = Api::new(Session::default()).unwrap();
        let qr = api.qr().unwrap();
        assert!(matches!(api.poll(&qr).unwrap(), Poll::Waiting));
    }
    #[test]
    fn only_expected_cookie_names_and_safe_values_survive() {
        let mut session = Session::default();
        session.accept("SESSDATA=test%2Cvalue; Secure; HttpOnly");
        session.accept("evil=secret");
        session.accept("bili_jct=bad\r\nvalue");
        assert_eq!(session.cookie(), "SESSDATA=test%2Cvalue");
    }
    #[cfg(windows)]
    #[test]
    fn dpapi_roundtrip_and_reject_corrupt_data() {
        let secret = b"test session fixture, not credentials";
        let encrypted = protect(secret, true, b"alice").unwrap();
        assert_ne!(encrypted, secret);
        assert_eq!(protect(&encrypted, false, b"alice").unwrap(), secret);
        assert!(protect(&encrypted, false, b"bob").is_err());
        assert!(protect(b"invalid", false, b"alice").is_err());
    }
    #[cfg(windows)]
    #[test]
    fn sessions_are_isolated_by_user_and_config_and_ignore_legacy_login() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let alice = AccountKey::new(Path::new("cabinet-a.toml"), "alice");
        let bob = AccountKey::new(Path::new("cabinet-a.toml"), "bob");
        let other = AccountKey::new(Path::new("cabinet-b.toml"), "alice");
        let mut a = Session::default();
        a.accept("SESSDATA=alice-fixture");
        let mut b = Session::default();
        b.accept("SESSDATA=bob-fixture");
        fs::write(root.join("bilibili-session.dat"), b"unused legacy session").unwrap();
        assert!(!load_in(root, &alice).unwrap().authenticated());
        save_in(root, &alice, &a).unwrap();
        assert!(!load_in(root, &bob).unwrap().authenticated());
        assert!(!load_in(root, &other).unwrap().authenticated());
        save_in(root, &bob, &b).unwrap();
        assert_eq!(load_in(root, &alice).unwrap().cookie(), a.cookie());
        assert_eq!(load_in(root, &bob).unwrap().cookie(), b.cookie());
        // Even copying ciphertext cannot silently attach Alice's session to Bob.
        fs::copy(alice.path_in(root), other.path_in(root)).unwrap();
        assert!(load_in(root, &other).is_err());
        logout_in(root, &alice).unwrap();
        assert!(!load_in(root, &alice).unwrap().authenticated());
        assert_eq!(load_in(root, &bob).unwrap().cookie(), b.cookie());
        assert!(root.join("bilibili-session.dat").exists());
    }
}
