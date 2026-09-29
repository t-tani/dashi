"""書き出した判定モデルの JSON から、出現の特徴の得点を出す。

モデルは LightGBM の 2 つのブースターで、得点はそれぞれの生の得点を人の抽出の平均と標準偏差で
標準化し、重みを付けて足したものである。木のたどり方は LightGBM と同じにする。akunuki の
検出器は同じたどり方を Rust で行う。
"""
import math

# LightGBM が 0 とみなす値の幅。
ZERO_THRESHOLD = 1e-35


def encode(features, model):
    """特徴の辞書を、モデルの列の順の数値の並びにする。カテゴリの値はモデルの水準の番号にし、
    水準にない値は欠損にする。"""
    out = []
    for name in model["features"]:
        v = features[name]
        levels = model["categorical"].get(name)
        if levels is not None:
            out.append(float(levels.index(str(v))) if str(v) in levels else math.nan)
        else:
            out.append(float(v))
    return out


def tree_value(node, x):
    while "leaf_value" not in node:
        v = x[node["split_feature"]]
        missing = node["missing_type"]
        if node["decision_type"] == "==":
            if math.isnan(v):
                if missing == "NaN":
                    node = node["right_child"]
                    continue
                v = 0.0
            code = int(v)
            if code < 0:
                node = node["right_child"]
                continue
            left = code in {int(c) for c in str(node["threshold"]).split("||")}
        else:
            if math.isnan(v) and missing != "NaN":
                v = 0.0
            if (missing == "Zero" and abs(v) <= ZERO_THRESHOLD) or (missing == "NaN" and math.isnan(v)):
                left = node["default_left"]
            else:
                left = v <= node["threshold"]
        node = node["left_child"] if left else node["right_child"]
    return node["leaf_value"]


def raw(booster, x):
    return sum(tree_value(t["tree_structure"], x) for t in booster["tree_info"])


def score(features, model):
    """出現 1 件の得点。線(`model["lines"]`)を超えたら指摘する。"""
    x = encode(features, model)
    return sum(b["weight"] * (raw(b, x) - b["mean"]) / b["std"] for b in model["boosters"])
