use serde::{Deserialize, Serialize};

/// Рівень пакета контенту.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackTier {
    /// ~100 МБ, вбудований у застосунок: термінові картки.
    Base,
    /// ~0,5 ГБ: «кишеньковий» набір.
    Pocket,
    /// ~15 ГБ: сімейний набір із картами регіону.
    Family,
    /// ~60 ГБ: громадський хаб (Raspberry Pi, домашній сервер).
    Community,
}

/// Що всередині пакета.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PackKind {
    /// Картки (`cards.json`, `cards/*.json`), статті (`articles/**/*.md`), довідники `*.zim`.
    #[default]
    Content,
    /// Офлайн-карта регіону (`*.pmtiles`).
    Map,
    /// Модель ШІ (`*.gguf` або `*.litertlm`).
    Model,
}

/// Один файл пакета.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackFile {
    /// Відносний шлях усередині пакета, через `/`.
    pub path: String,
    pub size: u64,
    /// SHA-256 у hex.
    pub sha256: String,
    /// Пряма адреса файлу, якщо він лежить не за `{base_url}/{id}/{path}`
    /// (GitHub Releases, Hugging Face). Лише в каталозі; у встановленому
    /// маніфесті не потрібна.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Опис пакета: лежить як `manifest.json` у теці пакета і в каталозі.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    pub id: String,
    pub tier: PackTier,
    #[serde(default)]
    pub kind: PackKind,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub version: String,
    pub files: Vec<PackFile>,
    /// SPDX-ідентифікатор або опис ліцензії вмісту.
    pub content_license: String,
    /// Для карт: ідентифікатор області з [`crate::regions`].
    #[serde(default)]
    pub region: Option<String>,
    /// Для моделей: ідентифікатор з [`crate::models`].
    #[serde(default)]
    pub model_id: Option<String>,
}

impl PackManifest {
    pub fn from_json(json: &str) -> Result<Self, crate::Error> {
        let m: PackManifest = serde_json::from_str(json)?;
        m.validate()?;
        Ok(m)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("маніфест серіалізується")
    }

    pub fn size_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Захист від шляхів на кшталт `../../etc/passwd` у чужих пакетах.
    pub fn validate(&self) -> Result<(), crate::Error> {
        if !is_safe_id(&self.id) {
            return Err(crate::Error::Invalid(format!(
                "недопустимий id пакета: {}",
                self.id
            )));
        }
        if self.files.is_empty() {
            return Err(crate::Error::Invalid("пакет без файлів".into()));
        }
        for f in &self.files {
            if !is_safe_rel_path(&f.path) || f.path == "manifest.json" {
                return Err(crate::Error::Invalid(format!(
                    "недопустимий шлях: {}",
                    f.path
                )));
            }
            if f.sha256.len() != 64 || !f.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(crate::Error::Invalid(format!(
                    "недопустимий SHA-256 для {}",
                    f.path
                )));
            }
        }
        Ok(())
    }
}

pub(crate) fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_' || b == b'.'
        })
        && !id.starts_with('.')
}

pub(crate) fn is_safe_rel_path(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with('/')
        && !p.contains('\\')
        && !p.contains(':')
        && p.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

/// Каталог пакетів: `catalog.json` на сервері, у хабі чи на іншому телефоні.
/// Файли пакета лежать за адресою `{base_url}/{id}/{path}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub base_url: String,
    pub packs: Vec<PackManifest>,
}

impl Catalog {
    pub fn from_json(json: &str) -> Result<Self, crate::Error> {
        let c: Catalog = serde_json::from_str(json)?;
        for p in &c.packs {
            p.validate()?;
        }
        Ok(c)
    }

    pub fn file_url(&self, pack: &PackManifest, file: &PackFile) -> String {
        if let Some(u) = &file.url {
            return u.clone();
        }
        format!(
            "{}/{}/{}",
            self.base_url.trim_end_matches('/'),
            pack.id,
            file.path
                .split('/')
                .map(crate::share::url_encode)
                .collect::<Vec<_>>()
                .join("/")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn parses_manifest() {
        let m = PackManifest::from_json(&format!(
            r#"{{"id":"pocket-uk","tier":"pocket","title":"Кишеньковий","version":"0.1.0",
               "files":[{{"path":"cards.json","size":0,"sha256":"{SHA}"}}],"content_license":"CC-BY-4.0"}}"#
        ))
        .unwrap();
        assert_eq!(m.tier, PackTier::Pocket);
        assert_eq!(m.kind, PackKind::Content);
    }

    #[test]
    fn rejects_unsafe_paths() {
        for bad in [
            "../x",
            "/etc/passwd",
            "a/../../b",
            "C:\\x",
            "a//b",
            "manifest.json",
        ] {
            let json = format!(
                r#"{{"id":"p","tier":"pocket","title":"t","version":"1",
                   "files":[{{"path":"{}","size":0,"sha256":"{SHA}"}}],"content_license":"x"}}"#,
                bad.replace('\\', "\\\\")
            );
            assert!(PackManifest::from_json(&json).is_err(), "{bad}");
        }
        assert!(!is_safe_id("../x"));
        assert!(is_safe_id("map-ua-kyiv"));
    }
}
