//! Round 29 · Provider API Key AEAD 加密。
//!
//! ## 威胁模型
//!
//! - **目标**:磁盘冷拷贝(用户备份 SQLite 文件 / 误把 `~/Library/...`
//!   分享出去 / 数据被恶意软件读取)时,API Key 不可还原。
//! - **不目标**:能 root / 能在用户机器上跑代码的攻击者 — 他们能拿到
//!   machine-uid + 我们的进程,本地解密 key 显然恢复;这是任何"无外部
//!   secret server"的本地加密的固有限制(等价于 Chrome / Slack 桌面
//!   端的密钥处理),通过应用沙盒 + 操作系统权限保护。
//!
//! ## 算法选择
//!
//! - **AEAD**:ChaCha20-Poly1305(RFC8439)。纯 Rust 实现无 native deps,
//!   性能 ~600MB/s 远超我们这个用 case 的需求(几十字节 key)。比 AES-GCM
//!   更不依赖硬件指令。
//! - **Key derivation**:`SHA-256(machine_uid || APP_SALT)`。machine_uid
//!   在同机器上稳定,跨机器/重装系统会变(等价于"重装系统后用户重输 key"
//!   — 业界标准做法)。`APP_SALT` 是写死的 32 字节常量,让同机器上其它
//!   应用拿同样的 machine-uid 也无法解密我们的密文。
//! - **Nonce**:每次加密随机生成 12 字节,与密文一起存入(不复用)。
//!
//! ## 存储格式
//!
//! `enc:v1:<base64(nonce(12) || ciphertext+tag)>`
//!
//! 前缀 `enc:v1:` 同时充当版本号 + 哨兵 — 解密侧识别已加密 vs 老明文 vs
//! 空字符串(unconfigured provider)三种状态。未来算法升级时 bump 到
//! `enc:v2:` 并保留 v1 解密路径直到下次 schema 迁移。

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

/// 32 字节随机静态盐。这是 build-time 常量 — 同一份二进制的所有用户
/// 共享同一个 salt,但 machine-uid 已经把每个用户的 key 隔离开。salt
/// 的目的是防止"用户机器上其它应用拿同一个 machine-uid 用 SHA-256 算
/// 出我们的派生 key"。**不要改这个值** — 改了会让所有已加密的 api_key
/// 无法解密,等价于全局凭证清空。
const APP_SALT: &[u8; 32] = &[
    0x44, 0x69, 0x73, 0x6b, 0x4d, 0x69, 0x6e, 0x64, // "DiskMind"
    0x2d, 0x76, 0x31, 0x2d, 0x73, 0x61, 0x6c, 0x74, // "-v1-salt"
    0xa7, 0x3c, 0x91, 0x4f, 0xb8, 0x52, 0xd1, 0xe6, // 8 random bytes
    0x29, 0x7f, 0x4a, 0x86, 0x35, 0xc0, 0x9d, 0x14, // 8 random bytes
];

const MARKER_PREFIX: &str = "enc:v1:";

/// 加密失败 / 解密失败的统一错误。在 IPC 层会被进一步包成
/// `Result<T, String>`,但模块内部用 enum 让上层能区分根因。
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("machine uid unavailable: {0}")]
    MachineId(String),
    #[error("base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("AEAD payload too short (need >= 12 bytes for nonce)")]
    PayloadTooShort,
    #[error("AEAD encrypt/decrypt failed (wrong key or tampered ciphertext)")]
    Aead,
    #[error("UTF-8 decode failed: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

/// 派生主密钥。用 OnceLock 缓存,避免每次 encrypt/decrypt 都重新调
/// machine-uid + SHA-256。多线程安全 — OnceLock 内部已原子化。
fn derive_master_key() -> Result<&'static [u8; 32], CryptoError> {
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    if let Some(k) = KEY.get() {
        return Ok(k);
    }
    let uid = machine_uid::get().map_err(|e| CryptoError::MachineId(e.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(uid.as_bytes());
    hasher.update(APP_SALT);
    let digest: [u8; 32] = hasher.finalize().into();
    // race condition: 两个线程同时 derive,后写者的 set 会失败,但拿到的
    // 都是同一个值,此时直接读已 set 的;不会冲突。
    let _ = KEY.set(digest);
    Ok(KEY.get().expect("just set above"))
}

/// 加密 API Key。空字符串保持空(unconfigured provider 不加密),已经
/// 是 enc:v1: 前缀的视为已加密直接返回(幂等,允许 migration 跑两次)。
pub fn encrypt_api_key(plain: &str) -> Result<String, CryptoError> {
    if plain.is_empty() {
        return Ok(String::new());
    }
    if plain.starts_with(MARKER_PREFIX) {
        return Ok(plain.to_string());
    }
    let key_bytes = derive_master_key()?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key_bytes));
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plain.as_bytes())
        .map_err(|_| CryptoError::Aead)?;
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(nonce.as_slice());
    combined.extend_from_slice(&ciphertext);
    Ok(format!("{}{}", MARKER_PREFIX, B64.encode(combined)))
}

