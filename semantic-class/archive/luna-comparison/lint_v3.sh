#!/bin/bash
# 指定した設定ディレクトリで、3 コーパスへ lint を掛ける。
# 使い方: lint_v3.sh <設定ディレクトリ> <印> <abstract_ratio>
W="$WORK_DIR"
DIR=$1
TAG=$2
RATIO=$3
sed -i "s/^abstract_ratio = .*/abstract_ratio = $RATIO/" "$W/$DIR/config.toml"
for c in sub docs-a docs-b; do
  case $c in
    sub) p="$AKUNUKI_DIR"/評価コーパス ;;
    *) p="$CORPUS_DIR/$c/docs" ;;
  esac
  "$AKUNUKI_DIR"/target/release/aku lint --format json \
    --config "$W/$DIR/config.toml" --dict "$AKUNUKI_DIR"/.aku/dict \
    "$p" > "$W/$c-$TAG-$RATIO.json" 2>/dev/null || true
done
echo "$TAG ratio $RATIO done"
