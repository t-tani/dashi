#!/bin/bash
# 指定した設定ディレクトリで、3 コーパスへ lint を掛ける。閾値は書き換えない。
# 使い方: lint_v4.sh <設定ディレクトリ> <印>
W="$WORK_DIR"
DIR=$1
TAG=$2
for c in sub docs-a docs-b; do
  case $c in
    sub) p="$AKUNUKI_DIR"/評価コーパス ;;
    *) p="$CORPUS_DIR/$c/docs" ;;
  esac
  "$AKUNUKI_DIR"/target/release/aku lint --format json \
    --config "$W/$DIR/config.toml" --dict "$AKUNUKI_DIR"/.aku/dict \
    "$p" > "$W/$c-$TAG.json" 2>/dev/null || true
done
echo "$TAG done"
