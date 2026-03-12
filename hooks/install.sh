#!/bin/sh
set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"

for hook in "$REPO_ROOT/hooks/"*; do
    name="$(basename "$hook")"
    [ "$name" = "install.sh" ] && continue
    ln -sf "$hook" "$HOOKS_DIR/$name"
    echo "Installed $name"
done
