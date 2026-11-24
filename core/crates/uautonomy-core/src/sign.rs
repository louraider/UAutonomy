//! Підписи пакетів і каталогу ключем проєкту (Ed25519).
//!
//! SHA-256 у маніфесті захищає лише від пошкодження: хто підмінив файл,
//! може підмінити й маніфест. Тому маніфест підписується приватним ключем
//! проєкту, а застосунок і хаб перевіряють підпис вбудованими публічними
//! ключами ([`Trust::builtin`]) перед будь-яким встановленням: з інтернету,
//! з хаба, з іншого телефона по Wi-Fi чи з флешки.
//!
//! Підпис охоплює все, що впливає на вміст і показ пакета: id, рівень, тип,
//! назву, опис, версію, ліцензію, область, модель, дату підпису й список
//! файлів із розмірами та SHA-256. Прямі адреси файлів (`url`) не входять:
//! вони потрібні лише для завантаження, а кожен файл однаково звіряється з
//! підписаним SHA-256.
//!
//! Дата підпису (`issued`) захищає від відкату: не можна встановити старішу
//! підписану редакцію поверх новішої (наприклад, відкликану картку).
//!
//! Каталог (`catalog.json`) з інтернету додатково підписується окремим
//! файлом `catalog.json.sig` (див. [`sign_bytes`], [`Trust::verify_bytes`]).

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::pack::{PackKind, PackManifest, PackTier};
use crate::Error;

/// Формат підписаного вмісту. Якщо колись додамо поля в підпис, буде v2,
/// а старі підписи v1 лишаться дійсними.
pub const PACK_FORMAT: &str = "uautonomy-pack-v1";
const BYTES_CONTEXT: &[u8] = b"uautonomy-file-v1\n";

/// Підпис маніфесту.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackSignature {
    /// Ідентифікатор ключа: перші 8 байтів SHA-256 публічного ключа, hex.
    pub key_id: String,
    /// Коли підписано (секунди Unix). Входить у підпис.
    pub issued: u64,
    /// Ed25519-підпис, hex (128 символів).
    pub sig: String,
}

/// Окремий підпис файлу (`catalog.json.sig`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetachedSignature {
    pub key_id: String,
    pub sig: String,
}

#[derive(Serialize)]
struct SignedFile<'a> {
    path: &'a str,
    size: u64,
    sha256: String,
}

/// Те, що реально підписується. Порядок і склад полів фіксовані форматом
/// [`PACK_FORMAT`], тому нові необов'язкові поля маніфесту не ламають
/// перевірку в старих версіях застосунку.
#[derive(Serialize)]
struct SignedPack<'a> {
    format: &'static str,
    id: &'a str,
    tier: PackTier,
    kind: PackKind,
    title: &'a str,
    description: &'a str,
    version: &'a str,
    content_license: &'a str,
    region: &'a Option<String>,
    model_id: &'a Option<String>,
    issued: u64,
    files: Vec<SignedFile<'a>>,
}

/// Байти, які підписуються для маніфесту з датою `issued`.
pub fn pack_payload(m: &PackManifest, issued: u64) -> Vec<u8> {
    let p = SignedPack {
        format: PACK_FORMAT,
        id: &m.id,
        tier: m.tier,
        kind: m.kind,
        title: &m.title,
        description: &m.description,
        version: &m.version,
        content_license: &m.content_license,
        region: &m.region,
        model_id: &m.model_id,
        issued,
        files: m
            .files
            .iter()
            .map(|f| SignedFile {
                path: &f.path,
                size: f.size,
                sha256: f.sha256.to_ascii_lowercase(),
            })
            .collect(),
    };
    serde_json::to_vec(&p).expect("підписаний вміст серіалізується")
}

/// Ідентифікатор публічного ключа.
pub fn key_id(public: &VerifyingKey) -> String {
    hex::encode(&Sha256::digest(public.as_bytes())[..8])
}

/// Приватний ключ із 32-байтного seed у hex (як у секреті CI).
pub fn signing_key_from_hex(hex_seed: &str) -> Result<SigningKey, Error> {
    let bytes = hex::decode(hex_seed.trim())
        .map_err(|_| Error::Invalid("приватний ключ має бути hex".into()))?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| Error::Invalid("приватний ключ має містити 32 байти".into()))?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Публічний ключ у hex.
pub fn public_hex(key: &SigningKey) -> String {
    hex::encode(key.verifying_key().as_bytes())
}

/// Підписує маніфест (замінює наявний підпис).
pub fn sign_pack(m: &mut PackManifest, key: &SigningKey, issued: u64) {
    let sig = key.sign(&pack_payload(m, issued));
    m.signature = Some(PackSignature {
        key_id: key_id(&key.verifying_key()),
        issued,
        sig: hex::encode(sig.to_bytes()),
    });
}

/// Підписує довільний файл (каталог).
pub fn sign_bytes(data: &[u8], key: &SigningKey) -> DetachedSignature {
    let mut msg = BYTES_CONTEXT.to_vec();
    msg.extend_from_slice(data);
    DetachedSignature {
        key_id: key_id(&key.verifying_key()),
        sig: hex::encode(key.sign(&msg).to_bytes()),
    }
}

/// Довірений публічний ключ (з `assets/keys/trusted.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedKey {
    /// Публічний ключ Ed25519, hex.
    pub public_key: String,
    /// Для людей: хто і навіщо тримає ключ.
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TrustedFile {
    keys: Vec<TrustedKey>,
}

