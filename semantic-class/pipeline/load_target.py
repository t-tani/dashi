"""分類対象の定義を読み込む。

分類対象(名詞・動詞)ごとの定義は、その成果物のディレクトリの target.py にある。
共通のスクリプトは --target でそのディレクトリを受け取り、この関数で読み込む。
定義が持つ名前は次のとおりである。

    NAME           分類対象の識別子。noun か verb
    POS_NAME       表の注記に書く品詞の名前
    CLASS_PREFIX   表へ書く分類番号の整数部。分類語彙表の類の番号で、1 が体、2 が用
    CATEGORIES     許す中項目番号の集合
    INSTRUCTIONS   Signature の指示文。中項目の一覧と生成の方針の全文を持つ
    DEMOS          few-shot の実例。(語, 用例文 or None, 中項目の一覧) の組の群
    is_target_pos  JMdict の 1 語義の品詞の集合を受け、分類にかける品詞なら True
    MANIFEST       manifest.json のうち、分類対象に固有の節

読み込んだモジュールには DIRECTORY を足す。manifest の来歴にディレクトリ名を
書くためである。
"""

import importlib.util
import os


def load(directory):
    path = os.path.join(directory, "target.py")
    spec = importlib.util.spec_from_file_location("target_definition", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.DIRECTORY = os.path.abspath(directory)
    return module
