#!/usr/bin/env bash
# Publish this project to GitHub as the "PlayCraft" repository (macOS / Linux).
# Needs git and the GitHub CLI (https://cli.github.com). Sign-in happens in the browser;
# no password is typed here or stored.
set -euo pipefail
cd "$(dirname "$0")/.."
NAME="${1:-PlayCraft}"

gh auth status >/dev/null 2>&1 || gh auth login --hostname github.com --git-protocol https --web
[ -d .git ] || git init -b main
OWNER=$(gh api user --jq .login)
git config user.name >/dev/null || git config user.name "$OWNER"
git config user.email >/dev/null || git config user.email "$OWNER@users.noreply.github.com"
git add -A
git diff --cached --quiet || git commit -m "PlayCraft: open-source DVD, CD and Blu-ray player"
if gh repo view "$OWNER/$NAME" >/dev/null 2>&1; then
  git remote get-url origin >/dev/null 2>&1 || git remote add origin "https://github.com/$OWNER/$NAME.git"
  git push -u origin main
else
  gh repo create "$NAME" --public --source . --remote origin --push \
    --description "Free, open-source DVD, CD and Blu-ray player for Windows, macOS and Linux"
fi
echo "Done: https://github.com/$OWNER/$NAME"
