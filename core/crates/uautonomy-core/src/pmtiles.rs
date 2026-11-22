//! Читач PMTiles v3 (офлайн-карти одним файлом). Застосунок бере з нього
//! векторні тайли (MVT) і малює їх сам, тож карта не потребує інтернету.
//! Формат: <https://github.com/protomaps/PMTiles/blob/main/spec/v3/spec.md>.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum PmError {
    #[error("помилка читання карти: {0}")]
    Io(#[from] std::io::Error),
    #[error("це не файл PMTiles v3")]
    NotPmtiles,
    #[error("пошкоджений файл карти: {0}")]
    Corrupt(&'static str),
    #[error("непідтримуване стиснення: {0}")]
    Compression(u8),
}

type Result<T> = std::result::Result<T, PmError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TileType {
    Unknown,
    Mvt,
    Png,
    Jpeg,
    Webp,
    Avif,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Header {
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// [захід, південь, схід, північ]
    pub bounds: [f64; 4],
    pub center: (f64, f64),
    pub center_zoom: u8,
    pub tile_type: TileType,
    #[serde(skip)]
    root_offset: u64,
    #[serde(skip)]
    root_length: u64,
    #[serde(skip)]
    metadata_offset: u64,
    #[serde(skip)]
    metadata_length: u64,
    #[serde(skip)]
    leaf_offset: u64,
    #[serde(skip)]
    data_offset: u64,
    #[serde(skip)]
    internal_compression: u8,
    #[serde(skip)]
    tile_compression: u8,
}

#[derive(Debug, Clone, Copy)]
struct DirEntry {
    tile_id: u64,
    offset: u64,
    length: u32,
    run_length: u32,
}

pub struct PmTiles {
    file: Mutex<File>,
    pub header: Header,
    root: Vec<DirEntry>,
    /// Кеш останньої листової теки.
    leaf: Mutex<Option<(u64, Vec<DirEntry>)>>,
}

impl std::fmt::Debug for PmTiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PmTiles")
            .field("header", &self.header)
            .finish()
    }
}

fn u64le(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
fn i32le(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl PmTiles {
    pub fn open(path: impl AsRef<Path>) -> Result<PmTiles> {
        let mut file = File::open(path)?;
        let mut h = [0u8; 127];
        file.read_exact(&mut h).map_err(|_| PmError::NotPmtiles)?;
        if &h[0..7] != b"PMTiles" || h[7] != 3 {
            return Err(PmError::NotPmtiles);
        }
        let e7 = |o| i32le(&h, o) as f64 / 1e7;
        let header = Header {
            root_offset: u64le(&h, 8),
            root_length: u64le(&h, 16),
            metadata_offset: u64le(&h, 24),
            metadata_length: u64le(&h, 32),
            leaf_offset: u64le(&h, 40),
            data_offset: u64le(&h, 56),
            internal_compression: h[97],
            tile_compression: h[98],
            tile_type: match h[99] {
                1 => TileType::Mvt,
                2 => TileType::Png,
                3 => TileType::Jpeg,
                4 => TileType::Webp,
                5 => TileType::Avif,
                _ => TileType::Unknown,
            },
            min_zoom: h[100],
            max_zoom: h[101],
            bounds: [e7(102), e7(106), e7(110), e7(114)],
            center_zoom: h[118],
            center: (e7(123), e7(119)),
        };
        let pm = PmTiles {
            file: Mutex::new(file),
            root: Vec::new(),
            leaf: Mutex::new(None),
            header,
        };
        let raw = pm.read(pm.header.root_offset, pm.header.root_length)?;
        let root = parse_dir(&decompress(pm.header.internal_compression, raw)?)?;
        Ok(PmTiles { root, ..pm })
    }

    fn read(&self, offset: u64, len: u64) -> Result<Vec<u8>> {
        if len > 256 << 20 {
            return Err(PmError::Corrupt("завеликий блок"));
        }
        let mut f = self.file.lock().unwrap();
        f.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; len as usize];
        f.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// JSON-метадані архіву (назва, атрибуція, шари).
    pub fn metadata(&self) -> Result<String> {
        let raw = self.read(self.header.metadata_offset, self.header.metadata_length)?;
        let data = decompress(self.header.internal_compression, raw)?;
        Ok(String::from_utf8_lossy(&data).into_owned())
    }

    /// Тайл z/x/y, уже розпакований (для MVT — сирий protobuf).
    pub fn tile(&self, z: u8, x: u32, y: u32) -> Result<Option<Vec<u8>>> {
        if z > 30 || x >= 1u32 << z || y >= 1u32 << z {
            return Ok(None);
        }
        let id = zxy_to_tile_id(z, x, y);
        let mut dir = self.root.clone();
        for _ in 0..4 {
            let Some(e) = find(&dir, id) else {
                return Ok(None);
            };
            if e.run_length > 0 {
                let raw = self.read(self.header.data_offset + e.offset, e.length as u64)?;
                return Ok(Some(decompress(self.header.tile_compression, raw)?));
            }
            let key = e.offset;
            let cached = self
                .leaf
                .lock()
                .unwrap()
                .as_ref()
                .filter(|(k, _)| *k == key)
                .map(|(_, d)| d.clone());
            dir = match cached {
                Some(d) => d,
                None => {
                    let raw = self.read(self.header.leaf_offset + e.offset, e.length as u64)?;
                    let d = parse_dir(&decompress(self.header.internal_compression, raw)?)?;
                    *self.leaf.lock().unwrap() = Some((key, d.clone()));
                    d
                }
            };
        }
        Err(PmError::Corrupt("завглибока ієрархія тек"))
    }
}

fn find(dir: &[DirEntry], id: u64) -> Option<DirEntry> {
    let idx = dir.partition_point(|e| e.tile_id <= id).checked_sub(1)?;
    let e = dir[idx];
    if e.run_length == 0 || id < e.tile_id + e.run_length as u64 {
        Some(e)
    } else {
        None
    }
}

fn decompress(kind: u8, data: Vec<u8>) -> Result<Vec<u8>> {
    match kind {
        0 | 1 => Ok(data),
        2 => {
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(&data[..]).read_to_end(&mut out)?;
            Ok(out)
        }
        4 => {
            let mut out = Vec::new();
            ruzstd::decoding::StreamingDecoder::new(&data[..])
                .map_err(|_| PmError::Corrupt("zstd"))?
                .read_to_end(&mut out)?;
            Ok(out)
        }
        k => Err(PmError::Compression(k)),
    }
}

fn varint(b: &[u8], pos: &mut usize) -> Result<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*pos).ok_or(PmError::Corrupt("варінт"))?;
        *pos += 1;
        v |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err(PmError::Corrupt("варінт"))
}

