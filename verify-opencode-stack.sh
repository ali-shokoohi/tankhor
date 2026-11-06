#!/usr/bin/env bash
# verify-opencode-stack.sh  (v2: OpenCode 1.x AND 2.x aware)
# Zero-LLM sanity checks for: RTK + DCP + codebase-memory-mcp + LSP + AGENTS.md + agenttrace
# Usage: ./verify-opencode-stack.sh [/path/to/project]   (run from the workspace root)
# Nothing here calls the model, so it costs no GPU time.

set -u
PROJECT="${1:-$PWD}"
OC_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/opencode"
OC_DATA="${XDG_DATA_HOME:-$HOME/.local/share}/opencode"
FAILS=0; WARNS=0

pass(){ printf '\033[32mPASS\033[0m %s\n' "$*"; }
fail(){ printf '\033[31mFAIL\033[0m %s\n' "$*"; FAILS=$((FAILS+1)); }
warn(){ printf '\033[33mWARN\033[0m %s\n' "$*"; WARNS=$((WARNS+1)); }
info(){ printf '     %s\n' "$*"; }
have(){ command -v "$1" >/dev/null 2>&1; }

cd "$PROJECT" || { echo "cannot cd to $PROJECT"; exit 2; }

echo "== 0. Binaries on PATH"
for b in opencode rtk agenttrace codebase-memory-mcp jq; do
  have "$b" && pass "$b found ($(command -v "$b"))" || fail "$b missing"
done

echo; echo "== 1. OpenCode version"
OC_VER="$(opencode --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n1)"
OC_MAJOR="${OC_VER%%.*}"; OC_MAJOR="${OC_MAJOR:-0}"
info "opencode $OC_VER (major $OC_MAJOR)"
if [[ "$OC_MAJOR" -ge 2 ]]; then
  warn "OpenCode 2.x: V1 plugins do NOT run, and the LSP subsystem is not run. Checks below are adjusted."
fi
OCAPI="opencode"; have opencode2 && OCAPI="opencode2"

