"""人の IT 文書の抽出を、日本語コーパスから文書の一覧どおりに写す。

一覧は、選んだ文書のソース・パス・写し先の名前・内容の SHA-256 を列に持つ TSV である。写す
ときに内容が一覧と同じかを確かめる。

再配布できないソースの文書は、名前を一覧に載せない。その行はパスが空で、ソースの平文の中から
内容の SHA-256 で文書を探す。写し先の名前は、学習に使った抽出での名前の並び順を保つ番号である。
写し先の名前は文書の並び(学習の行の順)を決めるので変えない。

使い方: uv run python pipeline/make_sample.py <文書の一覧> <日本語コーパスの data/> <写し先>
"""
import csv
import hashlib
import os
import sys


def sha256(body):
    return hashlib.sha256(body).hexdigest()


def by_digest(source_dir, wanted):
    """`source_dir` の下の文書のうち、内容の SHA-256 が `wanted` にあるものの本文。"""
    found = {}
    for root, _, files in os.walk(source_dir):
        for f in files:
            body = open(os.path.join(root, f), "rb").read()
            digest = sha256(body)
            if digest in wanted:
                found.setdefault(digest, body)
    return found


def main():
    listing, data, dst = sys.argv[1], sys.argv[2], sys.argv[3]
    rows = list(csv.DictReader(open(listing, encoding="utf-8"), delimiter="\t"))
    hidden = {}
    for source in {r["source"] for r in rows if not r["path"]}:
        wanted = {r["sha256"] for r in rows if r["source"] == source and not r["path"]}
        hidden.update(by_digest(os.path.join(data, source, "text"), wanted))
    for r in rows:
        if r["path"]:
            src = os.path.join(data, r["source"], "text", r["path"])
            body = open(src, "rb").read()
            if sha256(body) != r["sha256"]:
                raise SystemExit(f"{src} の内容が {listing} の SHA-256 と違う")
        elif r["sha256"] in hidden:
            body = hidden[r["sha256"]]
        else:
            raise SystemExit(f"{r['source']} に SHA-256 が {r['sha256']} の文書が無い")
        os.makedirs(os.path.join(dst, r["source"]), exist_ok=True)
        with open(os.path.join(dst, r["source"], r["name"]), "wb") as out:
            out.write(body)
    # corpus-tool の --text-dir は、manifest.tsv に並んだソースを、この順に読む。
    with open(os.path.join(dst, "manifest.tsv"), "w", encoding="utf-8") as m:
        m.write("name\trepo\tbranch\tcommit\tcommit_date\tpaths\tformat\tlicense\toutput_dir\n")
        for source in sorted({r["source"] for r in rows}):
            m.write(f"{source}\t-\t-\t-\t-\t-\ttext\t-\ttext\n")
    print(len(rows), "文書を写した")


if __name__ == "__main__":
    main()
