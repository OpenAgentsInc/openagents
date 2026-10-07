#!/usr/bin/env bash
# Fails when a surface uses a typeface other than Paper Mono (#10904).
#
# Two checks over the tracked files:
#
# 1. Every tracked font file is a Paper Mono face. The faces live in
#    `crates/paper-mono/fonts/`; the phone hosts copy them at build time.
# 2. No tracked surface file names another font family, a platform's system
#    face, or a generic family other than `monospace`.
#
# Archives are out of scope: recorded evidence under `bench/`, the
# transcript archive under `docs/transcripts/`, research notes, verification
# receipts, and fixtures, which hold third-party pages and recorded output.
# A line that must name another face, such as a fallback chain for glyphs
# Paper Mono lacks, carries the marker `check-fonts: allow` with its reason.
#
# Usage: scripts/check-fonts.sh
set -euo pipefail

root="$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)"
cd "$root"

archive=(
  ':!bench/'
  ':!docs/transcripts/'
  ':!docs/research/'
  ':!**/verification/**'
  ':!**/fixtures/**'
  ':!**/*fixture*'
  ':!vendor/'
  ':!crates/gym/questions/'
  ':!crates/gym/suites/'
  ':!docs/terminal-bench/'
  ':!docs/coder/thoughts-on-a-typesafe-coding-agent/'
  ':!scripts/check-fonts.sh'
)

failed=0

# 1. Font files.
fonts="$(git ls-files -- "${archive[@]}" \
  | grep -iE '\.(ttf|otf|ttc|woff2?|eot|dfont|pfb)$' \
  | grep -vE '(^|/)PaperMono-[A-Za-z]+\.(ttf|woff2)$' || true)"
if [ -n "$fonts" ]; then
  echo "check-fonts: a tracked font file is not Paper Mono:" >&2
  echo "$fonts" >&2
  failed=1
fi

# 2. Family names in surface files: source, styles, markup, native layout
# and build files, and current documentation.
families=(
  'InterVariable' 'FontFamily::(Inter|Geist)' '\bGeist\b' 'JetBrains ?Mono'
  'Fira ?(Mono|Code|Sans)' 'Cascadia' '\bSF ?(Mono|Pro)\b' 'SFMono' '\bMenlo\b'
  '\bMonaco\b' 'Consolas' 'Liberation (Mono|Sans|Serif)' 'DejaVu' 'Helvetica'
  '\bArial\b' 'Segoe UI' 'system-ui' 'ui-monospace' 'ui-sans-serif'
  '-apple-system' 'BlinkMacSystemFont' 'sans-serif' '\bserif\b[;,"]'
  '\bRoboto\b' 'Courier New' 'Times New Roman' '\bGeorgia\b' 'Iosevka' 'Terminus'
  'IBM Plex' 'Source Code Pro' 'Ubuntu Mono' 'Noto Sans Mono' 'Hack\b[ -]?Nerd'
  '"Inter"' "'Inter'" 'Inter,'
  # Native system faces.
  'systemFont' 'monospacedSystemFont' 'monospacedDigitSystemFont'
  'preferredFont\(' 'Font\.system\(' '\.font\(\.system' '\.fontDesign\('
  '\.font\(\.(largeTitle|title|title2|title3|headline|subheadline|body|callout|caption|caption2|footnote)\b'
  'Typeface\.(DEFAULT|DEFAULT_BOLD|MONOSPACE|SANS_SERIF|SERIF)'
  'FontFamily\.(Monospace|Default|SansSerif|Serif|Cursive)'
)
pattern="$(IFS='|'; echo "${families[*]}")"
hits="$(git grep -nIP "$pattern" -- \
  '*.rs' '*.css' '*.html' '*.js' '*.svg' '*.swift' '*.kt' '*.kts' '*.xml' \
  '*.plist' '*.yml' '*.yaml' '*.nix' '*.ini' '*.conf' '*.sh' '*.h' '*.md' \
  "${archive[@]}" \
  | grep -v 'check-fonts: allow' \
  | grep -vE '^[^:]+/PaperMono\.(swift|kt):' || true)"
if [ -n "$hits" ]; then
  echo "check-fonts: a surface names a family other than Paper Mono:" >&2
  echo "$hits" >&2
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  echo "check-fonts: use Paper Mono (crates/paper-mono) on every surface" >&2
  exit 1
fi
echo "check-fonts: every surface uses Paper Mono"
