#!/usr/bin/env bash
set -euo pipefail

# sync-submodules.sh — report and optionally pull/push all utoaudio submodules.
# Read-only by default. Use --pull or --push for write operations.

SUBMODULES=(
  "vendor/flick|moss-apps/Flick"
  "apps/desktop/src/lib/vendor/amll|amll-dev/applemusic-like-lyrics"
  "apps/desktop/src/lib/vendor/liquid-glass|danilofiumi/liquid-glass-svelte"
)

MODE="report"

for arg in "$@"; do
  case "$arg" in
    --pull)  MODE="pull" ;;
    --push)  MODE="push" ;;
    *)       echo "Usage: $0 [--pull | --push]" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

BOLD="\033[1m"
GREEN="\033[32m"
YELLOW="\033[33m"
RED="\033[31m"
RESET="\033[0m"

echo -e "${BOLD}=== utoaudio submodule sync ($MODE mode) ===${RESET}"
echo

for entry in "${SUBMODULES[@]}"; do
  path="${entry%%|*}"
  upstream_repo="${entry##*|}"

  echo -e "${BOLD}── ${path}${RESET}"

  cd "$ROOT/$path"

  echo "  upstream: $upstream_repo"

  # Fetch both remotes
  git fetch origin 2>/dev/null || true
  git fetch upstream 2>/dev/null || true

  origin_sha=$(git rev-parse origin/main 2>/dev/null || echo "???")
  upstream_sha=$(git rev-parse upstream/main 2>/dev/null || echo "???")
  head_sha=$(git rev-parse HEAD)

  echo "  HEAD:      ${head_sha:0:9}"
  echo "  origin:    ${origin_sha:0:9}"
  echo "  upstream:  ${upstream_sha:0:9}"

  # Ahead / behind fork (origin)
  behind_origin=$(git rev-list --count HEAD..origin/main 2>/dev/null || echo "?")
  ahead_origin=$(git rev-list --count origin/main..HEAD 2>/dev/null || echo "?")

  if [ "$behind_origin" != "0" ] && [ "$behind_origin" != "?" ]; then
    echo -e "  ${YELLOW}↘ behind fork by $behind_origin commit(s)${RESET}"
  fi
  if [ "$ahead_origin" != "0" ] && [ "$ahead_origin" != "?" ]; then
    echo -e "  ${GREEN}↗ ahead of fork by $ahead_origin commit(s)${RESET}"
  fi

  # Ahead / behind upstream
  behind_up=$(git rev-list --count HEAD..upstream/main 2>/dev/null || echo "?")
  ahead_up=$(git rev-list --count upstream/main..HEAD 2>/dev/null || echo "?")

  if [ "$behind_up" != "0" ] && [ "$behind_up" != "?" ]; then
    echo -e "  ${RED}↘ behind upstream by $behind_up commit(s)${RESET}"
  fi
  if [ "$ahead_up" != "0" ] && [ "$ahead_up" != "?" ]; then
    echo -e "  ${GREEN}↗ ahead of upstream by $ahead_up commit(s)${RESET}"
  fi

  # Diverged?
  if [ "$origin_sha" != "$head_sha" ] && [ "$origin_sha" != "???" ]; then
    echo -e "  ${YELLOW}⚠ HEAD diverged from fork${RESET}"
  fi

  # --pull: merge origin/main into HEAD
  if [ "$MODE" = "pull" ]; then
    if [ "$behind_origin" != "0" ] && [ "$behind_origin" != "?" ]; then
      echo -e "  ${GREEN}→ pulling origin/main…${RESET}"
      git merge origin/main --no-edit || echo -e "  ${RED}✗ merge conflict — resolve manually${RESET}"
    else
      echo "  (up to date with fork, no pull needed)"
    fi
  fi

  # --push: push HEAD to origin/main
  if [ "$MODE" = "push" ]; then
    if [ "$ahead_origin" != "0" ] && [ "$ahead_origin" != "?" ]; then
      echo -e "  ${GREEN}→ pushing to origin/main…${RESET}"
      git push origin main || echo -e "  ${RED}✗ push failed — check GitHub auth${RESET}"
    else
      echo "  (no local commits to push)"
    fi
  fi

  echo
done

cd "$ROOT"
echo -e "${BOLD}=== main repo submodule references ===${RESET}"
git submodule status

echo
echo -e "${BOLD}Done.${RESET} Run with --pull or --push to sync."