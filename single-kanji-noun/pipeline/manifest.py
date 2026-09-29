"""配布する成果物の来歴を manifest.json に書く。

成果物は `--release` のディレクトリに、リリースのアセットの名前で置く。成果物ごとのバイト数と SHA-256 と、モデルの線を載せる。回数を数えた入力の中身は載せない。
akunuki は、取得した成果物をこの SHA-256 と突き合わせる。

使い方: uv run python pipeline/manifest.py --version <版> --akunuki-rev <akunuki の commit> \
            --release <成果物のディレクトリ> --out manifest.json
"""
import argparse
import hashlib
import json
import os

ARTIFACTS = [f"single-kanji-noun-{name}" for name in (
    "model.json", "novelty.tsv.zst", "targets.tsv", "case-frame-counts.tsv", "noun-modifier-counts.tsv")]


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    for name in ("version", "akunuki-rev", "release", "out"):
        ap.add_argument(f"--{name}", required=True)
    a = ap.parse_args()
    model = json.load(open(os.path.join(a.release, ARTIFACTS[0]), encoding="utf-8"))
    files = []
    for name in ARTIFACTS:
        body = open(os.path.join(a.release, name), "rb").read()
        files.append(dict(name=name, bytes=len(body), sha256=hashlib.sha256(body).hexdigest()))
    out = dict(
        artifact="single-kanji-noun", version=a.version, akunuki=dict(rev=a.akunuki_rev),
        lines=model["lines"], files=files,
    )
    with open(a.out, "w", encoding="utf-8") as w:
        json.dump(out, w, ensure_ascii=False, indent=2)
        w.write("\n")


if __name__ == "__main__":
    main()
