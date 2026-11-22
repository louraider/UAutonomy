#!/usr/bin/env python3
"""Маніфест пакета моделі, що лежить на Hugging Face (файл не копіюємо:
застосунок качає його напряму з докачкою, а SHA-256 береться з LFS).

    python3 tools/packs/hf_model_pack.py <repo> <file> <model_id> <title> > model.json
"""
import json
import sys
import urllib.parse
import urllib.request

repo, fname, model_id, title = sys.argv[1:5]
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
lfs = entry["lfs"]
url = f"https://huggingface.co/{repo}/resolve/main/{urllib.parse.quote(fname)}"
pack = {
    "id": "model-" + model_id,
    "tier": "pocket",
    "kind": "model",
    "title": title,
    "description": f"Модель ШІ для роботи без інтернету ({repo}).",
    "version": entry.get("oid", "1")[:12],
    "content_license": "Apache-2.0",
    "model_id": model_id,
    "files": [{"path": fname.rsplit("/", 1)[-1], "size": lfs["size"], "sha256": lfs["oid"], "url": url}],
}
json.dump([pack], sys.stdout, ensure_ascii=False, indent=1)
print(f"{fname}: {lfs['size'] / 1e9:.2f} ГБ", file=sys.stderr)
