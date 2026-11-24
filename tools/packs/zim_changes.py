#!/usr/bin/env python3
"""Чи є нові версії енциклопедій порівняно з опублікованим каталогом.

    python3 tools/packs/zim_changes.py <zims.json> [<catalog.json>]

Друкує зміни і пише `changed=true|false` у $GITHUB_OUTPUT (якщо задано).
Без попереднього каталогу — зміни є. Пакет, який kiwix_pack.py зараз
пропустив (мережа, перейменування), зміною не вважається: у каталозі
лишається попередня версія.
"""
import json
import os
import sys

new = json.load(open(sys.argv[1], encoding="utf-8"))
prev = {}
if len(sys.argv) > 2 and os.path.exists(sys.argv[2]):
    with open(sys.argv[2], encoding="utf-8") as f:
        prev = {p["id"]: p for p in json.load(f).get("packs", [])}

changes = []
for p in new:
    old = prev.get(p["id"])
    sha = p["files"][0]["sha256"]
    if old is None:
        changes.append(f"{p['id']}: новий ({p['files'][0]['path']})")
    elif [f.get("sha256") for f in old.get("files", [])] != [sha]:
        was = old["files"][0]["path"] if old.get("files") else "?"
        changes.append(f"{p['id']}: {was} → {p['files'][0]['path']}")

for c in changes:
    print(c)
if not changes:
    print("Нових версій енциклопедій немає.")
if not new:
    print("::warning::Kiwix не дав жодного пакета: перевірте tools/packs/kiwix.json")
out = os.environ.get("GITHUB_OUTPUT")
if out:
    with open(out, "a") as f:
        f.write(f"changed={'true' if changes else 'false'}\n")
