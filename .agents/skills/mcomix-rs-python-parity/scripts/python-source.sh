#!/usr/bin/env bash
# Extract (or inspect) the original Python MComix3 sources from git history.
#
# The Python implementation was removed in commit bdbc2e1; its parent is the
# last commit that contained it. This script locates that commit automatically.
#
# Usage:
#   python-source.sh                # print the reference commit + module list
#   python-source.sh <DIR>          # extract the mcomix/ tree into DIR
#   python-source.sh -f FILE        # cat one file (path inside the repo, e.g. mcomix/main.py)
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

deletion_commit="$(git log --diff-filter=D --format=%H -1 -- mcomix/main.py || true)"
if [ -z "$deletion_commit" ]; then
    ref="HEAD"
else
    ref="$(git rev-parse "${deletion_commit}^")"
fi

case "${1:-}" in
    -f)
        [ $# -ge 2 ] || { echo "usage: $0 -f <path>" >&2; exit 2; }
        git show "$ref:$2"
        ;;
    "")
        echo "reference commit: $ref"
        echo "  $(git log --format='%h %s' -1 "$ref")"
        echo
        echo "modules:"
        git ls-tree -r --name-only "$ref" | grep '^mcomix/.*\.py$' | sed 's/^/  /'
        ;;
    *)
        dest="$1"
        mkdir -p "$dest"
        git archive "$ref" mcomix | tar -x -C "$dest"
        echo "extracted mcomix/ into $dest (reference commit $ref)"
        ;;
esac