/// 解密。空字符串 / 不带 `enc:v1:` 前缀的(老明文)直接返回原值,允许
/// migration 之前已经被读到上层缓存的明文继续 work。
pub fn decrypt_api_key(stored: &str) -> Result<String, CryptoError> {
    if stored.is_empty() {
        return Ok(String::new());
    }
    if !stored.starts_with(MARKER_PREFIX) {
        // 老明文兜底:DB 里还没经过 v12 migration 的行,或者 v12 migration
        // 之外的代码路径写入的明文。返回原值,让调用方仍然能用,后续
        // upsert 会自动加密。
        return Ok(stored.to_string());
    }
    let body = &stored[MARKER_PREFIX.len()..];
    let combined = B64.decode(body)?;
    if combined.len() < 12 {
        return Err(CryptoError::PayloadTooShort);
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let key_bytes = derive_master_key()?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key_bytes));
    let plain = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| CryptoError::Aead)?;
    Ok(String::from_utf8(plain)?)
}

/// `is_encrypted("enc:v1:abc")` → true,空 / 老明文 → false。给 db
/// migration 用:只对"非空且非加密"的行做加密写回,避免重复加密把已加密
/// 的 base64 字符串再加密一层。
pub fn is_encrypted(s: &str) -> bool {
    s.starts_with(MARKER_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_round_trips_empty() {
        assert_eq!(encrypt_api_key("").unwrap(), "");
        assert_eq!(decrypt_api_key("").unwrap(), "");
    }

    #[test]
    fn round_trip_simple_ascii() {
        let plain = "sk-test-abc123";
        let enc = encrypt_api_key(plain).unwrap();
        assert!(enc.starts_with(MARKER_PREFIX));
        assert!(!enc.contains(plain), "ciphertext should not embed plaintext");
        let dec = decrypt_api_key(&enc).unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn round_trip_unicode() {
        // API key 一般是 ASCII,但兜底 unicode 不能炸 UTF-8 验证
        let plain = "key-中文-🔑-test";
        let enc = encrypt_api_key(plain).unwrap();
        let dec = decrypt_api_key(&enc).unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn each_encryption_uses_fresh_nonce() {
        // 同一明文加密两次 ciphertext 应该不同(随机 nonce 防重放
        // 与 known-plaintext 攻击)
        let plain = "same-plaintext";
        let a = encrypt_api_key(plain).unwrap();
        let b = encrypt_api_key(plain).unwrap();
        assert_ne!(a, b, "ciphertext must differ across encryptions");
        // 但都能解回原值
        assert_eq!(decrypt_api_key(&a).unwrap(), plain);
        assert_eq!(decrypt_api_key(&b).unwrap(), plain);
    }

    #[test]
    fn legacy_plaintext_passes_through() {
        // 没有 enc:v1: 前缀的字符串视为老明文,decrypt 返回原值
        let legacy = "legacy-plaintext-key";
        assert_eq!(decrypt_api_key(legacy).unwrap(), legacy);
    }

    #[test]
    fn already_encrypted_is_idempotent_on_encrypt() {
        // encrypt 给一个已经是 enc:v1: 的字符串,直接返回不再二次加密
        let plain = "abc";
        let enc = encrypt_api_key(plain).unwrap();
        let enc_again = encrypt_api_key(&enc).unwrap();
        assert_eq!(enc, enc_again);
    }

    #[test]
    fn tampered_ciphertext_fails_decrypt() {
        // AEAD 完整性:改一个字节就 decrypt 失败,不返回错误明文
        let plain = "secret";
        let enc = encrypt_api_key(plain).unwrap();
        let mut bytes = enc.into_bytes();
        // 改最后一个 base64 字符(在 ciphertext+tag 范围内)
        let last = bytes.len() - 1;
        bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(bytes).unwrap();
        assert!(decrypt_api_key(&tampered).is_err());
    }

    #[test]
    fn is_encrypted_detects_marker() {
        assert!(!is_encrypted(""));
        assert!(!is_encrypted("plain-key"));
        assert!(is_encrypted("enc:v1:abc"));
    }
}
