//! Читач файлів ZIM (формат Kiwix/openZIM) на чистому Rust.
//!
//! Чому не libzim: це C++ з залежностями (xapian, icu), і збирати його під
//! Android, Windows і Raspberry Pi окремо дорого. Для читання статей нам
//! досить заголовка, покажчиків і кластерів (zstd або xz), а повнотекстовий
//! пошук ми робимо своїм індексом. Формат: <https://wiki.openzim.org/wiki/ZIM_file_format>.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

const MAGIC: u32 = 72_173_914;
const REDIRECT: u16 = 0xffff;
const LINK_TARGET: u16 = 0xfffe;
const DELETED: u16 = 0xfffd;

#[derive(Debug, thiserror::Error)]
pub enum ZimError {
    #[error("помилка читання ZIM: {0}")]
    Io(#[from] std::io::Error),
    #[error("це не файл ZIM")]
    NotZim,
    #[error("пошкоджений ZIM: {0}")]
    Corrupt(&'static str),
    #[error("непідтримуване стиснення кластера: {0}")]
    Compression(u8),
}

type Result<T> = std::result::Result<T, ZimError>;

#[derive(Debug, Clone)]
struct Header {
    entry_count: u32,
    cluster_count: u32,
    url_ptr_pos: u64,
    title_ptr_pos: u64,
    cluster_ptr_pos: u64,
    main_page: u32,
    checksum_pos: u64,
}

/// Запис каталогу ZIM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub index: u32,
    pub namespace: char,
    pub url: String,
    pub title: String,
    pub mime: Option<String>,
    target: Target,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Blob { cluster: u32, blob: u32 },
    Redirect(u32),
    None,
}

impl Entry {
    pub fn is_redirect(&self) -> bool {
        matches!(self.target, Target::Redirect(_))
    }
    /// Шлях у стилі Kiwix: `C/Main_Page` або `A/Стаття`.
    pub fn path(&self) -> String {
        format!("{}/{}", self.namespace, self.url)
    }
}

/// Номер кластера, розпаковані дані, чи 8-байтові зміщення.
type CachedCluster = (u32, std::sync::Arc<Vec<u8>>, bool);

pub struct Zim {
    file: Mutex<File>,
    header: Header,
    mimes: Vec<String>,
    /// Останній розпакований кластер (номер, дані, розмір зміщень).
    cache: Mutex<Option<CachedCluster>>,
    /// Покажчики за заголовком, завантажені при першому пошуку.
    titles: std::sync::OnceLock<TitleIndex>,
}

/// Звідки брати порядок за заголовком.
#[derive(Debug)]
enum TitleIndex {
    /// `X/listing/titleOrdered/v1`: лише статті, у пам'яті (новий формат).
    Listing(Vec<u32>),
    /// Список із заголовка файлу (старий формат і libzim до 9).
    Header(u32),
    None,
}

impl std::fmt::Debug for Zim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Zim")
            .field("entries", &self.header.entry_count)
            .finish()
    }
}

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u64le(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

impl Zim {
    pub fn open(path: impl AsRef<Path>) -> Result<Zim> {
        let mut file = File::open(path)?;
        let mut h = [0u8; 80];
        file.read_exact(&mut h).map_err(|_| ZimError::NotZim)?;
        if u32le(&h, 0) != MAGIC {
            return Err(ZimError::NotZim);
        }
        let header = Header {
            entry_count: u32le(&h, 24),
            cluster_count: u32le(&h, 28),
            url_ptr_pos: u64le(&h, 32),
            title_ptr_pos: u64le(&h, 40),
            cluster_ptr_pos: u64le(&h, 48),
            main_page: u32le(&h, 64),
            checksum_pos: u64le(&h, 72),
        };
        let mime_pos = u64le(&h, 56);
        let mime_len = header.url_ptr_pos.saturating_sub(mime_pos).min(1 << 20) as usize;
        let mut mbuf = vec![0u8; mime_len];
        file.seek(SeekFrom::Start(mime_pos))?;
        file.read_exact(&mut mbuf)?;
        let mut mimes = Vec::new();
        for part in mbuf.split(|b| *b == 0) {
            if part.is_empty() {
                break;
            }
            mimes.push(String::from_utf8_lossy(part).into_owned());
        }
        Ok(Zim {
            file: Mutex::new(file),
            header,
            mimes,
            cache: Mutex::new(None),
            titles: std::sync::OnceLock::new(),
        })
    }

    pub fn entry_count(&self) -> u32 {
        self.header.entry_count
    }

    /// Простір імен статей: `C` у новому форматі (ZIM 6.1+), `A` у старому.
    pub fn content_namespace(&self) -> char {
        if self.has_namespace('A') && !self.has_namespace('C') {
            'A'
        } else {
            'C'
        }
    }

    fn has_namespace(&self, ns: char) -> bool {
        let (mut lo, mut hi) = (0u32, self.header.entry_count);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match self.entry(mid) {
                Ok(e) if e.namespace < ns => lo = mid + 1,
                Ok(_) => hi = mid,
                Err(_) => return false,
            }
        }
        lo < self.header.entry_count && self.entry(lo).is_ok_and(|e| e.namespace == ns)
    }

    fn read_at(&self, pos: u64, buf: &mut [u8]) -> Result<()> {
        let mut f = self.file.lock().unwrap();
        f.seek(SeekFrom::Start(pos))?;
        f.read_exact(buf)?;
        Ok(())
    }

    fn url_ptr(&self, idx: u32) -> Result<u64> {
        let mut b = [0u8; 8];
        self.read_at(self.header.url_ptr_pos + idx as u64 * 8, &mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    /// Новіші libzim не пишуть список заголовків у шапку файлу (або пишуть
    /// повний, з картинками), а кладуть упорядкований перелік статей у
    /// `X/listing/titleOrdered/v1`. Беремо його, а шапку лише як запасний шлях.
    fn title_index(&self) -> &TitleIndex {
        self.titles.get_or_init(|| {
            let n = self.header.entry_count;
            let len = self
                .file
                .lock()
                .unwrap()
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0);
            let end = self.header.title_ptr_pos.checked_add(n as u64 * 4);
            let header_ok = self.header.title_ptr_pos >= 80 && end.is_some_and(|e| e <= len);
            let listing: Vec<u32> = self
                .get_by_path('X', "listing/titleOrdered/v1")
                .and_then(|e| self.content(&e).ok())
                .map(|(_, b)| {
                    b.as_chunks::<4>()
                        .0
                        .iter()
                        .map(|c| u32::from_le_bytes(*c))
                        .filter(|i| *i < n)
                        .collect()
                })
                .unwrap_or_default();
            // Деякі архіви (zimgit) позначають статтею лише головну сторінку:
            // тоді повний список із шапки корисніший.
            let tiny = (listing.len() as u64) * 20 < n as u64;
            if !listing.is_empty() && !(tiny && header_ok) {
                TitleIndex::Listing(listing)
            } else if header_ok {
                TitleIndex::Header(n)
            } else {
                TitleIndex::None
            }
        })
    }

    /// Опис джерела порядку заголовків (для діагностики).
    pub fn title_index_info(&self) -> String {
        match self.title_index() {
            TitleIndex::Listing(v) => format!("titleOrdered/v1, {} статей", v.len()),
            TitleIndex::Header(n) => format!("шапка файлу, {n} записів"),
            TitleIndex::None => "немає".into(),
        }
    }

    fn title_len(&self) -> u32 {
        match self.title_index() {
            TitleIndex::Listing(v) => v.len() as u32,
            TitleIndex::Header(n) => *n,
            TitleIndex::None => 0,
        }
    }

    fn title_ptr(&self, idx: u32) -> Result<u32> {
        if let TitleIndex::Listing(v) = self.title_index() {
            return v
                .get(idx as usize)
                .copied()
                .ok_or(ZimError::Corrupt("номер у переліку заголовків"));
        }
        let mut b = [0u8; 4];
        self.read_at(self.header.title_ptr_pos + idx as u64 * 4, &mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    /// Читає запис за номером у списку URL.
    pub fn entry(&self, idx: u32) -> Result<Entry> {
        if idx >= self.header.entry_count {
            return Err(ZimError::Corrupt("номер запису поза межами"));
        }
        let pos = self.url_ptr(idx)?;
        // Записи короткі; читаємо з запасом і дочитуємо за потреби.
        let mut len = 512usize;
        loop {
            let mut buf = vec![0u8; len];
            let read = {
                let mut f = self.file.lock().unwrap();
                f.seek(SeekFrom::Start(pos))?;
                let mut total = 0;
                while total < len {
                    let n = f.read(&mut buf[total..])?;
                    if n == 0 {
                        break;
                    }
                    total += n;
                }
                total
            };
            buf.truncate(read);
            if let Some(e) = parse_entry(idx, &buf, &self.mimes) {
                return Ok(e);
            }
            if read < len || len > 1 << 16 {
                return Err(ZimError::Corrupt("запис каталогу"));
            }
            len *= 4;
        }
    }

    fn key_cmp(e: &Entry, ns: char, s: &str, by_title: bool) -> std::cmp::Ordering {
        let k = if by_title { &e.title } else { &e.url };
        (e.namespace, k.as_str()).cmp(&(ns, s))
    }

    /// Пошук за точним шляхом (простір імен + URL), двійковий пошук.
    pub fn get_by_path(&self, ns: char, url: &str) -> Option<Entry> {
        let (mut lo, mut hi) = (0u32, self.header.entry_count);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let e = self.entry(mid).ok()?;
            match Self::key_cmp(&e, ns, url, false) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return Some(e),
            }
        }
        None
    }

    /// Приймає шлях `C/Стаття`, `A/Стаття` або просто `Стаття`.
    pub fn get(&self, path: &str) -> Option<Entry> {
        let mut chars = path.chars();
        if let (Some(ns), Some('/')) = (chars.next(), chars.next()) {
            if let Some(e) = self.get_by_path(ns, &path[ns.len_utf8() + 1..]) {
                return Some(e);
            }
        }
        let ns = self.content_namespace();
        self.get_by_path(ns, path)
    }

    /// Статті, заголовок яких починається з `prefix` (з урахуванням регістру,
    /// як у ZIM). Для пошуку без регістру викликайте з кількома варіантами.
    pub fn find_by_title_prefix(&self, ns: char, prefix: &str, limit: usize) -> Vec<Entry> {
        let n = self.title_len();
        let (mut lo, mut hi) = (0u32, n);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let Ok(e) = self.title_ptr(mid).and_then(|i| self.entry(i)) else {
                return Vec::new();
            };
            if Self::key_cmp(&e, ns, prefix, true) == std::cmp::Ordering::Less {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let mut out = Vec::new();
        let mut i = lo;
        while i < n && out.len() < limit {
            let Ok(e) = self.title_ptr(i).and_then(|x| self.entry(x)) else {
                break;
            };
            if e.namespace != ns || !e.title.starts_with(prefix) {
                break;
            }
            if e.mime
                .as_deref()
                .is_some_and(|m| m.starts_with("text/html"))
                || e.is_redirect()
            {
                out.push(e);
            }
            i += 1;
        }
        out
    }

    /// Головна сторінка архіву.
    pub fn main_page(&self) -> Option<Entry> {
        if self.header.main_page == u32::MAX {
            return None;
        }
        self.entry(self.header.main_page).ok()
    }

    /// Метадані (`M/Title`, `M/Language`, ...).
    pub fn metadata(&self, name: &str) -> Option<String> {
        let e = self.get_by_path('M', name)?;
        self.content(&e)
            .ok()
            .map(|(_, b)| String::from_utf8_lossy(&b).into_owned())
    }

    /// Іде за перенаправленнями (не більше 16 кроків).
    pub fn resolve(&self, e: &Entry) -> Result<Entry> {
        let mut cur = e.clone();
        for _ in 0..16 {
            match cur.target {
                Target::Redirect(i) => cur = self.entry(i)?,
                _ => return Ok(cur),
            }
        }
        Err(ZimError::Corrupt("цикл перенаправлень"))
    }

    /// Вміст запису: (MIME, байти).
    pub fn content(&self, e: &Entry) -> Result<(String, Vec<u8>)> {
        let e = self.resolve(e)?;
        let Target::Blob { cluster, blob } = e.target else {
            return Err(ZimError::Corrupt("запис без вмісту"));
        };
        let data = self.blob(cluster, blob)?;
        Ok((e.mime.unwrap_or_default(), data))
    }

    fn cluster_bounds(&self, c: u32) -> Result<(u64, u64)> {
        let mut b = [0u8; 8];
        self.read_at(self.header.cluster_ptr_pos + c as u64 * 8, &mut b)?;
        let start = u64::from_le_bytes(b);
        let end = if c + 1 < self.header.cluster_count {
            self.read_at(self.header.cluster_ptr_pos + (c as u64 + 1) * 8, &mut b)?;
            u64::from_le_bytes(b)
        } else {
            self.header.checksum_pos
        };
        if end <= start {
            return Err(ZimError::Corrupt("межі кластера"));
        }
        Ok((start, end))
    }

    fn cluster(&self, c: u32) -> Result<(std::sync::Arc<Vec<u8>>, bool)> {
        if let Some((n, data, ext)) = self.cache.lock().unwrap().as_ref() {
            if *n == c {
                return Ok((data.clone(), *ext));
            }
        }
        if c >= self.header.cluster_count {
            return Err(ZimError::Corrupt("номер кластера"));
        }
        let (start, end) = self.cluster_bounds(c)?;
        let mut raw = vec![0u8; (end - start) as usize];
        self.read_at(start, &mut raw)?;
        let info = raw[0];
        let extended = info & 0x10 != 0;
        let data = match info & 0x0f {
            0 | 1 => raw[1..].to_vec(),
            4 => {
                let mut out = Vec::new();
                lzma_rs::xz_decompress(&mut &raw[1..], &mut out)
                    .map_err(|_| ZimError::Corrupt("xz"))?;
                out
            }
            5 => {
                let mut out = Vec::new();
                let mut dec = ruzstd::decoding::StreamingDecoder::new(&raw[1..])
                    .map_err(|_| ZimError::Corrupt("zstd"))?;
                dec.read_to_end(&mut out)?;
                out
            }
            other => return Err(ZimError::Compression(other)),
        };
        let data = std::sync::Arc::new(data);
        *self.cache.lock().unwrap() = Some((c, data.clone(), extended));
        Ok((data, extended))
    }

    fn blob(&self, c: u32, b: u32) -> Result<Vec<u8>> {
        let (data, ext) = self.cluster(c)?;
        let w = if ext { 8 } else { 4 };
        let off = |i: usize| -> Option<usize> {
            let o = i * w;
            if o + w > data.len() {
                return None;
            }
            Some(if ext {
                u64le(&data, o) as usize
            } else {
                u32le(&data, o) as usize
            })
        };
        let first = off(0).ok_or(ZimError::Corrupt("кластер"))?;
        let count = first / w - 1;
        if b as usize >= count {
            return Err(ZimError::Corrupt("номер блоба"));
        }
        let (s, e) = (
            off(b as usize).ok_or(ZimError::Corrupt("кластер"))?,
            off(b as usize + 1).ok_or(ZimError::Corrupt("кластер"))?,
        );
        if s > e || e > data.len() {
            return Err(ZimError::Corrupt("межі блоба"));
        }
        Ok(data[s..e].to_vec())
    }

    /// Перебирає всі HTML-статті простору вмісту (для побудови індексу).
    pub fn articles(&self) -> impl Iterator<Item = Entry> + '_ {
        let ns = self.content_namespace();
        (0..self.header.entry_count).filter_map(move |i| {
            let e = self.entry(i).ok()?;
            (e.namespace == ns
                && !e.is_redirect()
                && e.mime
                    .as_deref()
                    .is_some_and(|m| m.starts_with("text/html")))
            .then_some(e)
        })
    }
}

fn parse_entry(index: u32, b: &[u8], mimes: &[String]) -> Option<Entry> {
    if b.len() < 12 {
        return None;
    }
    let mime_idx = u16le(b, 0);
    let param_len = b[2] as usize;
    let namespace = b[3] as char;
    let (target, mut p) = match mime_idx {
        REDIRECT => (Target::Redirect(u32le(b, 8)), 12),
        LINK_TARGET | DELETED => (Target::None, 8),
        _ => {
            if b.len() < 16 {
                return None;
            }
            (
                Target::Blob {
                    cluster: u32le(b, 8),
                    blob: u32le(b, 12),
                },
                16,
            )
        }
    };
    let url_end = p + b.get(p..)?.iter().position(|c| *c == 0)?;
    let url = String::from_utf8_lossy(&b[p..url_end]).into_owned();
    p = url_end + 1;
    let title_end = p + b.get(p..)?.iter().position(|c| *c == 0)?;
    let title = String::from_utf8_lossy(&b[p..title_end]).into_owned();
    if b.len() < title_end + 1 + param_len {
        return None;
    }
    let mime = if mime_idx < REDIRECT - 2 {
        mimes.get(mime_idx as usize).cloned()
    } else {
        None
    };
    Some(Entry {
        index,
        namespace,
        title: if title.is_empty() { url.clone() } else { title },
        url,
        mime,
        target,
    })
}

/// Мінімальний записувач ZIM для тестів і для збирання власних невеликих
/// довідників у `tools/`. Пише один кластер, стиснутий zstd.
pub mod writer {
    use std::io::Write;

    pub struct Item {
        pub namespace: char,
        pub url: String,
        pub title: String,
        pub mime: String,
        pub data: Vec<u8>,
    }

    pub fn write_zim(
        mut items: Vec<Item>,
        main_url: Option<&str>,
        out: &mut impl Write,
    ) -> std::io::Result<()> {
        // Як сучасний libzim: упорядкований перелік статей у X/listing.
        const LISTING: &str = "listing/titleOrdered/v1";
        if items.iter().any(|i| i.namespace == 'C') && !items.iter().any(|i| i.url == LISTING) {
            items.push(Item {
                namespace: 'X',
                url: LISTING.into(),
                title: String::new(),
                mime: "application/octet-stream+zimlisting".into(),
                data: Vec::new(),
            });
        }
        items.sort_by(|a, b| (a.namespace, &a.url).cmp(&(b.namespace, &b.url)));
        let mut listing: Vec<u32> = (0..items.len() as u32)
            .filter(|i| {
                let it = &items[*i as usize];
                it.namespace == 'C' && it.mime.starts_with("text/html")
            })
            .collect();
        let title_of = |it: &Item| {
            if it.title.is_empty() {
                it.url.clone()
            } else {
                it.title.clone()
            }
        };
        listing.sort_by_key(|i| title_of(&items[*i as usize]));
        if let Some(l) = items
            .iter_mut()
            .find(|i| i.namespace == 'X' && i.url == LISTING)
        {
            l.data = listing.iter().flat_map(|i| i.to_le_bytes()).collect();
        }
        let mut mimes: Vec<String> = items.iter().map(|i| i.mime.clone()).collect();
        mimes.sort();
        mimes.dedup();

        // Кластер: зміщення (u32) + блоби.
        let n = items.len();
        let mut cluster = Vec::new();
        let mut off = ((n + 1) * 4) as u32;
        for it in &items {
            cluster.extend_from_slice(&off.to_le_bytes());
            off += it.data.len() as u32;
        }
        cluster.extend_from_slice(&off.to_le_bytes());
        for it in &items {
            cluster.extend_from_slice(&it.data);
        }
        let compressed = ruzstd::encoding::compress_to_vec(
            &cluster[..],
            ruzstd::encoding::CompressionLevel::Fastest,
        );

        let mut mime_list = Vec::new();
        for m in &mimes {
            mime_list.extend_from_slice(m.as_bytes());
            mime_list.push(0);
        }
        mime_list.push(0);

        let mut dirents: Vec<Vec<u8>> = Vec::new();
        for (i, it) in items.iter().enumerate() {
            let mut d = Vec::new();
            let mi = mimes.iter().position(|m| *m == it.mime).unwrap() as u16;
            d.extend_from_slice(&mi.to_le_bytes());
            d.push(0);
            d.push(it.namespace as u8);
            d.extend_from_slice(&0u32.to_le_bytes());
            d.extend_from_slice(&0u32.to_le_bytes());
            d.extend_from_slice(&(i as u32).to_le_bytes());
            d.extend_from_slice(it.url.as_bytes());
            d.push(0);
            d.extend_from_slice(it.title.as_bytes());
            d.push(0);
            dirents.push(d);
        }

        let mime_pos = 80u64;
        let url_ptr_pos = mime_pos + mime_list.len() as u64;
        let title_ptr_pos = url_ptr_pos + 8 * n as u64;
        let cluster_ptr_pos = title_ptr_pos + 4 * n as u64;
        let dirent_pos = cluster_ptr_pos + 8;
        let mut pos = dirent_pos;
        let mut url_ptrs = Vec::new();
        for d in &dirents {
            url_ptrs.push(pos);
            pos += d.len() as u64;
        }
        let cluster_pos = pos;
        let checksum_pos = cluster_pos + 1 + compressed.len() as u64;

        let mut by_title: Vec<u32> = (0..n as u32).collect();
        by_title.sort_by(|a, b| {
            let (x, y) = (&items[*a as usize], &items[*b as usize]);
            (x.namespace, title_of(x)).cmp(&(y.namespace, title_of(y)))
        });
        let main = main_url
            .and_then(|u| items.iter().position(|i| i.url == u))
            .map(|p| p as u32)
            .unwrap_or(u32::MAX);

        let mut h = Vec::with_capacity(80);
        h.extend_from_slice(&super::MAGIC.to_le_bytes());
        h.extend_from_slice(&6u16.to_le_bytes());
        h.extend_from_slice(&1u16.to_le_bytes());
        h.extend_from_slice(&[0u8; 16]);
        h.extend_from_slice(&(n as u32).to_le_bytes());
        h.extend_from_slice(&1u32.to_le_bytes());
        h.extend_from_slice(&url_ptr_pos.to_le_bytes());
        h.extend_from_slice(&title_ptr_pos.to_le_bytes());
        h.extend_from_slice(&cluster_ptr_pos.to_le_bytes());
        h.extend_from_slice(&mime_pos.to_le_bytes());
        h.extend_from_slice(&main.to_le_bytes());
        h.extend_from_slice(&u32::MAX.to_le_bytes());
        h.extend_from_slice(&checksum_pos.to_le_bytes());
        out.write_all(&h)?;
        out.write_all(&mime_list)?;
        for p in url_ptrs {
            out.write_all(&p.to_le_bytes())?;
        }
        for t in by_title {
            out.write_all(&t.to_le_bytes())?;
        }
        out.write_all(&cluster_pos.to_le_bytes())?;
        for d in &dirents {
            out.write_all(d)?;
        }
        out.write_all(&[5u8])?;
        out.write_all(&compressed)?;
        out.write_all(&[0u8; 16])?; // місце під MD5 (не перевіряємо)
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::writer::{write_zim, Item};
    use super::*;

    fn item(ns: char, url: &str, title: &str, mime: &str, data: &str) -> Item {
        Item {
            namespace: ns,
            url: url.into(),
            title: title.into(),
            mime: mime.into(),
            data: data.as_bytes().to_vec(),
        }
    }

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.zim");
        let mut f = File::create(&path).unwrap();
        write_zim(
            vec![
                item(
                    'C',
                    "Вода",
                    "Очищення води",
                    "text/html",
                    "<p>Кип'ятіть воду</p>",
                ),
                item('C', "Main", "Головна", "text/html", "<h1>Головна</h1>"),
                item('C', "Опіки", "Опіки", "text/html", "<p>Охолодіть</p>"),
                item('M', "Title", "Title", "text/plain", "Тестовий довідник"),
            ],
            Some("Main"),
            &mut f,
        )
        .unwrap();
        drop(f);

        let z = Zim::open(&path).unwrap();
        assert_eq!(z.entry_count(), 5, "4 записи + перелік заголовків");
        assert_eq!(z.content_namespace(), 'C');
        assert_eq!(z.metadata("Title").unwrap(), "Тестовий довідник");
        let e = z.get("C/Вода").unwrap();
        let (mime, data) = z.content(&e).unwrap();
        assert_eq!(mime, "text/html");
        assert_eq!(String::from_utf8(data).unwrap(), "<p>Кип'ятіть воду</p>");
        assert_eq!(z.main_page().unwrap().url, "Main");
        let found = z.find_by_title_prefix('C', "Оч", 10);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].url, "Вода");
        assert_eq!(z.articles().count(), 3);
        assert!(Zim::open(dir.path().join("nope")).is_err());
    }

    #[test]
    fn title_search_uses_listing_when_header_has_none() {
        // Новіші libzim: порожні заголовки (= URL) і перелік у X/listing.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("n.zim");
        let mut f = File::create(&path).unwrap();
        write_zim(
            vec![
                item('C', "Кровотеча", "", "text/html", "<p>Тисніть</p>"),
                item('C', "Вода", "Вода питна", "text/html", "<p>Кип'ятіть</p>"),
                item('C', "logo.png", "", "image/png", "png"),
            ],
            None,
            &mut f,
        )
        .unwrap();
        drop(f);
        let mut b = std::fs::read(&path).unwrap();
        b[40..48].copy_from_slice(&u64::MAX.to_le_bytes());
        std::fs::write(&path, b).unwrap();
        let z = Zim::open(&path).unwrap();
        assert!(z.title_index_info().starts_with("titleOrdered/v1, 2"));
        let found = z.find_by_title_prefix('C', "Кров", 10);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Кровотеча");
        assert_eq!(z.find_by_title_prefix('C', "Вода", 10).len(), 1);
        assert!(z.find_by_title_prefix('C', "logo", 10).is_empty());
    }

    #[test]
    fn rejects_non_zim() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.zim");
        std::fs::write(&p, vec![1u8; 100]).unwrap();
        assert!(matches!(Zim::open(&p), Err(ZimError::NotZim)));
    }
}
