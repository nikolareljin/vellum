#!/usr/bin/env bash
set -euo pipefail

# Rejects <img src="https://..."> tags in Markdown files.
# Use native Markdown syntax instead: ![alt](url)
# HTML img tags with external src may silently fail to render in many viewers.

bad=$(grep -rn '<img[^>]*src="https://' \
  --include="*.md" \
  --exclude-dir=".remember" \
  --exclude-dir="scripts/script-helpers" \
  . 2>/dev/null || true)

if [[ -n "$bad" ]]; then
  echo "ERROR: Found <img> tags with external src= in Markdown files."
  echo "Use native Markdown image syntax instead:  ![alt](url)"
  echo ""
  echo "Offending lines:"
  echo "$bad"
  echo ""
  echo "Acceptable HTML form (GitHub-only, size control):"
  echo '  <img width="N" height="N" alt="…" src="https://…" loading="lazy" />'
  echo "Only use HTML form when width/height are required AND the file is GitHub-only."
  exit 1
fi

echo "OK: no external <img src> tags found in Markdown files."
