#!/usr/bin/env bash
set -euo pipefail

# Import Claude Code config from a tarball into ~/.claude.
# Usage: ./import-claude-setup.sh [tarball-path]
#   tarball-path defaults to ./claude-setup.tar.gz

TARBALL="${1:-./claude-setup.tar.gz}"
CLAUDE_DIR="${HOME}/.claude"

if [[ ! -f "$TARBALL" ]]; then
  echo "Error: tarball not found at $TARBALL" >&2
  echo "Usage: $0 [path-to-claude-setup.tar.gz]" >&2
  exit 1
fi

echo "Importing Claude Code setup from $TARBALL"
echo "  Target: $CLAUDE_DIR"
echo ""

# Preview what's in the tarball
echo "Contents:"
tar -tzf "$TARBALL" | head -40
TOTAL=$(tar -tzf "$TARBALL" | wc -l | tr -d ' ')
if [[ "$TOTAL" -gt 40 ]]; then
  echo "  ... and $((TOTAL - 40)) more files"
fi
echo ""

# Check if ~/.claude already exists
if [[ -d "$CLAUDE_DIR" ]]; then
  echo "Warning: $CLAUDE_DIR already exists."
  echo "  Existing files will be OVERWRITTEN by tarball contents."
  echo "  Files not in the tarball will be left alone."
  read -rp "Continue? [y/N] " confirm
  if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    echo "Aborted."
    exit 0
  fi
fi

mkdir -p "$CLAUDE_DIR"
tar -xzf "$TARBALL" -C "$HOME"

echo ""
echo "Imported successfully."
echo ""

# Verify key files
echo "Verification:"
for item in CLAUDE.md settings.json commands agents hooks skills; do
  if [[ -e "$CLAUDE_DIR/$item" ]]; then
    if [[ -d "$CLAUDE_DIR/$item" ]]; then
      count=$(find "$CLAUDE_DIR/$item" -type f | wc -l | tr -d ' ')
      echo "  OK  $item/ ($count files)"
    else
      echo "  OK  $item"
    fi
  else
    echo "  --  $item (not present)"
  fi
done

echo ""

# Check plugin auto-install readiness
SETTINGS="$CLAUDE_DIR/settings.json"
if [[ -f "$SETTINGS" ]] && command -v python3 &>/dev/null; then
  PLUGIN_COUNT=$(python3 -c "
import json, sys
try:
    d = json.load(open('$SETTINGS'))
    plugins = d.get('enabledPlugins', {})
    enabled = [k for k, v in plugins.items() if v]
    print(len(enabled))
except: print(0)
" 2>/dev/null || echo "0")
  if [[ "$PLUGIN_COUNT" -gt 0 ]]; then
    echo "Note: settings.json references $PLUGIN_COUNT enabled plugin(s)."
    echo "  These will be auto-downloaded by Claude Code on first run."
    echo "  Ensure the box has internet access for plugin installation."
  fi
fi

# Check hook dependencies
if [[ -d "$CLAUDE_DIR/hooks" ]]; then
  TS_HOOKS=$(find "$CLAUDE_DIR/hooks" -name "*.ts" -type f 2>/dev/null | wc -l | tr -d ' ')
  if [[ "$TS_HOOKS" -gt 0 ]]; then
    echo ""
    echo "Hook dependency check:"
    if command -v node &>/dev/null; then
      echo "  OK  node $(node --version)"
    else
      echo "  MISSING  node — hooks using 'npx tsx' will fail"
    fi
    if command -v npx &>/dev/null; then
      echo "  OK  npx available"
    else
      echo "  MISSING  npx — hooks using 'npx tsx' will fail"
    fi
  fi
fi

echo ""
echo "Setup complete. Run 'claude' to start."
