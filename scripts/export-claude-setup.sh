#!/usr/bin/env bash
set -euo pipefail

# Export portable Claude Code config from ~/.claude into a tarball.
# Usage: ./export-claude-setup.sh [output-path]
#   output-path defaults to ./claude-setup.tar.gz

CLAUDE_DIR="${HOME}/.claude"
OUTPUT="${1:-./claude-setup.tar.gz}"

if [[ ! -d "$CLAUDE_DIR" ]]; then
  echo "Error: $CLAUDE_DIR does not exist" >&2
  exit 1
fi

# Portable config paths (relative to ~/.claude)
INCLUDE_PATHS=()

for item in \
  CLAUDE.md \
  settings.json \
  manifest.json \
  commands \
  agents \
  skills \
  hooks
do
  if [[ -e "$CLAUDE_DIR/$item" ]]; then
    INCLUDE_PATHS+=(".claude/$item")
  else
    echo "Warning: $CLAUDE_DIR/$item not found, skipping" >&2
  fi
done

if [[ ${#INCLUDE_PATHS[@]} -eq 0 ]]; then
  echo "Error: nothing to export" >&2
  exit 1
fi

echo "Exporting Claude Code setup..."
echo "  Source: $CLAUDE_DIR"
echo "  Output: $OUTPUT"
echo ""
echo "Including:"
for p in "${INCLUDE_PATHS[@]}"; do
  echo "  $p"
done

tar -czf "$OUTPUT" -C "$HOME" "${INCLUDE_PATHS[@]}"

echo ""
echo "Done. Tarball size: $(du -h "$OUTPUT" | cut -f1)"
echo ""
echo "To import on a remote box:"
echo "  scp $OUTPUT user@remote-host-domain:/path/to/nanoclaw/scripts/"
echo "  ssh user@remote-host-domain"
echo "  cd /path/to/nanoclaw/scripts/"
echo "  ./import-claude-setup.sh"
