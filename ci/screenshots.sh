#!/usr/bin/env bash
# README screenshots: OktoSpace on a virtual display with a fresh profile, after it has fetched
# release information and icons. Writes PNGs to $OUT (default: screenshots/).
set -euo pipefail
BIN="${BIN:-target/release/oktospace}"
OUT="$(mkdir -p "${OUT:-screenshots}" && cd "${OUT:-screenshots}" && pwd)"
WORK="$(mktemp -d)"
export HOME="$WORK/home" OKTOSPACE_HOME="$WORK/oktospace" DISPLAY=:92
mkdir -p "$OKTOSPACE_HOME"
echo '{"theme":"dark","offer_self_install":false,"install_mode_chosen":true}' > "$OKTOSPACE_HOME/settings.json"
Xvfb :92 -screen 0 1400x900x24 >/dev/null 2>&1 &
trap 'kill $(jobs -p) 2>/dev/null || true' EXIT
sleep 2

shot() { # name view wait
  OKTOSPACE_VIEW="$2" "$BIN" >"$OUT/$1.log" 2>&1 &
  local app=$!
  for _ in $(seq 30); do id=$(xdotool search --onlyvisible --name '^OktoSpace$' 2>/dev/null | head -1) && [ -n "$id" ] && break; sleep 1; done
  xdotool windowmove "$id" 0 0 windowsize "$id" 1280 820
  xdotool mousemove 1390 890
  sleep "$3"
  import -window root -crop 1280x820+0+0 "$OUT/$1.png"
  kill "$app"; wait "$app" 2>/dev/null || true
}

# The first start fetches releases and icons; later ones use the cache.
shot apps "" 60
shot apps "" 15
shot discover discover 15
shot app-details app:zed 20
shot build app:llama-cpp:build 15
ls -la "$OUT"
