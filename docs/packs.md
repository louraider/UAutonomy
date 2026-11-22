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

Публічний сервер пакетів ще не розгорнуто: поки адресу каталогу вводять вручну.

## Карти

Застосунок показує растрові PMTiles через `flutter_map_pmtiles` і не ходить
по тайли в інтернет. Пакет області збирається так:

1. Вирізати область з OSM (Geofabrik `ukraine-latest.osm.pbf`) за `bbox` з `regions.rs`.
2. **Прибрати критичну інфраструктуру** (енергетика, військові об'єкти, водоканали тощо) до рендерингу.
3. Відрендерити тайли z5–z14 власним стилем (масові тайли з tile.openstreetmap.org брати заборонено правилами OSMF).
4. Запакувати у PMTiles (`pmtiles convert`), далі `uautonomy-hub pack --kind map --region <id>`.

Векторні PMTiles (менші за розміром) — наступний крок: зараз пакети
`flutter_map_pmtiles` і `vector_map_tiles_pmtiles` несумісні за залежностями.

## Моделі

Каталог моделей (`models.rs`) — JSON з вимогами до ОЗП. Застосунок
рекомендує модель за залізом (`hardware::recommend`): на телефонах із
3,5–6 ГБ ОЗП Gemma 4 E2B пропонується за бажанням, з 6 ГБ вмикається
за замовчуванням; ПК з 12 ГБ отримують E4B, з 32 ГБ — 26B A4B. MamayLM у
каталозі лише для порівняльних тестів. Будь-який `.gguf` / `.litertlm`
можна додати вручну на вкладці «Помічник».
