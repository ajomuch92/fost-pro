#!/usr/bin/env bash
# Runs the public Visual FoxPro examples from Rosetta Code through the interpreter and
# reports how many finish without an error, plus the most common reasons for failing.
# The examples are downloaded into ./corpus (git-ignored: they carry their own license).
set -euo pipefail
cd "$(dirname "$0")"

if [ ! -d corpus ]; then
  git clone --quiet --depth 1 --filter=blob:none --sparse https://github.com/acmeism/RosettaCodeData corpus
  git -C corpus sparse-checkout set --no-cone 'Task/*/Visual-FoxPro/*'
fi

cargo build --quiet --release
pass=0 total=0 reasons=$(mktemp)
while IFS= read -r -d '' f; do
  total=$((total + 1))
  # no stdin and a time limit: some examples wait for input or loop forever
  if err=$(timeout 5 ../target/release/fost "$f" 2>&1 >/dev/null </dev/null); then
    pass=$((pass + 1))
  else
    echo "${err:-tiempo agotado}" | head -1 | sed 's/^Línea [0-9]*: //' >>"$reasons"
  fi
done < <(find corpus/Task -type f -print0)

echo "$pass de $total programas terminan sin error"
sort "$reasons" | uniq -c | sort -rn | head -"${1:-25}"
rm "$reasons"
