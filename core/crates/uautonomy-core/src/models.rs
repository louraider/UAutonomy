//! Каталог моделей ШІ. Модель змінна: каталог — це JSON, його можна
//! замінити або доповнити (MamayLM, Lapa, власні донавчені версії), а
//! користувач може підкласти будь-який файл GGUF / LiteRT-LM вручну.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    /// llama.cpp
    Gguf,
    /// LiteRT-LM
    Litertlm,
}

impl ModelFormat {
    pub fn from_file_name(name: &str) -> Option<Self> {
        let n = name.to_lowercase();
        if n.ends_with(".gguf") {
            Some(ModelFormat::Gguf)
        } else if n.ends_with(".litertlm") {
            Some(ModelFormat::Litertlm)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    pub id: String,
    pub title: String,
    /// gemma4, mamaylm, lapa, ...
    pub family: String,
    pub format: ModelFormat,
    /// Орієнтовний розмір файлу.
    pub size_bytes: u64,
    /// Нижче цього обсягу ОЗП модель не пропонуємо зовсім.
    pub min_ram_mb: u64,
    /// З цього обсягу модель вмикається за замовчуванням.
    pub recommended_ram_mb: u64,
    pub context_size: u32,
    pub license: String,
    /// Пакет у каталозі, з якого береться файл (kind = model).
    #[serde(default)]
    pub pack_id: Option<String>,
    #[serde(default)]
    pub notes: String,
    /// Лише для ПК: на телефонах не пропонується зовсім.
    #[serde(default)]
    pub desktop_only: bool,
}

/// Вбудований каталог. Розміри приблизні (квантизація Q4); точні байти та
/// SHA-256 приходять з маніфесту пакета моделі.
pub const DEFAULT_CATALOG: &str = r#"[
  {"id":"gemma-4-e2b-it-q4","title":"Gemma 4 E2B (телефон)","family":"gemma4","format":"gguf",
   "size_bytes":3200000000,"min_ram_mb":3500,"recommended_ram_mb":6000,"context_size":4096,
   "license":"Apache-2.0","pack_id":"model-gemma-4-e2b-it-q4",
   "notes":"Базова модель для телефонів і запасна для ПК з малою пам'яттю."},
  {"id":"gemma-4-e4b-it-q4","title":"Gemma 4 E4B (слабкий ПК)","family":"gemma4","format":"gguf",
   "size_bytes":4977171584,"min_ram_mb":7000,"recommended_ram_mb":8000,"context_size":8192,
   "license":"Apache-2.0","pack_id":"model-gemma-4-e4b-it-q4","desktop_only":true,
   "notes":"Легка модель для ПК з 8–15 ГБ ОЗП."},
  {"id":"mamaylm-gemma-3-12b-it-q4","title":"MamayLM 12B (ПК)","family":"mamaylm","format":"gguf",
   "size_bytes":7300778080,"min_ram_mb":12000,"recommended_ram_mb":16000,"context_size":8192,
   "license":"Gemma Terms of Use","pack_id":"model-mamaylm-gemma-3-12b-it-q4","desktop_only":true,
   "notes":"Основна модель для ПК з 16+ ГБ ОЗП: українська MamayLM v1.0 (INSAIT) на Gemma 3, найкраща в наших тестах. Умови: ai.google.dev/gemma/terms."},
  {"id":"gemma-4-26b-a4b-it-q4","title":"Gemma 4 26B A4B (лише вручну)","family":"gemma4","format":"gguf",
   "size_bytes":16000000000,"min_ram_mb":24000,"recommended_ram_mb":99999999,"context_size":8192,
   "license":"Apache-2.0","pack_id":"model-gemma-4-26b-a4b-it-q4","desktop_only":true,
   "notes":"Не рекомендується: у тестах лише трохи краща за MamayLM 12B, а займає 17 ГБ і не влазить у 16 ГБ відеопам'яті."}
]"#;

pub fn default_catalog() -> Vec<ModelSpec> {
    serde_json::from_str(DEFAULT_CATALOG).expect("вбудований каталог моделей коректний")
}

pub fn parse_catalog(json: &str) -> Result<Vec<ModelSpec>, crate::Error> {
    Ok(serde_json::from_str(json)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_parses() {
        let c = default_catalog();
        assert!(c.iter().any(|m| m.family == "gemma4"));
        assert_eq!(
            ModelFormat::from_file_name("X.GGUF"),
            Some(ModelFormat::Gguf)
        );
        assert_eq!(
            ModelFormat::from_file_name("a.litertlm"),
            Some(ModelFormat::Litertlm)
        );
        assert_eq!(ModelFormat::from_file_name("a.bin"), None);
    }
}
