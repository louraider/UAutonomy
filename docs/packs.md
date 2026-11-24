# Пакети, каталог, карти й моделі

## Пакет

Пакет — тека з `manifest.json` і файлами. Кожен файл має розмір і SHA-256,
тому пакет можна докачувати по файлах і перевіряти після флешки.

```json
{
  "id": "pocket-uk", "tier": "pocket", "kind": "content",
  "title": "Кишеньковий набір", "version": "0.1.0",
  "content_license": "CC-BY-4.0",
  "files": [{"path": "cards/first_aid.json", "size": 1234, "sha256": "…"}]
}
```

| `kind` | Що всередині |
|---|---|
| `content` | `cards.json` або `cards/*.json`, `articles/**/*.md`, довідники `*.zim` |
| `map` | `*.pmtiles` (растрові тайли), поле `region` = id області з `regions.rs` |
| `model` | `*.gguf` або `*.litertlm`, поле `model_id` = id з каталогу моделей |

Один файл `.uapack` — це звичайний tar без стиснення, де першим іде
`manifest.json`. Його зручно носити на флешці; застосунок імпортує і теку, і `.uapack`.

## Інструменти (хаб)

```sh
cd core
cargo run -p uautonomy-hub -- pack    ../build/pocket --id pocket-uk --title "Кишеньковий" --tier pocket
cargo run -p uautonomy-hub -- export  ../build/pocket pocket-uk.uapack
cargo run -p uautonomy-hub -- catalog ../build --base-url https://example.org/packs
cargo run -p uautonomy-hub -- install --data /srv/autonomy pocket-uk.uapack
cargo run -p uautonomy-hub -- serve   --data /srv/autonomy --app avtonomiia.apk
```

## Каталог

`catalog.json` = `{ "base_url": "…", "packs": [маніфести] }`. Файл пакета
лежить за адресою `{base_url}/{id}/{path}`. Якщо `base_url` порожній, береться
тека, де лежить `catalog.json`. Кожен телефон чи хаб з увімкненим «Обміном»
віддає `/packs/catalog.json` зі своїми пакетами, тож пакети розходяться
телефон-телефон без інтернету.

Офіційний каталог — випуск `packs` у GitHub Releases проєкту
(`https://github.com/louraider/UAutonomy/releases/download/packs/catalog.json`),
його застосунок відкриває за замовчуванням. GitHub Releases безкоштовні й
підтримують докачку; межа — 2 ГБ на файл, тому моделі лежать на Hugging Face,
а в каталозі для них є пряма адреса (`files[].url`) і SHA-256 з LFS.

## Карти

Карти векторні: PMTiles зі схемою OpenMapTiles. Ядро читає тайли
(`pmtiles.rs`), застосунок малює їх через `vector_map_tiles`, інтернет-тайлів
немає. Пакети збирає `tools/maps/build_maps.sh` у GitHub Actions
(workflow «Пакети»):

1. OSM України з Geofabrik.
2. Карти повні, як у відкритому OSM: дані й так публічні, тому нічого не
   прибираємо. Фільтр `tools/maps/exclude-tags.txt` лишився необов'язковим
   (`FILTER_INFRA=1`, у workflow — галочка `filter_infra`).
3. Вирізати область за `bbox` з `regions.rs` (`osmium extract`).
4. `tilemaker` → `.pmtiles` z0–z14. Моря немає (шари з shapefile вимкнено), лише дані OSM.
5. `uautonomy-hub pack --kind map --region <id>`.

### Карти в браузері (iPhone, ноутбук)

Сервер Wi-Fi-обміну віддає `/map`: сторінку з MapLibre GL JS, стилем
OpenMapTiles (`webmap.rs`), тайлами `/map/tiles/<область>/{z}/{x}/{y}.pbf`
і гліфами Noto Sans (латиниця й кирилиця). Усе вбудовано в ядро, тож і
роздавач, і той, хто дивиться, обходяться без інтернету. tilemaker не
записує межі в заголовок PMTiles і кладе назви в `name:latin`, тому межі
беремо з області, а підписи з `name:latin`.

## Енциклопедії Kiwix (ZIM)

Списки енциклопедій лежать у `tools/packs/kiwix.json`. Workflow «Пакети»
запускає `tools/packs/kiwix_pack.py`, який бере найновіший файл кожної з
download.kiwix.org, а розмір і SHA-256 читає з `.meta4`. Файли ZIM ми не
копіюємо: у каталозі для них прямі посилання (Kiwix сам перенаправляє на
дзеркала, докачка працює). Файли до 700 МБ workflow завантажує повністю,
звіряє SHA-256 і відкриває нашим читачем (`uautonomy-hub zim-check`).
Якщо якоїсь назви на Kiwix немає, запис пропускається, а в журналі
з'являються схожі назви.

Бібліотеки zimgit (water, medicine, post-disaster) у каталог не входять: у
них дві HTML-сторінки, решта — PDF, а наш пошук і ШІ поки читають лише HTML.

## Моделі

Каталог моделей (`models.rs`) — JSON з вимогами до ОЗП. Застосунок
рекомендує модель за залізом (`hardware::recommend`): на телефонах із
3,5–6 ГБ ОЗП Gemma 4 E2B пропонується за бажанням, з 6 ГБ вмикається
за замовчуванням; ПК з 12 ГБ отримують E4B, з 32 ГБ — 26B A4B. MamayLM у
каталозі лише для порівняльних тестів. Будь-який `.gguf` / `.litertlm`
можна додати вручну на вкладці «Помічник».
