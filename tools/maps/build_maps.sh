#!/usr/bin/env bash
# Збирає пакети офлайн-карт областей з OpenStreetMap.
#
#   tools/maps/build_maps.sh <ukraine.osm.pbf> <hub> <regions.json> <ids|all> <out-dir>
#
# Кроки: прибрати критичну інфраструктуру (exclude-tags.txt) → вирізати
# область → tilemaker (схема OpenMapTiles) → PMTiles → маніфест пакета.
# Потрібні: osmium-tool, docker (образ tilemaker), python3.
set -euo pipefail
PBF=$1 HUB=$2 REGIONS=$3 IDS=$4 OUT=$5
HERE=$(cd "$(dirname "$0")" && pwd)
WORK=${WORK:-$(mktemp -d)}
TILEMAKER_IMAGE=${TILEMAKER_IMAGE:-ghcr.io/systemed/tilemaker:master}
mkdir -p "$OUT" "$WORK"

echo "::group::Фільтр критичної інфраструктури"
grep -vE '^\s*(#|$)' "$HERE/exclude-tags.txt" > "$WORK/exclude.txt"
osmium tags-filter --invert-match --overwrite -e "$WORK/exclude.txt" "$PBF" -o "$WORK/filtered.osm.pbf"
osmium fileinfo -e "$WORK/filtered.osm.pbf" | grep -E 'Number of (nodes|ways|relations)' || true
echo "::endgroup::"

# Конфіг tilemaker (схема OpenMapTiles) з репозиторію tilemaker, без шарів
# із shapefile (океан, Natural Earth): у пакеті лише дані OSM.
TM_RAW=${TM_RAW:-https://raw.githubusercontent.com/systemed/tilemaker/master/resources}
curl -fsSL "$TM_RAW/config-openmaptiles.json" -o "$WORK/config-src.json"
curl -fsSL "$TM_RAW/process-openmaptiles.lua" -o "$WORK/process.lua"
python3 - "$WORK/config-src.json" "$WORK/config.json" <<'PY'
import json, sys
c = json.load(open(sys.argv[1]))
c["layers"] = {k: v for k, v in c["layers"].items() if "source" not in v}
c["settings"]["name"] = "Автономія"
c["settings"]["description"] = "OpenStreetMap без критичної інфраструктури"
json.dump(c, open(sys.argv[2], "w"), ensure_ascii=False, indent=1)
PY

python3 - "$REGIONS" "$IDS" > "$WORK/selected.tsv" <<'PY'
import json, sys
regions = json.load(open(sys.argv[1]))
want = None if sys.argv[2] == "all" else set(sys.argv[2].split(","))
for r in regions:
    if want is None or r["id"] in want:
        print("\t".join([r["id"], r["title"], ",".join(str(v) for v in r["bbox"])]))
PY
[ -s "$WORK/selected.tsv" ] || { echo "Не вибрано жодної області: $IDS" >&2; exit 1; }

DATE=$(date -u +%Y.%m.%d)
while IFS=$'\t' read -r id title bbox; do
  echo "::group::$title ($id)"
  osmium extract --overwrite -s smart -b "$bbox" "$WORK/filtered.osm.pbf" -o "$WORK/$id.osm.pbf"
  docker run --rm -v "$WORK:/data" "$TILEMAKER_IMAGE" \
    --input "/data/$id.osm.pbf" --output "/data/$id.pmtiles" \
    --config /data/config.json --process /data/process.lua
  dir="$OUT/map-$id"
  rm -rf "$dir" && mkdir -p "$dir"
  mv "$WORK/$id.pmtiles" "$dir/$id.pmtiles"
  rm -f "$WORK/$id.osm.pbf"
  "$HUB" pack "$dir" --id "map-$id" --kind map --tier family --region "$id" \
    --title "Карта: $title" --version "$DATE" --license "ODbL-1.0" \
    --description "OpenStreetMap, без критичної інфраструктури. © учасники OpenStreetMap"
  echo "::endgroup::"
done < "$WORK/selected.tsv"
ls -la "$OUT"/*/