/// Набір ключів, яким довіряє застосунок.
#[derive(Debug, Clone, Default)]
pub struct Trust {
    keys: Vec<(String, VerifyingKey)>,
}

impl Trust {
    /// Ключі, вшиті в застосунок і хаб.
    pub fn builtin() -> Trust {
        Trust::from_json(include_str!("../assets/keys/trusted.json"))
            .expect("вбудовані ключі коректні")
    }

    pub fn from_json(json: &str) -> Result<Trust, Error> {
        let f: TrustedFile = serde_json::from_str(json)?;
        let mut t = Trust::default();
        for k in f.keys {
            t.add_hex(&k.public_key)?;
        }
        Ok(t)
    }

    pub fn from_keys(keys: &[VerifyingKey]) -> Trust {
        Trust {
            keys: keys.iter().map(|k| (key_id(k), *k)).collect(),
        }
    }

    pub fn add_hex(&mut self, public_hex: &str) -> Result<(), Error> {
        let bytes: [u8; 32] = hex::decode(public_hex.trim())
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| Error::Invalid("публічний ключ має бути 32 байти в hex".into()))?;
        let k = VerifyingKey::from_bytes(&bytes)
            .map_err(|_| Error::Invalid("недійсний публічний ключ".into()))?;
        self.keys.push((key_id(&k), k));
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    fn key(&self, id: &str) -> Result<&VerifyingKey, Error> {
        self.keys
            .iter()
            .find(|(k, _)| k == id)
            .map(|(_, k)| k)
            .ok_or_else(|| Error::Untrusted(format!("невідомий ключ підпису {id}")))
    }

    /// Перевіряє підпис маніфесту. Помилка — пакет не встановлювати.
    pub fn verify_pack(&self, m: &PackManifest) -> Result<(), Error> {
        let s = m.signature.as_ref().ok_or_else(|| {
            Error::Untrusted(format!("пакет «{}» не підписаний проєктом", m.title))
        })?;
        let key = self.key(&s.key_id)?;
        let sig = parse_sig(&s.sig)?;
        key.verify(&pack_payload(m, s.issued), &sig)
            .map_err(|_| Error::Untrusted(format!("підпис пакета «{}» не збігається", m.title)))
    }

    /// Перевіряє окремий підпис файлу (каталогу).
    pub fn verify_bytes(&self, data: &[u8], s: &DetachedSignature) -> Result<(), Error> {
        let key = self.key(&s.key_id)?;
        let mut msg = BYTES_CONTEXT.to_vec();
        msg.extend_from_slice(data);
        key.verify(&msg, &parse_sig(&s.sig)?)
            .map_err(|_| Error::Untrusted("підпис каталогу не збігається".into()))
    }
}

fn parse_sig(s: &str) -> Result<Signature, Error> {
    let bytes: [u8; 64] = hex::decode(s)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| Error::Untrusted("підпис пошкоджено".into()))?;
    Ok(Signature::from_bytes(&bytes))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Тестовий ключ (лише для тестів, ніде не довірений).
    pub fn test_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    pub fn test_trust() -> Trust {
        Trust::from_keys(&[test_key().verifying_key()])
    }

    fn manifest() -> PackManifest {
        PackManifest::from_json(
            r#"{"id":"p","tier":"pocket","title":"Т","version":"1","content_license":"CC0-1.0",
                "files":[{"path":"cards.json","size":2,"sha256":"4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn sign_and_verify() {
        let mut m = manifest();
        let trust = test_trust();
        assert!(matches!(trust.verify_pack(&m), Err(Error::Untrusted(_))));
        sign_pack(&mut m, &test_key(), 100);
        trust.verify_pack(&m).unwrap();
        // Підпис переживає JSON і не залежить від адрес завантаження.
        let mut back = PackManifest::from_json(&m.to_json()).unwrap();
        back.files[0].url = Some("https://example.org/x".into());
        trust.verify_pack(&back).unwrap();
    }

    #[test]
    fn rejects_tampering_and_unknown_key() {
        let mut m = manifest();
        sign_pack(&mut m, &test_key(), 100);
        let trust = test_trust();

        let mut t = m.clone();
        t.files[0].sha256 = "00".repeat(32);
        assert!(trust.verify_pack(&t).is_err(), "підмінений SHA-256");
        let mut t = m.clone();
        t.title = "Інша назва".into();
        assert!(trust.verify_pack(&t).is_err(), "підмінена назва");
        let mut t = m.clone();
        t.signature.as_mut().unwrap().issued = 200;
        assert!(trust.verify_pack(&t).is_err(), "підмінена дата");

        let other = SigningKey::from_bytes(&[9u8; 32]);
        let mut t = m.clone();
        sign_pack(&mut t, &other, 100);
        let err = trust.verify_pack(&t).unwrap_err().to_string();
        assert!(err.contains("невідомий ключ"), "{err}");
        assert!(Trust::default().verify_pack(&m).is_err());
    }

    #[test]
    fn detached_signature() {
        let data = br#"{"packs":[]}"#;
        let s = sign_bytes(data, &test_key());
        test_trust().verify_bytes(data, &s).unwrap();
        assert!(test_trust().verify_bytes(b"{}", &s).is_err());
    }

    #[test]
    fn builtin_keys_parse() {
        let _ = Trust::builtin();
        let mut t = Trust::default();
        t.add_hex(&public_hex(&test_key())).unwrap();
        assert!(!t.is_empty());
    }
}
