#!/usr/bin/env python3
"""Маніфести пакетів-енциклопедій з Kiwix (ZIM). Файли не копіюємо:
застосунок качає їх напряму з download.kiwix.org (дзеркала, докачка),
розмір і SHA-256 беруться з metalink-файлу `.meta4` поруч.

    python3 tools/packs/kiwix_pack.py tools/packs/kiwix.json > zims.json

Для кожного запису з конфігурації береться найновіший файл
`<prefix>_YYYY-MM.zim` у теці `<dir>`. Відсутні записи пропускаються
з попередженням (і переліком схожих файлів), щоб назви легко виправити.
"""
import json
import re
import sys
import urllib.request

BASE = "https://download.kiwix.org/zim"
UA = {"User-Agent": "uautonomy-packs/1"}


def get(url, method="GET"):
    req = urllib.request.Request(url, headers=UA, method=method)
    return urllib.request.urlopen(req, timeout=120)


listings = {}


def listing(d):
    if d not in listings:
        with get(f"{BASE}/{d}/") as r:
            listings[d] = sorted(set(re.findall(r'href="([^"/?]+\.zim)"', r.read().decode())))
    return listings[d]


out = []
for c in json.load(open(sys.argv[1], encoding="utf-8")):
    files = [f for f in listing(c["dir"]) if re.fullmatch(re.escape(c["prefix"]) + r"_\d{4}-\d{2}\.zim", f)]
    if not files:
        stem = c["prefix"].split("_")[0] + "_" + (c["prefix"].split("_")[1] if "_" in c["prefix"] else "")
        similar = [f for f in listing(c["dir"]) if f.startswith(stem)][-15:]
        print(f"ПРОПУСК {c['id']}: немає {c['dir']}/{c['prefix']}_*.zim. Схожі: {similar}", file=sys.stderr)
        continue
    name = files[-1]
    url = f"{BASE}/{c['dir']}/{name}"
    # Metalink поруч із файлом містить і розмір, і SHA-256.
    with get(url + ".meta4") as r:
        meta = r.read().decode()
    size = int(re.search(r"<size>(\d+)</size>", meta).group(1))
    m = re.search(r'<hash type="sha-256">([0-9a-fA-F]{64})</hash>', meta)
    if m:
        sha = m.group(1).lower()
    else:
        with get(url + ".sha256") as r:
            sha = r.read().decode().split()[0].lower()
    if not re.fullmatch(r"[0-9a-f]{64}", sha):
        print(f"ПРОПУСК {c['id']}: дивний SHA-256 {sha!r}", file=sys.stderr)
        continue
    version = re.search(r"_(\d{4}-\d{2})\.zim$", name).group(1).replace("-", ".")
    out.append({
        "id": c["id"],
        "tier": c["tier"],
        "kind": "content",
        "title": c["title"],
        "description": c.get("description", f"Офлайн-енциклопедія Kiwix ({name})."),
        "version": version,
        "content_license": c["license"],
        "files": [{"path": name, "size": size, "sha256": sha, "url": url}],
    })
    print(f"{c['id']}: {name}, {size / 1e9:.2f} ГБ", file=sys.stderr)

json.dump(out, sys.stdout, ensure_ascii=False, indent=1)
