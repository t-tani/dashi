# 判定済み 140 件の見立て。番号は sample-new.txt / sample-old.txt /
# sample-both.txt に印字した通し番号である。見立ては自動の判定(LLM)であり、
# 人手の確認を経ていない。
# ok = 妥当、ng = 誤検出、na = 表の当否を測れない(語の部門は正しく、
# 段落に抽象的な語が多いという判定のほうがずれている)。

NEW_OK = {2, 3, 19, 20, 24, 35, 37, 38, 41, 45, 52, 55, 56, 58}
NEW_NA = {13, 27, 43, 57}

OLD_NG = {5, 48}
OLD_NA = {4, 18, 28, 30, 50, 51}

BOTH_OK = {1, 7, 8, 9, 10, 12, 13, 14, 15, 17, 19, 20}
BOTH_NA = {2, 3, 4, 5, 6, 11, 16, 18}


def verdict(side, index):
    if side == "new":
        if index in NEW_OK:
            return "ok"
        return "na" if index in NEW_NA else "ng"
    if side == "old":
        if index in OLD_NA:
            return "na"
        return "ng" if index in OLD_NG else "ok"
    if index in BOTH_OK:
        return "ok"
    return "na" if index in BOTH_NA else "ng"


# Haiku 表だけが出す指摘 30 件の見立て(sample-haiku-fresh.txt の通し番号)。
FRESH_OK = {2, 3, 5, 8, 13, 17, 18, 19, 20, 23, 24, 25, 27}
FRESH_NA = {14}


def fresh_verdict(index):
    if index in FRESH_OK:
        return "ok"
    return "na" if index in FRESH_NA else "ng"