echo; echo "== 2. Merged config shape (opencode debug config)"
RAW="$(mktemp)"; CFG="$(mktemp)"; trap 'rm -f "$RAW" "$CFG"' EXIT
if opencode debug config >"$RAW" 2>/dev/null && jq empty "$RAW" 2>/dev/null; then
  TYPE="$(jq -r 'type' "$RAW")"
  info "top-level type: $TYPE"
  if [[ "$TYPE" == "array" ]]; then
    info "element types: $(jq -c '[.[] | type] | group_by(.) | map({(.[0]): length}) | add' "$RAW")"
    info "first element keys: $(jq -c '.[0] | if type=="object" then keys else . end' "$RAW" | head -c 400)"
    # Assume an ordered list of config layers; deep-merge objects, later wins.
    jq 'reduce (.[] | objects) as $x ({}; . * $x)' "$RAW" >"$CFG"
    info "merged $(jq '[.[]|objects]|length' "$RAW") object layers (assumption: array = config layers; verify with the keys above)"
  else
    cp "$RAW" "$CFG"
  fi

  info "top-level keys: $(jq -c 'keys' "$CFG" | head -c 600)"

  # Plugins: V2 key is "plugins", V1 is "plugin"
  PLUGINS="$(jq -r '(.plugins // .plugin // [])[]? | if type=="array" then .[0] elif type=="object" then (.package // .name // tostring) else . end' "$CFG")"
  if [[ -n "$PLUGINS" ]]; then
    while read -r p; do
      [[ -z "$p" ]] && continue
      if [[ "$p" == *"@latest"* ]]; then warn "unpinned plugin: $p"
      elif [[ "$p" == file://* || "$p" == ./* || "$p" == ../* || "$p" == /* ]]; then
        [[ "$p" == *cbm-augment* ]] && warn "local plugin still present: $p (codebase-memory is off; delete it)" || pass "local plugin: $p"
      elif [[ "$p" =~ ^-?@?[^@]+$ ]]; then warn "no version pin: $p"
      else pass "configured plugin: $p"; fi
    done <<<"$PLUGINS"
  else
    info "no npm plugins in config (file plugins in $OC_DIR/plugins are auto-discovered)"
  fi
  [[ "$OC_MAJOR" -ge 2 ]] && jq -e 'has("plugin") and (has("plugins")|not)' "$CFG" >/dev/null \
    && warn "config uses V1 key \"plugin\"; V2 key is \"plugins\" (V2 normalizes in memory, but V1 *implementations* still won't run)"

  # MCP: V2 groups under mcp.servers and uses "disabled"; V1 uses mcp.<n>.enabled
  CBM_KEY="$(jq -r '(.mcp.servers // .mcp // {}) | if type=="object" then keys[] else empty end | select(test("codebase"))' "$CFG" | head -n1)"
  if [[ -n "$CBM_KEY" ]]; then
    ST="$(jq -c --arg k "$CBM_KEY" '(.mcp.servers // .mcp)[$k] | {enabled, disabled}' "$CFG")"
    info "mcp $CBM_KEY: $ST"
    jq -e --arg k "$CBM_KEY" '(.mcp.servers // .mcp)[$k] | (.enabled == false) or (.disabled == true)' "$CFG" >/dev/null \
      && info "codebase-memory MCP disabled in config (intended until the repo has code)" || info "codebase-memory MCP enabled in config"
  else
    warn "no codebase-memory entry found under .mcp / .mcp.servers"
  fi

  info "agent(s) explore: $(jq -c '(.agents.explore // .agent.explore // "none")' "$CFG" | head -c 400)"
  info "compaction = $(jq -c 'if has("compaction") then .compaction else "default" end' "$CFG")   snapshot = $(jq -c 'if has("snapshot") then .snapshot elif has("snapshots") then .snapshots else "default" end' "$CFG")"
  info "model = $(jq -r '.model // "unset"' "$CFG")   small_model = $(jq -r '.small_model // .smallModel // "unset"' "$CFG")"
else
  fail "opencode debug config failed or did not return JSON"
fi

echo; echo "== 3. Plugins actually LOADED (not just present on disk)"
ls -1 "$OC_DIR"/plugins "$PROJECT"/.opencode/plugins 2>/dev/null | sed 's/^/     /'
if [[ "$OC_MAJOR" -ge 2 ]]; then
  LOADED="$($OCAPI api get /api/plugin 2>/dev/null)"
  if [[ -n "$LOADED" ]]; then
    info "active plugins (from $OCAPI api get /api/plugin):"
    echo "$LOADED" | jq -c '.' 2>/dev/null | head -c 1500 | sed 's/^/     /'; echo
    for want in rtk dcp cbm; do
      grep -qi "$want" <<<"$LOADED" && pass "loaded: *$want*" || fail "NOT loaded: *$want* (file on disk is not enough)"
    done
  else
    warn "could not query /api/plugin (no running server?). Use the log check below."
  fi
  LOG="$(ls -t "$OC_DATA"/log/*.log 2>/dev/null | head -n1)"
  if [[ -n "$LOG" ]]; then
    ERRS="$(grep -iE 'plugin' "$LOG" | grep -iE 'error|warn|SchemaError|must export|cannot find' | tail -n 10)"
    [[ -n "$ERRS" ]] && { fail "plugin load errors in $LOG:"; echo "$ERRS" | cut -c1-220 | sed 's/^/     /'; } \
                     || pass "no plugin load errors in latest log ($LOG)"
  else
    warn "no log in $OC_DATA/log; run once: opencode run --standalone --print-logs \"hi\" 2>&1 | grep -iE 'plugin|rtk|dcp|cbm'"
  fi
fi

if [[ "$OC_MAJOR" -lt 2 ]]; then
  # CLI and server both append to log/opencode.log; only look at today's lines
  LOG="$OC_DATA/log/opencode.log"
  TODAY="$(date -u +%F)"
  if [[ -f "$LOG" ]] && grep -q "$TODAY" "$LOG"; then
    RECENT="$(grep "$TODAY" "$LOG")"
    info "log: $LOG ($(wc -l <<<"$RECENT") lines from $TODAY UTC)"
    for want in rtk dcp; do
      grep -qi "$want" <<<"$RECENT" && pass "today's log mentions *$want*" || warn "today's log never mentions *$want* (run: opencode run --print-logs --log-level DEBUG hi)"
    done
    ERRS="$(grep -iE 'plugin' <<<"$RECENT" | grep -iE 'error|fail|cannot|must export' | grep -viE 'ShowHelp' | tail -n 10)"
    [[ -n "$ERRS" ]] && { fail "plugin errors in log:"; echo "$ERRS" | cut -c1-220 | sed 's/^/     /'; } || pass "no plugin errors in today's log"
  else
    warn "no log lines from today; run: opencode run --print-logs --log-level DEBUG hi, then re-run"
  fi
fi

echo; echo "== 4. RTK"
rtk gain >/dev/null 2>&1 && pass "rtk binary OK (correct package)" || fail "rtk gain failed"
if [[ "$OC_MAJOR" -ge 2 ]]; then
  rtk hook opencode --help >/dev/null 2>&1; RC=$?
  case $RC in
    0) pass "rtk has 'hook opencode' (needed for V2 plugin)";;
    2) fail "this rtk build lacks 'hook opencode' -> its OpenCode plugin cannot work on V2";;
    *) fail "rtk hook opencode --help exited $RC";;
  esac
  grep -q 'export default' "$OC_DIR/plugins/rtk.ts" 2>/dev/null && pass "rtk.ts has a default export" \
    || fail "rtk.ts has no default export -> V1-only plugin, rejected by V2"
