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

/// Опис завантажуваного пакета.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    pub id: String,
    pub tier: PackTier,
    pub title: String,
    pub version: String,
    pub size_bytes: u64,
    /// SHA-256 архіву пакета у hex.
    pub sha256: String,
    /// SPDX-ідентифікатор або опис ліцензії вмісту.
    pub content_license: String,
}

impl PackManifest {
    pub fn from_json(json: &str) -> Result<Self, crate::Error> {
        Ok(serde_json::from_str(json)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest() {
        let m = PackManifest::from_json(
            r#"{"id":"base-uk","tier":"base","title":"Базовий","version":"0.1.0",
               "size_bytes":1024,"sha256":"00","content_license":"CC-BY-4.0"}"#,
        )
        .unwrap();
        assert_eq!(m.tier, PackTier::Base);
    }
}
