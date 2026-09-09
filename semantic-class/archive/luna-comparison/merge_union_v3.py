# Haiku の表と luna の表を、語ごとに中項目の和集合で合併する。
# 片方にしかない語は、その片方の分類をそのまま採る。
import os
import datetime
import json

WORK = os.environ["WORK_DIR"]


def main():
    haiku = json.load(open(WORK + "haiku-v2-classifications.json", encoding="utf-8"))
    luna = json.load(open(WORK + "luna-v3-classifications.json", encoding="utf-8"))
    merged = {}
    for word in set(haiku) | set(luna):
        merged[word] = sorted(set(haiku.get(word, [])) | set(luna.get(word, [])))
    both = set(haiku) & set(luna)
    same = sum(1 for w in both if set(haiku[w]) == set(luna[w]))
    print(f"Haiku {len(haiku)} 語 / luna {len(luna)} 語 / 合併 {len(merged)} 語")
    print(f"両方に載る {len(both)} 語 / 中項目の集合が一致 {same} 語 ({same / len(both):.1%})")
    grew = sum(1 for w in both if len(merged[w]) > max(len(haiku[w]), len(luna[w])))
    print(f"合併で語義が両方より増えた語 {grew}")

    with open(WORK + "union-v3-classifications.json", "w", encoding="utf-8") as sink:
        json.dump(merged, sink, ensure_ascii=False, indent=1)

    today = datetime.date.today().isoformat()
    header = [
        "# 意味分類表(見出し語と分類番号の対応)。検証用に生成した表である。",
        "# 2 つの生成物を語ごとに中項目の和集合で合併したものである。片方にしか",
        "# 載らない語は、その片方の分類をそのまま採った。",
        "#",
        "# 合併の元は 2 つある。第 1 は claude-haiku-4-5-20251001 が 43 中項目へ",
        "# 分類した表、第 2 は gpt-5.6-luna が同じ 43 中項目へ分類した表である。",
        "# どちらも同じ生成の方針・同じ few-shot 26 語・同じ用例で作った。",
        "#",
        "# 生成の方針は次のとおりである。語義として挙げるのは、現代の日本語で日常的に",
        "# 使われると広く認められるものだけである。比喩や転用に由来する語義も、辞書に",
        "# 載る程度に定着していれば挙げる。攻撃の対象を指す「標的」がこれに当たる。",
        "# 挙げないのは 3 つで、まだ定着していない直訳や転用、特定の文脈や界隈でしか",
        "# 通じない用法、文学的・修辞的な言い回しにだけ現れる語義である。迷ったら",
        "# 挙げない側に倒す。",
        "#",
        "# 分類番号は 1.mm00 の形で、mm が中項目の 2 桁である。小数点以下第 3 位と",
        "# 第 4 位は 0 で埋めてあり、意味を持たない。",
        f"# 生成した日は {today} である。",
        "# 対象は、評価コーパスに現れた見出し語だけである。",
        "# 版: union-mid-3",
    ]
    with open(WORK + "union-v3-semantic-class-table.tsv", "w", encoding="utf-8") as sink:
        for line in header:
            print(line, file=sink)
        for word in sorted(merged):
            numbers = "\t".join(f"1.{c}00" for c in merged[word])
            print(f"{word}\t{numbers}", file=sink)
    print("表を書き出した")


if __name__ == "__main__":
    main()