fi

echo; echo "== 5. DCP"
DCP_CFG="$(ls "$OC_DIR"/dcp.json{c,} "$PROJECT"/.opencode/dcp.json{c,} 2>/dev/null | head -n1)"
if [[ -n "$DCP_CFG" ]]; then
  pass "DCP config: $DCP_CFG"
  grep -Eq '"permission"[[:space:]]*:[[:space:]]*"deny"' "$DCP_CFG" && pass "compress denied (zero-LLM mode)" \
    || warn "compress not denied -> model spends decode tokens on summaries"
else
  warn "no dcp.jsonc"
fi
[[ "$OC_MAJOR" -ge 2 ]] && info "V2 needs DCP >= 3.2.0 (first release with native V2 support)"

echo; echo "== 6. Source files in this repo"
NRS="$(git ls-files '*.rs' 2>/dev/null | wc -l)"; NGO="$(git ls-files '*.go' 2>/dev/null | wc -l)"
info ".rs files: $NRS   .go files: $NGO"
if [[ "$NRS" -eq 0 && "$NGO" -eq 0 ]]; then
  warn "no Rust/Go code yet -> code graph and LSP add cost but have nothing to index"
fi

echo; echo "== 7. codebase-memory-mcp"
opencode mcp list 2>/dev/null | grep -i codebase | sed 's/^/     /'
opencode mcp list 2>/dev/null | grep -i codebase | grep -qi 'connected' \
  && ! opencode mcp list 2>/dev/null | grep -i codebase | grep -qiE 'not connected|disconnected|failed' \
  && pass "MCP connected" || info "MCP not connected (expected while disabled)"
PJ="$(timeout 30 codebase-memory-mcp cli list_projects '{}' 2>/dev/null)"
grep -qE 'projects:[[:space:]]*0' <<<"$PJ" && info "0 projects indexed" || { info "indexed:"; echo "$PJ" | head -c 800 | sed 's/^/     /'; echo; }

echo; echo "== 8. LSP"
if [[ "$OC_MAJOR" -ge 2 ]]; then
  info "OpenCode 2 does not run language servers or expose LSP tools; LSP config is kept but inert."
  [[ "${OPENCODE_EXPERIMENTAL_LSP_TOOL:-}" == "true" ]] && warn "OPENCODE_EXPERIMENTAL_LSP_TOOL is set but has no effect on V2"
  info "Feedback loop on V2 = cargo check / go vet via bash (compressed by RTK once it loads)."
else
  [[ "${OPENCODE_EXPERIMENTAL_LSP_TOOL:-}" == "true" ]] && pass "LSP tool env set" || fail "OPENCODE_EXPERIMENTAL_LSP_TOOL not set"
fi
have rust-analyzer && info "rust-analyzer on PATH"; have gopls && info "gopls on PATH"

echo; echo "== 9. AGENTS.md (sent in every prompt prefix)"
TOTAL=0
for AG in "$OC_DIR/AGENTS.md" "$PROJECT/AGENTS.md"; do
  [[ -f "$AG" ]] || continue
  SZ=$(wc -c <"$AG"); TOTAL=$((TOTAL+SZ)); info "$AG: ${SZ}B"
  grep -qiE 'codebase-memory|search_graph|index_repository|graph tools' "$AG" \
    && warn "$AG tells the agent to use graph tools (installer block?)"
done
grep -q "Token discipline" "$OC_DIR/AGENTS.md" 2>/dev/null && pass "token-discipline block present" || fail "token-discipline block missing"
info "combined ${TOTAL}B ~ $((TOTAL/4)) tokens on EVERY request (rough bytes/4)"
[[ $TOTAL -gt 6000 ]] && warn "combined AGENTS.md > 6KB; trim it"

echo; echo "== 10. agenttrace"
if have agenttrace; then
  agenttrace --doctor 2>&1 | tail -n 15 | sed 's/^/     /'
else
  info "install: cargo install --git https://github.com/luoyuctl/agenttrace agenttrace"
fi

echo; echo "== Summary: $FAILS fail, $WARNS warn"
[[ $FAILS -eq 0 ]]