fn parse_dir(b: &[u8]) -> Result<Vec<DirEntry>> {
    let mut p = 0;
    let n = varint(b, &mut p)? as usize;
    if n > 10_000_000 {
        return Err(PmError::Corrupt("тека"));
    }
    let mut e = vec![
        DirEntry {
            tile_id: 0,
            offset: 0,
            length: 0,
            run_length: 0
        };
        n
    ];
    let mut last = 0u64;
    for x in e.iter_mut() {
        last += varint(b, &mut p)?;
        x.tile_id = last;
    }
    for x in e.iter_mut() {
        x.run_length = varint(b, &mut p)? as u32;
    }
    for x in e.iter_mut() {
        x.length = varint(b, &mut p)? as u32;
    }
    for i in 0..n {
        let v = varint(b, &mut p)?;
        e[i].offset = if v == 0 && i > 0 {
            e[i - 1].offset + e[i - 1].length as u64
        } else {
            v.checked_sub(1).ok_or(PmError::Corrupt("зміщення"))?
        };
    }
    Ok(e)
}

/// Номер тайла на кривій Гільберта, як у специфікації PMTiles.
pub fn zxy_to_tile_id(z: u8, x: u32, y: u32) -> u64 {
    let base: u64 = (0..z).map(|t| 1u64 << (2 * t)).sum();
    let n = 1u64 << z;
    let (mut x, mut y) = (x as u64, y as u64);
    let mut d = 0u64;
    let mut s = n / 2;
    while s > 0 {
        let rx = u64::from(x & s > 0);
        let ry = u64::from(y & s > 0);
        d += s * s * ((3 * rx) ^ ry);
        if ry == 0 {
            if rx == 1 {
                x = n - 1 - x;
                y = n - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        s /= 2;
    }
    base + d
}

/// Мінімальний записувач для тестів: один корінь, без стиснення тек.
#[cfg(test)]
pub(crate) type TestTile = ((u8, u32, u32), Vec<u8>);

#[cfg(test)]
pub(crate) fn write_test_pmtiles(tiles: &[TestTile], gzip_tiles: bool) -> Vec<u8> {
    use std::io::Write;
    let mut entries: Vec<(u64, Vec<u8>)> = tiles
        .iter()
        .map(|((z, x, y), d)| {
            let data = if gzip_tiles {
                let mut enc =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
                enc.write_all(d).unwrap();
                enc.finish().unwrap()
            } else {
                d.clone()
            };
            (zxy_to_tile_id(*z, *x, *y), data)
        })
        .collect();
    entries.sort_by_key(|e| e.0);
    let put = |v: &mut Vec<u8>, mut x: u64| loop {
        let b = (x & 0x7f) as u8;
        x >>= 7;
        if x == 0 {
            v.push(b);
            break;
        }
        v.push(b | 0x80);
    };
    let mut dir = Vec::new();
    put(&mut dir, entries.len() as u64);
    let mut last = 0;
    for (id, _) in &entries {
        put(&mut dir, id - last);
        last = *id;
    }
    for _ in &entries {
        put(&mut dir, 1);
    }
    for (_, d) in &entries {
        put(&mut dir, d.len() as u64);
    }
    let mut off = 0u64;
    for (_, d) in &entries {
        put(&mut dir, off + 1);
        off += d.len() as u64;
    }
    let meta = br#"{"name":"test"}"#;
    let root_off = 127u64;
    let meta_off = root_off + dir.len() as u64;
    let data_off = meta_off + meta.len() as u64;
    let mut h = vec![0u8; 127];
    h[0..7].copy_from_slice(b"PMTiles");
    h[7] = 3;
    let mut set = |o: usize, v: u64| h[o..o + 8].copy_from_slice(&v.to_le_bytes());
    set(8, root_off);
    set(16, dir.len() as u64);
    set(24, meta_off);
    set(32, meta.len() as u64);
    set(40, data_off);
    set(48, 0);
    set(56, data_off);
    set(64, off);
    h[97] = 1;
    h[98] = if gzip_tiles { 2 } else { 1 };
    h[99] = 1;
    h[100] = 0;
    h[101] = 14;
    h[102..106].copy_from_slice(&(302_000_000i32).to_le_bytes());
    h[106..110].copy_from_slice(&(502_000_000i32).to_le_bytes());
    h[110..114].copy_from_slice(&(309_000_000i32).to_le_bytes());
    h[114..118].copy_from_slice(&(506_000_000i32).to_le_bytes());
    let mut out = h;
    out.extend_from_slice(&dir);
    out.extend_from_slice(meta);
    for (_, d) in &entries {
        out.extend_from_slice(d);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hilbert_ids_match_spec() {
        assert_eq!(zxy_to_tile_id(0, 0, 0), 0);
        assert_eq!(zxy_to_tile_id(1, 0, 0), 1);
        assert_eq!(zxy_to_tile_id(1, 0, 1), 2);
        assert_eq!(zxy_to_tile_id(1, 1, 1), 3);
        assert_eq!(zxy_to_tile_id(1, 1, 0), 4);
        assert_eq!(zxy_to_tile_id(2, 0, 0), 5);
        // Значення зі специфікації / go-pmtiles.
        assert_eq!(zxy_to_tile_id(3, 7, 7), 63);
        assert_eq!(zxy_to_tile_id(20, 0, 0), 366503875925);
        // Властивість кривої Гільберта: усі номери різні, а сусідні номери —
        // сусідні тайли.
        for z in 1..6u8 {
            let n = 1u32 << z;
            let mut by_id = vec![(0u32, 0u32); (n * n) as usize];
            let base = zxy_to_tile_id(z, 0, 0);
            for x in 0..n {
                for y in 0..n {
                    by_id[(zxy_to_tile_id(z, x, y) - base) as usize] = (x, y);
                }
            }
            for w in by_id.windows(2) {
                let d = w[0].0.abs_diff(w[1].0) + w[0].1.abs_diff(w[1].1);
                assert_eq!(d, 1, "z{z} {:?}", w);
            }
        }
    }

    #[test]
    fn reads_tiles() {
        let dir = tempfile::tempdir().unwrap();
        for gz in [false, true] {
            let p = dir.path().join(format!("t{gz}.pmtiles"));
            std::fs::write(
                &p,
                write_test_pmtiles(
                    &[
                        ((0, 0, 0), b"root".to_vec()),
                        ((14, 9577, 5541), b"kyiv".to_vec()),
                    ],
                    gz,
                ),
            )
            .unwrap();
            let pm = PmTiles::open(&p).unwrap();
            assert_eq!(pm.header.tile_type, TileType::Mvt);
            assert_eq!(pm.header.max_zoom, 14);
            assert!((pm.header.bounds[0] - 30.2).abs() < 1e-6);
            assert_eq!(pm.tile(0, 0, 0).unwrap().unwrap(), b"root");
            assert_eq!(pm.tile(14, 9577, 5541).unwrap().unwrap(), b"kyiv");
            assert_eq!(pm.tile(14, 9577, 5542).unwrap(), None);
            assert_eq!(pm.tile(3, 9, 0).unwrap(), None);
            assert!(pm.metadata().unwrap().contains("test"));
        }
        let bad = dir.path().join("bad");
        std::fs::write(&bad, vec![0u8; 200]).unwrap();
        assert!(matches!(PmTiles::open(&bad), Err(PmError::NotPmtiles)));
    }
}
