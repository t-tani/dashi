#!/bin/bash
# 既定の前後で 4 つの対象へ lint を掛ける。設定は閾値と表だけが違う。
W="$WORK_DIR"
AKU="$AKUNUKI_DIR"/target/release/aku
for side in before after; do
  for t in sub docs-a docs-b akunuki-docs; do
    case $t in
      sub) p=""$AKUNUKI_DIR"/評価コーパス" ;;
      docs-a) p="$CORPUS_DIR/docs-a/docs" ;;
      docs-b) p="$CORPUS_DIR/docs-b/docs" ;;
      akunuki-docs) p=""$AKUNUKI_DIR"/README.md "$AKUNUKI_DIR"/docs" ;;
    esac
    $AKU lint --format json --config "$W/cfg-$side/config.toml" \
      --dict "$AKUNUKI_DIR"/.aku/dict $p > "$W/$t-$side.json" 2>/dev/null || true
  done
  echo "$side 完了"
done
