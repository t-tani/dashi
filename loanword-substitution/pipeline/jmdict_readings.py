"""JMdict の XML から、読みがカタカナの項目を読みで引く索引を JSON に書く。

見出しが漢字の項目(頁・釦)も読みで引けるようにするための索引である。項目ごとに語義
(`<sense>`)の一覧を持ち、語義ごとに英訳と、計算機分野のタグの有無を持つ。

語義の境界を保つのは、1 つの語義が複数の英訳を持つためである。`ティア` の語義の 1 つは
`tear` と `teardrop shape` の 2 つの英訳を持ち、これは涙の形という 1 つの意味を 2 通りの
英語で言ったものである。英訳を平らに並べると、この 2 つが別の意味になる。

英訳の括弧書きは落とす。同じ読みの項目が複数あれば、配列に並べる。
"""
import argparse
import json
import re
import xml.etree.ElementTree as ET

LANG = "{http://www.w3.org/XML/1998/namespace}lang"
KATAKANA = re.compile(r"^[ァ-ヴー]+$")


def core(gloss):
    """英訳から括弧書きを外し、英字と空白と省略符とハイフンだけの語なら小文字で返す。"""
    text = re.sub(r"\s*\([^)]*\)", "", gloss).strip().lower()
    return text if re.fullmatch(r"[a-z][a-z' -]*[a-z]", text) else None


def sense_glosses(sense):
    """1 つの `<sense>` の英訳を並び順のまま返す。括弧書きを外した結果が重なる英訳は 1 つにまとめる。

    `raw` と `raw (e.g. raw data)` は括弧を外すと同じ語になる。まとめないと同じ英訳が 2 回並ぶ。
    """
    glosses = []
    for gloss in sense.iter("gloss"):
        if gloss.get(LANG) not in (None, "eng") or not gloss.text:
            continue
        text = core(gloss.text)
        if text and text not in glosses:
            glosses.append(text)
    return glosses


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jmdict", required=True, help="JMdict_e.xml")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    index = {}
    for _, element in ET.iterparse(args.jmdict, events=("end",)):
        if element.tag != "entry":
            continue
        readings = [r.text for r in element.iter("reb") if r.text and KATAKANA.match(r.text)]
        if readings:
            senses, languages = [], set()
            for sense in element.iter("sense"):
                computing = any("comput" in (f.text or "") for f in sense.iter("field"))
                glosses = sense_glosses(sense)
                if glosses:
                    senses.append({"glosses": glosses, "computing": computing})
                for source in sense.iter("lsource"):
                    languages.add(source.get(LANG) or "eng")
            for reading in readings:
                index.setdefault(reading, []).append({"senses": senses, "lsource": sorted(languages)})
        element.clear()
    with open(args.out, "w", encoding="utf-8") as sink:
        json.dump(index, sink, ensure_ascii=False)
    print(f"readings {len(index)}")


if __name__ == "__main__":
    main()
