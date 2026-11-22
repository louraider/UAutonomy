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

pub struct MapHeader {
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// [захід, південь, схід, північ]
    pub bounds: Vec<f64>,
    pub center_lat: f64,
    pub center_lon: f64,
    pub vector: bool,
}

fn open_map(path: &str) -> anyhow::Result<std::sync::Arc<uautonomy_core::pmtiles::PmTiles>> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};
    static OPEN: OnceLock<Mutex<HashMap<String, Arc<uautonomy_core::pmtiles::PmTiles>>>> =
        OnceLock::new();
    let mut m = OPEN.get_or_init(Default::default).lock().unwrap();
    if let Some(p) = m.get(path) {
        return Ok(p.clone());
    }
    let p = Arc::new(uautonomy_core::pmtiles::PmTiles::open(path)?);
    m.insert(path.to_string(), p.clone());
    Ok(p)
}

pub fn map_header(path: String) -> anyhow::Result<MapHeader> {
    let h = open_map(&path)?.header.clone();
    Ok(MapHeader {
        min_zoom: h.min_zoom,
        max_zoom: h.max_zoom,
        bounds: h.bounds.to_vec(),
        center_lat: h.center.0,
        center_lon: h.center.1,
        vector: h.tile_type == uautonomy_core::pmtiles::TileType::Mvt,
    })
}

/// Тайл z/x/y з файлу карти (розпакований). `None` — тайла немає.
pub fn map_tile(path: String, z: u8, x: u32, y: u32) -> anyhow::Result<Option<Vec<u8>>> {
    Ok(open_map(&path)?.tile(z, x, y)?)
}
