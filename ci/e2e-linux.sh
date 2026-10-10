#!/usr/bin/env bash
# End-to-end checks against real releases (Linux). Run from the repository root after
# `cargo build -p oktospace-cli`.
set -euxo pipefail
cli="$PWD/target/debug/oktospace-cli"
export OKTOSPACE_HOME="$RUNNER_TEMP/cs" XDG_DATA_HOME="$RUNNER_TEMP/xdg" XDG_BIN_HOME="$RUNNER_TEMP/bin" XDG_CONFIG_HOME="$RUNNER_TEMP/config"
installed() { python3 -c "import json,sys; d=json.load(open('$OKTOSPACE_HOME/installed.json')); print(d['apps']['$1']['current']['$2'] if '$1' in d['apps'] else '')"; }

"$cli" list
"$cli" info photocraft

# Tarball: install an older version, update, roll back, verify, uninstall.
"$cli" install photocraft --version 0.3.0
test "$(installed photocraft version)" = "0.3.0"
test -L "$XDG_BIN_HOME/photocraft"
"$XDG_BIN_HOME/photocraft-cli" --version
ls "$XDG_DATA_HOME/applications/" | grep -q photocraft
grep -q "Exec=$OKTOSPACE_HOME" "$XDG_DATA_HOME"/applications/*photocraft*.desktop
"$cli" verify photocraft
"$cli" update photocraft
test "$(installed photocraft version)" != "0.3.0"
"$XDG_BIN_HOME/photocraft-cli" --version
"$cli" rollback photocraft
test "$(installed photocraft version)" = "0.3.0"
"$cli" channel photocraft pin
"$cli" check || test $? -eq 10
"$cli" channel photocraft default
"$cli" uninstall photocraft --yes
test ! -e "$XDG_BIN_HOME/photocraft"
test -z "$(installed photocraft version)"

# AppImage with a delta update from the .zsync file.
"$cli" config prefer_appimage true
"$cli" install photocraft --version 0.3.0
test "$(installed photocraft kind)" = "app-image"
"$cli" -v update photocraft 2>&1 | tee "$RUNNER_TEMP/delta.log"
# Either a delta update, or a full download because too little could be reused.
grep -qiE "delta update" "$RUNNER_TEMP/delta.log"
"$cli" verify photocraft
"$cli" uninstall photocraft --yes
"$cli" config prefer_appimage false

# Export / import.
"$cli" install gridcraft -q
"$cli" export "$RUNNER_TEMP/apps.json"
"$cli" uninstall gridcraft --yes
"$cli" import "$RUNNER_TEMP/apps.json"
test -n "$(installed gridcraft version)"

# Machine policy: required apps get installed, others hidden.
echo '{"required_apps": ["deckcraft"], "allowed_apps": ["deckcraft", "gridcraft"], "settings": {"keep_previous_version": false}}' > "$RUNNER_TEMP/policy.json"
OKTOSPACE_POLICY="$RUNNER_TEMP/policy.json" "$cli" apply-policy
test -n "$(installed deckcraft version)"
test "$(OKTOSPACE_POLICY="$RUNNER_TEMP/policy.json" "$cli" list | tail -n +2 | wc -l)" -eq 2
if OKTOSPACE_POLICY="$RUNNER_TEMP/policy.json" "$cli" config keep_previous_version true; then echo "policy should lock this"; exit 1; fi

# IT and classrooms: a lab policy (pinned version, no uninstalling, update window, reports) and a
# shared package cache that a second computer installs from.
lab="$RUNNER_TEMP/lab"
mkdir -p "$lab/share/reports" "$lab/share/packages"
cat > "$lab/policy.json" <<POLICY
{
  "organization": "CI Art Lab",
  "support": "https://example.com/help",
  "required_apps": ["photocraft"],
  "pinned_versions": { "photocraft": "0.3.0" },
  "update_window": { "days": [], "from": "03:00", "to": "03:00" },
  "prevent_uninstall": true,
  "report_dir": "$lab/share/reports",
  "settings": { "package_cache": "$lab/share/packages", "package_cache_write": true }
}
POLICY
export OKTOSPACE_POLICY="$lab/policy.json"
"$cli" policy check "$lab/policy.json"
"$cli" policy show | tee "$lab/show.txt"
grep -q "CI Art Lab" "$lab/show.txt"
export OKTOSPACE_HOME="$lab/pc1"
"$cli" apply-policy
test "$(installed photocraft version)" = "0.3.0"
ls "$lab/share/packages" | grep -q "photocraft-0.3.0"
ls "$lab/share/reports"/*.json
"$cli" report --json | python3 -c "import json,sys; d=json.load(sys.stdin); a={x['id']:x for x in d['apps']}; assert a['photocraft']['version']=='0.3.0', a; assert d['policy']['organization']=='CI Art Lab', d['policy']"
# Pinned and outside the update window: nothing changes.
"$cli" update --scheduled
test "$(installed photocraft version)" = "0.3.0"
# Students can't uninstall (the runner isn't root).
if "$cli" uninstall photocraft --yes; then echo "prevent_uninstall should stop this"; exit 1; fi
test "$(installed photocraft version)" = "0.3.0"
# A second computer gets the package from the share.
export OKTOSPACE_HOME="$lab/pc2"
"$cli" -v apply-policy 2>&1 | tee "$lab/pc2.txt"
grep -q "from the package cache" "$lab/pc2.txt"
test "$(installed photocraft version)" = "0.3.0"
unset OKTOSPACE_POLICY

# Other sources (optional): an app from any GitHub repository.
export OKTOSPACE_HOME="$RUNNER_TEMP/sources"
"$cli" config prefer_system_installer false
if "$cli" source add BurntSushi/ripgrep --binary rg; then echo "other sources are off by default"; exit 1; fi
"$cli" source check BurntSushi/ripgrep
"$cli" source enable
"$cli" source add BurntSushi/ripgrep --binary rg
"$cli" list | grep -i ripgrep
"$cli" install ripgrep
"$(installed ripgrep executable)" --version
"$cli" verify ripgrep
"$cli" source list | tee "$RUNNER_TEMP/sources.txt"
grep -q "BurntSushi/ripgrep" "$RUNNER_TEMP/sources.txt"
if "$cli" source remove ripgrep; then echo "installed apps stay on the list"; exit 1; fi
"$cli" uninstall ripgrep --yes
"$cli" source remove ripgrep
"$cli" source set oktospace emircesur/OktoSpace
"$cli" source set oktospace official
"$cli" source disable
test -z "$("$cli" list | grep -i ripgrep || true)"
export OKTOSPACE_HOME="$RUNNER_TEMP/cs"

# Build from source: a Go program from its latest release, installed like a download.
export OKTOSPACE_HOME="$RUNNER_TEMP/build"
"$cli" build junegunn/fzf 2>&1 | tee "$RUNNER_TEMP/build.log"
grep -q "Build system: Go" "$RUNNER_TEMP/build.log"
grep -q "Built and installed fzf" "$RUNNER_TEMP/build.log"
"$(installed fzf executable)" --version
"$cli" uninstall fzf --yes
test -z "$(installed fzf version)"
export OKTOSPACE_HOME="$RUNNER_TEMP/cs"

# Start at login, self-install.
"$cli" autostart on
test -f "$XDG_CONFIG_HOME/autostart/oktospace.desktop"
"$cli" autostart off
test ! -f "$XDG_CONFIG_HOME/autostart/oktospace.desktop"
"$cli" self-install
"$XDG_BIN_HOME/oktospace-cli" --version
echo "Linux end-to-end checks passed"
