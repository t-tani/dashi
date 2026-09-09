# 判定基準 v2 での見立て。基準は人手で定めた。
#
# 妥当とするのは、定着していない転用や直訳で、現代の技術文書の読み手が
# 引っかかる使い方だけである。現代の技術文書で定着した語義・術語は、語の
# 分類がどうであれ誤検出と数える。定着の目安は、現行の辞書がその語義を
# 載せているか、実務の文書で普通に使われるかである。
#
# v1 にあった「表の当否を測れない」の区分は無くした。語の分類が正しくても、
# 読み手が引っかからない指摘は誤検出である。
#
# 番号は sample-new.txt / sample-old.txt / sample-both.txt /
# sample-haiku-fresh.txt に印字した通し番号である。

OK = {
    "new": {58},   # バケット
    "old": {1, 38, 55},  # 野生・ボード・導線
    "both": {15, 17},  # 窓・原子
    "fresh": set(),
}

# 妥当と見た件の理由。
WHY_OK = {
    ("new", 58): "bucket をそのまま写した語で、日本語の資産運用の文書では区分や枠と書く。読み手が引っかかる",
    ("old", 1): "in the wild の直訳。日本語の脆弱性情報では実環境での悪用と書く",
    ("old", 38): "board をそのまま写した語で、日本語では取締役会か経営層と書く(境界の判定)",
    ("old", 55): "電線を指す導線を、人の動きを指す動線の意味で使っている。同音の語の取り違えである",
    ("both", 15): "window を写した比喩で、日本語では観測の口や経路と書く",
    ("both", 17): "atomic の直訳。実務ではアトミックか不可分と書く(境界の判定)",
}

# 語ごとの、誤検出とした理由の型。
ESTABLISHED = "現代の技術文書で定着した術語であり、読み手は引っかからない"
ORDINARY = "普通の日本語の用法で、転用でも直訳でもない"

REASON = {
    # 計算機・セキュリティで定着した術語
    "アクセス": ESTABLISHED, "シェル": ESTABLISHED, "ストア": ESTABLISHED,
    "処理": ESTABLISHED, "スキャン": ESTABLISHED, "インターネット": ESTABLISHED,
    "ハッシュ": ESTABLISHED, "検体": ESTABLISHED, "マップ": ESTABLISHED,
    "バッファー": ESTABLISHED, "プラットフォーム": ESTABLISHED,
    "モジュール": ESTABLISHED, "スナップショット": ESTABLISHED,
    "バッチ処理": ESTABLISHED, "パイプ": ESTABLISHED, "ノイズ": ESTABLISHED,
    "標的": ESTABLISHED, "マイナー": ESTABLISHED, "パッチ": ESTABLISHED,
    "鍵": ESTABLISHED, "キー": ESTABLISHED, "亜種": ESTABLISHED,
    "ラップ": ESTABLISHED, "チェーン": ESTABLISHED, "バイパス": ESTABLISHED,
    "オーディオ": ESTABLISHED, "ロック": ESTABLISHED, "フォーク": ESTABLISHED,
    "タグ": ESTABLISHED, "トンネル": ESTABLISHED, "台帳": ESTABLISHED,
    "チケット": ESTABLISHED, "寿命": ESTABLISHED, "踏み台": ESTABLISHED,
    "麻痺": ESTABLISHED, "格子": ESTABLISHED, "指紋": ESTABLISHED,
    "消化": ESTABLISHED, "炎上": ESTABLISHED, "ラボ": ESTABLISHED,
    "コンテナ": ESTABLISHED, "ブラックボックス": ESTABLISHED,
    "ルーム": ESTABLISHED, "インジケーター": ESTABLISHED,
    "アダプター": ESTABLISHED, "発火": ESTABLISHED, "ブラウザ": ESTABLISHED,
    "ホーム": ESTABLISHED, "コレクション": ESTABLISHED, "ユニット": ESTABLISHED,
    "パッケージ": ESTABLISHED, "ページ": ESTABLISHED, "システム": ESTABLISHED,
    "プログラム": ESTABLISHED, "サブシステム": ESTABLISHED, "セット": ESTABLISHED,
    "ファイル": ESTABLISHED, "木": ESTABLISHED, "スクリプト": ESTABLISHED,
    "コントローラー": ESTABLISHED, "コンポーネント": ESTABLISHED,
    "ネットワーク": ESTABLISHED, "アプリ": ESTABLISHED, "拡張子": ESTABLISHED,
    "版": ESTABLISHED, "ポート": ESTABLISHED, "トリガー": ESTABLISHED,
    "ルーター": ESTABLISHED, "エクスポート": ESTABLISHED, "ツリー": ESTABLISHED,
    "デスクトップ": ESTABLISHED, "ハードウェア": ESTABLISHED,
    "フィールド": ESTABLISHED, "テーブル": ESTABLISHED, "ロード": ESTABLISHED,
    "ディスク": ESTABLISHED, "フレーム": ESTABLISHED, "ラベル": ESTABLISHED,
    "クローン": ESTABLISHED, "装置": ESTABLISHED, "機器": ESTABLISHED,
    "計算機": ESTABLISHED, "機械": ESTABLISHED, "台": ESTABLISHED,
    "紐": ESTABLISHED, "枠": ESTABLISHED, "主体": ESTABLISHED,
    "透明性": ESTABLISHED, "遠隔操作": ESTABLISHED, "配線": ESTABLISHED,
    "ハーネス": ESTABLISHED, "スタック": ESTABLISHED,
    # 普通の日本語の用法
    "勝手": ORDINARY, "水道": ORDINARY, "進化": ORDINARY, "添付": ORDINARY,
    "製品": ORDINARY, "自動": ORDINARY, "社": ORDINARY, "取引所": ORDINARY,
    "無害": ORDINARY, "土台": ORDINARY, "映像": ORDINARY, "携帯電話": ORDINARY,
    "電波": ORDINARY, "背景": ORDINARY, "反映": ORDINARY, "内線": ORDINARY,
    "自動車": ORDINARY, "工場": ORDINARY, "手": ORDINARY, "分子": ORDINARY,
    "組み込み": ORDINARY, "持ち出し": ORDINARY, "リセット": ORDINARY,
    "現象": ORDINARY, "性能": ORDINARY, "高速": ORDINARY, "電力": ORDINARY,
    "定番": ORDINARY, "汚染": ORDINARY, "武器": ORDINARY, "素通し": ORDINARY,
    "入れ子": ORDINARY, "導線": ORDINARY,
}


def verdict(side, index):
    return "ok" if index in OK[side] else "ng"


def reason(side, index, word):
    if index in OK[side]:
        return WHY_OK[(side, index)]
    return REASON.get(word, ESTABLISHED)


# 訂正後の方針の表 + abstract_ratio 0.85 で残った指摘 30 件の見立て
# (sample-haiku2-final.txt の通し番号)。基準は上と同じ。
FINAL_OK = {26, 30}
# 26 死: 「システムの死」は定着していない修辞的な転用(境界の判定)
# 30 機械: 検査対象の文書自身が「機械」を退ける語として挙げている箇所


def final_verdict(index):
    return "ok" if index in FINAL_OK else "ng"
