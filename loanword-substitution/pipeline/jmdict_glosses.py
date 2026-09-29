"""JMdict の XML から、見出し(漢字表記があればそれ、なければ読み)ごとの語義を JSON に書く。

語義(`<sense>`)ごとに英訳の一覧と、計算機分野のタグの有無を持つ。語義の境界を保つ理由は
jmdict_readings.py と同じで、1 つの語義が複数の英訳を持つためである。英訳は括弧書きを外す。
同じ見出しを持つ項目が複数あれば語義を並べる。カタカナの読みだけの項目は
jmdict_readings.py の索引が受け持つので、ここでは漢字とかなの見出しを対象にする。
"""
import argparse
import json
import xml.etree.ElementTree as ET

from jmdict_readings import sense_glosses


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jmdict", required=True, help="JMdict_e.xml")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    index = {}
    for _, element in ET.iterparse(args.jmdict, events=("end",)):
        if element.tag != "entry":
            continue
        headwords = [k.text for k in element.iter("keb") if k.text] or [r.text for r in element.iter("reb") if r.text]
        senses = []
        for sense in element.iter("sense"):
            computing = any("comput" in (f.text or "") for f in sense.iter("field"))
            glosses = sense_glosses(sense)
            if glosses:
                senses.append({"glosses": glosses, "computing": computing})
        for headword in headwords:
            known = index.setdefault(headword, [])
            for sense in senses:
                if sense not in known:
                    known.append(sense)
        element.clear()
    with open(args.out, "w", encoding="utf-8") as sink:
        json.dump(index, sink, ensure_ascii=False)
    print(f"headwords {len(index)}")


if __name__ == "__main__":
    main()
