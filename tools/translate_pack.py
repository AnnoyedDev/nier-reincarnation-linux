"""Builds a French language pack (internal/translation, format 1) from the
game's English text, with a local LLM through Ollama.

  go run ./cmd/text-export --data <game>/data > en.json      (in server/)
  python tools/translate_pack.py en.json fr.json [--model mistral-small] [--limit N]
  copy fr.json to <game>/data/assets/translation/active.json, text language English

Tags (<TAP>, <i>, {0}...), literal \\n and numbers are swapped for ⟦n⟧ markers
before translation and put back after, so "Attack up by 7.4%" and "Attack up by
9.9%" are one request. A translation that loses or adds a marker is retried
alone, then left out (the game shows the English). Finished translations are
kept in <out>.cache.jsonl: the script can be stopped and started again.
"""
import argparse
import json
import re
import sys
import time
import urllib.request
from collections import Counter
from pathlib import Path

PROTECT = re.compile(r"<[^<>]{1,40}>|\{\d+\}|%[sd]|\\n|\d+(?:\.\d+)?")
MARK = re.compile(r"⟦(\d+)⟧")
BATCH = 20
SYSTEM = """Tu traduis en français le jeu mobile NieR Re[in]carnation (SQUARE ENIX), un jeu au ton poétique et mélancolique.
Règles :
- Réponds uniquement avec un objet JSON qui a exactement les mêmes clés, chaque valeur étant la traduction française.
- Garde chaque marqueur ⟦n⟧ tel quel, le même nombre de fois ; tu peux le déplacer si la phrase française l'exige.
- Garde les noms propres anglais (Mama, Fio, Levania, Dark Lord...), mais traduis les noms communs (Cage -> Cage, Weapon -> Arme, Memoir -> Souvenir, Costume -> Costume).
- Style naturel et concis, vouvoiement pour les menus, registre des personnages respecté dans les dialogues.
- "-" ou un texte vide reste identique."""


def protect(text):
    parts = []

    def sub(m):
        parts.append(m.group(0))
        return f"⟦{len(parts) - 1}⟧"

    return PROTECT.sub(sub, text), parts


def restore(template, parts):
    return MARK.sub(lambda m: parts[int(m.group(1))], template)


def valid(src, out):
    return isinstance(out, str) and out.strip() != "" and "\n" not in out and "\r" not in out \
        and Counter(MARK.findall(src)) == Counter(MARK.findall(out))


def ask(model, items):
    body = {"model": model, "stream": False, "format": "json", "options": {"temperature": 0.2, "num_ctx": 8192},
            "messages": [{"role": "system", "content": SYSTEM},
                         {"role": "user", "content": json.dumps(items, ensure_ascii=False)}]}
    req = urllib.request.Request("http://127.0.0.1:11434/api/chat", json.dumps(body).encode(),
                                 {"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=600) as r:
        reply = json.loads(r.read())["message"]["content"]
    try:
        out = json.loads(reply)
    except json.JSONDecodeError:
        return {}
    return out if isinstance(out, dict) else {}


def story(key):
    return re.match(r"^[A-Z]ID_", key) is not None


def main():
    p = argparse.ArgumentParser()
    p.add_argument("source", type=Path)
    p.add_argument("out", type=Path)
    p.add_argument("--model", default="mistral-small")
    p.add_argument("--limit", type=int, default=0, help="translate at most N new templates (a trial)")
    p.add_argument("--build-only", action="store_true", help="only write the pack from the cache, no model calls")
    a = p.parse_args()

    english = json.loads(a.source.read_text(encoding="utf-8"))
    # Menus first, then the story in scene order (keys sort by scene), so a batch is one scene.
    keys = sorted(english, key=lambda k: (story(k), k))
    templates, order = {}, []
    for k in keys:
        t, _ = protect(english[k])
        if t not in templates:
            templates[t] = None
            order.append(t)

    cache_path = a.out.with_suffix(a.out.suffix + ".cache.jsonl")
    done = {}
    if cache_path.exists():
        for line in cache_path.read_text(encoding="utf-8").splitlines():
            row = json.loads(line)
            done[row["en"]] = row["fr"]
    todo = [t for t in order if t not in done and t.strip() not in ("", "-") and re.search(r"[A-Za-z]", MARK.sub("", t))]
    if a.limit:
        todo = todo[:a.limit]
    if a.build_only:
        todo = []
    print(f"{len(order)} templates, {len(done)} cached, {len(todo)} to translate", file=sys.stderr)

    start, n = time.time(), 0
    with cache_path.open("a", encoding="utf-8") as cache:
        for i in range(0, len(todo), BATCH):
            batch = {str(j): t for j, t in enumerate(todo[i:i + BATCH])}
            out = ask(a.model, batch)
            for j, src in batch.items():
                fr = out.get(j)
                if not valid(src, fr):
                    fr = ask(a.model, {"0": src}).get("0")  # alone, once
                if valid(src, fr):
                    done[src] = fr
                    cache.write(json.dumps({"en": src, "fr": fr}, ensure_ascii=False) + "\n")
            cache.flush()
            n += len(batch)
            rate = n / (time.time() - start)
            print(f"\r{n}/{len(todo)}  {rate:.1f}/s  ~{(len(todo) - n) / rate / 3600:.1f} h left ", end="", file=sys.stderr)
    print(file=sys.stderr)

    game = {}
    for k, v in english.items():
        t, parts = protect(v)
        if t in done:
            fr = restore(done[t], parts)
            if fr != v:
                game[k] = fr
    pack = {"format": 1, "language": "fr", "name": "Français", "version": time.strftime("%Y%m%d"),
            "game": game}
    a.out.write_text(json.dumps(pack, ensure_ascii=False), encoding="utf-8")
    print(f"{len(game)} of {len(english)} keys translated -> {a.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
