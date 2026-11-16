#!/bin/sh
# Fetch the design system release and generate the display assets from it.
#
#   ./update-design.sh        # needs gh, uv and network access
#
# Nothing it downloads or generates is committed: .design/ and
# rt-hal/src/generated/ are in .gitignore. Run it again to take a new
# design system release.
set -eu
cd "$(dirname "$0")"
mkdir -p .design
gh release download --repo belikh/design --dir .design --clobber \
    --pattern jupiter.tokens.json --pattern jupiter-fonts.tgz
tar xzf .design/jupiter-fonts.tgz -C .design \
    package/files/jetbrains-mono.woff2 package/files/JetBrainsMono-OFL.txt
uv run --quiet --with pillow --with fonttools --with brotli python assets/gen.py
