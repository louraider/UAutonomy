#!/usr/bin/env bash
# Завантажує свіжий OSM-витяг України у <out.osm.pbf>.
#
#   tools/maps/fetch_ukraine.sh work/ukraine.osm.pbf
#
# 1. Geofabrik (один файл ~1,5 ГБ). Сервер іноді відмовляє раннерам GitHub
#    у з'єднанні, тож пробуємо кілька разів із паузами.
# 2. Дзеркало openstreetmap.fr. Цілого файлу України там немає
#    (europe/ukraine-latest.osm.pbf — заглушка, 404), зате є всі області
#    окремо в europe/ukraine/. Качаємо їх, звіряємо MD5 і зливаємо
#    `osmium merge` (спільні об'єкти на межах областей не дублюються).
# OSM_MIRROR_ONLY=1 — одразу п. 2 (так PR перевіряє резервний шлях).
# Потрібні: curl, osmium-tool, md5sum.
set -euo pipefail
OUT=$1
GEOFABRIK=https://download.geofabrik.de/europe/ukraine-latest.osm.pbf
OSMFR=https://download.openstreetmap.fr/extracts/europe/ukraine
mkdir -p "$(dirname "$OUT")"

for attempt in 1 2 3; do
  [ "${OSM_MIRROR_ONLY:-0}" = "1" ] && break
  if curl -fL --retry 4 --retry-delay 20 --retry-all-errors --connect-timeout 30 -o "$OUT" "$GEOFABRIK"; then
    echo "Geofabrik: $(du -h "$OUT" | cut -f1)"
    exit 0
  fi
  rm -f "$OUT"
  echo "::warning::Geofabrik недоступний (спроба $attempt)"
  if [ "$attempt" -lt 3 ]; then sleep 120; fi
done

echo "Резерв: області з openstreetmap.fr"
PARTS=$(mktemp -d)
trap 'rm -rf "$PARTS"' EXIT
# Беремо файли «<назва>.osm.pbf» (без -latest — це ті самі файли).
# Однакові об'єкти з різних файлів (межі, місто Київ) merge пише один раз.
# Області оновлюються в різні дні, тож на межі може трапитися той самий
# об'єкт різних версій: зливаємо як історію і беремо зріз «зараз»
# (osmium time-filter лишає по одній, найновішій версії).
names=$(curl -fsSL --retry 4 --retry-all-errors "$OSMFR/" |
  grep -oE 'href="[a-z_-]+\.osm\.pbf"' | sed -E 's/href="(.*)"/\1/' |
  grep -v -- '-latest\.osm\.pbf$' | sort -u)
count=$(echo "$names" | grep -c . || true)
if [ "$count" -lt 25 ]; then
  echo "::error::На openstreetmap.fr знайдено лише $count файлів областей: $names"
  exit 1
fi
for f in $names; do
  curl -fL --retry 6 --retry-delay 15 --retry-all-errors -s -o "$PARTS/$f" "$OSMFR/$f"
  want=$(curl -fsSL --retry 4 --retry-all-errors "$OSMFR/$f.md5" | cut -d' ' -f1)
  got=$(md5sum "$PARTS/$f" | cut -d' ' -f1)
  if [ "$want" != "$got" ]; then
    echo "::error::$f: MD5 не збігається ($got замість $want)"
    exit 1
  fi
  echo "$f: $(du -h "$PARTS/$f" | cut -f1)"
done
osmium merge --overwrite "$PARTS"/*.osm.pbf -o "$PARTS/merged.osh.pbf"
osmium time-filter --overwrite "$PARTS/merged.osh.pbf" -o "$OUT"
osmium fileinfo "$OUT" | head -20
