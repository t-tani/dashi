"""JMdict の XML から、分類にかける見出しの一覧を書き出す。

採る品詞は分類対象の定義(target.py の is_target_pos)が決め、残りの絞りは
jmdict.py にある。`--exclude` は、既に判定のある語の一覧を除くために使う。
"""

import argparse

import jmdict
import load_target


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, help="分類対象のディレクトリ")
    parser.add_argument("--jmdict", required=True)
    parser.add_argument("--out")
    parser.add_argument("--exclude", help="この一覧にある語を除く")
    parser.add_argument("--drop-names", action="store_true")
    parser.add_argument("--priority", action="store_true")
    args = parser.parse_args()

    target = load_target.load(args.target)
    words, total = jmdict.collect(
        args.jmdict, target.is_target_pos, args.drop_names, args.priority
    )
    print(f"JMdict の項目 {total} / 採った見出し {len(words)}")
    if args.exclude:
        known = {
            w for w in open(args.exclude, encoding="utf-8").read().split("\n") if w
        }
        words = [w for w in words if w not in known]
        print(f"既知の {len(known)} 語を除いた残り {len(words)}")
    if args.out:
        with open(args.out, "w", encoding="utf-8") as sink:
            for word in sorted(words):
                print(word, file=sink)
        print(f"書き出した: {args.out}")


if __name__ == "__main__":
    main()
