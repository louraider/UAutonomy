#!/usr/bin/env python3
"""Пакети моделей, що лежать на Hugging Face (ваги не копіюємо: застосунок
качає їх напряму з докачкою, а SHA-256 береться з LFS).

    python3 tools/packs/hf_model_pack.py <models.json> <тека-пакетів> [id,id… | all]

Для кожної моделі створює теку `<тека-пакетів>/model-<id>/` з manifest.json.
Файл моделі в маніфесті має адресу Hugging Face і в теці не лежить; файли з
`attach` (NOTICE, умови ліцензії) копіюються в теку і публікуються разом з
пакетом, тож вони є в кожній копії моделі, зокрема переданій через «Обмін».
"""
import hashlib
import json
import shutil
import sys
import urllib.parse
import urllib.request
from pathlib import Path


def hf_entry(repo: str, fname: str) -> dict:
    api = f"https://huggingface.co/api/models/{repo}/tree/main"
    dirname = fname.rsplit("/", 1)[0] if "/" in fname else ""
    if dirname:
        api += "/" + urllib.parse.quote(dirname)
    with urllib.request.urlopen(api, timeout=60) as r:
        tree = json.load(r)
    entry = next((e for e in tree if e.get("path") == fname), None)
    if entry is None or "lfs" not in entry:
        names = [e.get("path") for e in tree if str(e.get("path", "")).endswith((".gguf", ".litertlm"))]
        sys.exit(f"Файл {fname} не знайдено в {repo}. Є: {names}")
    return entry


def build(m: dict, out: Path) -> None:
    entry = hf_entry(m["repo"], m["file"])
    lfs = entry["lfs"]
    if m.get("sha256") and m["sha256"] != lfs["oid"]:
        sys.exit(f"{m['repo']}/{m['file']}: SHA-256 {lfs['oid']} не збігається з перевіреним {m['sha256']}")
    url = f"https://huggingface.co/{m['repo']}/resolve/main/{urllib.parse.quote(m['file'])}"
    pack_id = "model-" + m["model_id"]
    folder = out / pack_id
    folder.mkdir(parents=True, exist_ok=True)
    files = [{"path": m["file"].rsplit("/", 1)[-1], "size": lfs["size"], "sha256": lfs["oid"], "url": url}]
    if m.get("attach"):
        for src in sorted(Path(m["attach"]).iterdir()):
            if not src.is_file():
                continue
            shutil.copyfile(src, folder / src.name)
            data = src.read_bytes()
            files.append({"path": src.name, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    pack = {
        "id": pack_id,
        "tier": "pocket",
        "kind": "model",
        "title": m["title"],
        "description": m.get("description") or f"Модель ШІ для роботи без інтернету ({m['repo']}).",
        "version": entry.get("oid", "1")[:12],
        "content_license": m.get("license", "Apache-2.0"),
        "model_id": m["model_id"],
        "files": files,
    }
    (folder / "manifest.json").write_text(json.dumps(pack, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"{pack_id}: {m['repo']}/{m['file']} {lfs['size'] / 1e9:.2f} ГБ, файлів {len(files)}", file=sys.stderr)


def main() -> None:
    cfg, out = sys.argv[1], Path(sys.argv[2])
    want = sys.argv[3] if len(sys.argv) > 3 else "all"
    models = json.loads(Path(cfg).read_text(encoding="utf-8"))["models"]
    if want not in ("", "all"):
        ids = {w.strip() for w in want.split(",") if w.strip()}
        unknown = ids - {m["model_id"] for m in models}
        if unknown:
            sys.exit(f"Невідомі моделі: {sorted(unknown)}")
        models = [m for m in models if m["model_id"] in ids]
    for m in models:
        build(m, out)


if __name__ == "__main__":
    main()
