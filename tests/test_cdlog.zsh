#!/usr/bin/env zsh
set -euo pipefail

SCRIPT_DIR="${0:A:h}"
PROJECT_ROOT="${SCRIPT_DIR:h}"

TEST_TMP=$(mktemp -d "/tmp/cdlog_zsh_test_XXXXXX")
trap 'rm -rf "$TEST_TMP"' EXIT

TEST_LOG="$TEST_TMP/.cdlog"
export CDLOG_PATH="$TEST_LOG"
export PATH="$PROJECT_ROOT/target/release:$PATH"

if ! (( $+commands[cdlog] )); then
  echo "Error: cdlog binary not found in PATH ($PROJECT_ROOT/target/release). Please build with cargo build --release."
  exit 1
fi

echo "=== 1. Load cdlog.zsh ==="
source "$PROJECT_ROOT/cdlog.zsh"
echo "OK: cdlog.zsh loaded"

echo "=== 2. Test chpwd hook with directory changes ==="
DIR_A="$TEST_TMP/dir_a"
DIR_B="$TEST_TMP/dir_b"
DIR_SPECIAL="$TEST_TMP/dir with spaces & 'quotes'\""
mkdir -p "$DIR_A" "$DIR_B" "$DIR_SPECIAL"

cd "$DIR_A"
cd "$DIR_B"
cd "$DIR_SPECIAL"

echo "=== 3. Verify log contents ==="
if [[ ! -f "$TEST_LOG" ]]; then
  echo "Error: log file $TEST_LOG was not created!"
  exit 1
fi

LOG_CONTENT=$(cat "$TEST_LOG")
echo "Log contents:"
echo "$LOG_CONTENT"
echo "---"

FIRST_LINE=$(head -n 1 "$TEST_LOG")
EXPECTED_SPECIAL="${(q)DIR_SPECIAL}"
if [[ "$FIRST_LINE" != "$EXPECTED_SPECIAL" ]]; then
  echo "Error: expected first line to be '$EXPECTED_SPECIAL', got '$FIRST_LINE'"
  exit 1
fi
echo "OK: Latest path is at the top with proper escaping"

echo "=== 4. Test deduplication and move-to-front ==="
cd "$DIR_A"
FIRST_LINE=$(head -n 1 "$TEST_LOG")
EXPECTED_A="${(q)DIR_A}"
if [[ "$FIRST_LINE" != "$EXPECTED_A" ]]; then
  echo "Error: expected first line to be '$EXPECTED_A', got '$FIRST_LINE'"
  exit 1
fi

LINE_COUNT=$(grep -c -F "$EXPECTED_A" "$TEST_LOG")
if [[ "$LINE_COUNT" -ne 1 ]]; then
  echo "Error: duplicate entry found for $EXPECTED_A (count: $LINE_COUNT)"
  exit 1
fi
echo "OK: Deduplication and move-to-front works correctly"

echo "=== 5. Test cdlog-cd navigation (mock finder) ==="
export CDLOG_FUZZY_FINDER="head -n 1"
cdlog-cd
if [[ "$PWD" != "$DIR_SPECIAL" ]]; then
  echo "Error: cdlog-cd failed to change directory to '$DIR_SPECIAL'. Current PWD: '$PWD'"
  exit 1
fi
echo "OK: cdlog-cd successfully navigated to special character directory"

echo "=== 6. Test multibyte and emoji paths ==="
DIR_JA="$TEST_TMP/日本語のディレクトリ_プロジェクト名"
DIR_EMOJI="$TEST_TMP/🦀_rust_プロジェクト_🚀"
mkdir -p "$DIR_JA" "$DIR_EMOJI"

cd "$DIR_JA"
FIRST_LINE=$(head -n 1 "$TEST_LOG")
EXPECTED_JA="${(q)DIR_JA}"
if [[ "$FIRST_LINE" != "$EXPECTED_JA" ]]; then
  echo "Error: expected first line to be '$EXPECTED_JA', got '$FIRST_LINE'"
  exit 1
fi

cd "$DIR_EMOJI"
FIRST_LINE=$(head -n 1 "$TEST_LOG")
EXPECTED_EMOJI="${(q)DIR_EMOJI}"
if [[ "$FIRST_LINE" != "$EXPECTED_EMOJI" ]]; then
  echo "Error: expected first line to be '$EXPECTED_EMOJI', got '$FIRST_LINE'"
  exit 1
fi

cdlog-cd
if [[ "$PWD" != "$DIR_JA" ]]; then
  echo "Error: cdlog-cd failed to navigate to Japanese directory. Current PWD: '$PWD'"
  exit 1
fi
echo "OK: Successfully recorded, retrieved and navigated to Japanese/Emoji directories"

echo "=== 7. Test control chars and non-UTF8 in Zsh ==="
zsh_edge_cases=(
  $'\x11_dc1_control'
  $'\x01_soh_control'
  $'\x1b_esc_control'
  $'\x7f_del_control'
  $'\t_tab_control'
  $'\xff\xfe_non_utf8_bytes'
  'shell_`backtick`_and_$dollar_and_"double"_and_'\''single'\'''
)

for edge_case in "${zsh_edge_cases[@]}"; do
  edge_dir="$TEST_TMP/dir_$edge_case"
  mkdir -p "$edge_dir"
  cd "$edge_dir"

  expected_escaped="${(q)edge_dir}"
  first_line=$(head -n 1 "$TEST_LOG")
  if [[ "$first_line" != "$expected_escaped" ]]; then
    echo "Error: expected first line to be '$expected_escaped', got '$first_line'"
    exit 1
  fi

  cd "$TEST_TMP"
  cdlog-cd
  if [[ "$PWD" != "$edge_dir" ]]; then
    echo "Error: cdlog-cd failed to restore directory: '$edge_dir'. Current PWD: '$PWD'"
    exit 1
  fi
done
echo "OK: All control character and non-UTF8 directories correctly handled end-to-end"

echo "=== All ZSH plugin tests passed successfully! ==="
