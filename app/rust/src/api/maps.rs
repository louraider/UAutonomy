use uautonomy_core::{regions, PackKind};

use crate::state;

pub struct RegionView {
    pub id: String,
    pub title: String,
    /// [захід, південь, схід, північ]
    pub bbox: Vec<f64>,
    pub center_lat: f64,
    pub center_lon: f64,
    pub pack_id: String,
    /// Шлях до встановленого файлу `.pmtiles`, якщо карта є на пристрої.
    pub map_path: Option<String>,
}

#[flutter_rust_bridge::frb(sync)]
pub fn map_regions() -> Vec<RegionView> {
    let installed = state::app_ref()
        .map(|a| a.shared.read().unwrap().store.clone())
        .ok();
    regions::REGIONS
        .iter()
        .map(|r| {
            let map_path = installed.as_ref().and_then(|store| {
                let m = store
                    .installed()
                    .into_iter()
                    .find(|m| m.kind == PackKind::Map && m.region.as_deref() == Some(r.id))?;
                let f = m.files.iter().find(|f| f.path.ends_with(".pmtiles"))?;
                store
                    .file_path(&m.id, &f.path)
                    .map(|p| p.to_string_lossy().into_owned())
            });
            let (lat, lon) = r.center();
            RegionView {
                id: r.id.into(),
                title: r.title.into(),
                bbox: r.bbox.to_vec(),
                center_lat: lat,
                center_lon: lon,
                pack_id: r.pack_id(),
                map_path,
            }
        })
        .collect()
}

/// Область за координатами (для «моя область» з GPS).
#[flutter_rust_bridge::frb(sync)]
pub fn region_at(lat: f64, lon: f64) -> Option<String> {
    regions::at(lat, lon).map(|r| r.id.to_string())
}
