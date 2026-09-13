#!/bin/zsh
# Maintenance status probe. Run: ./repo-status.sh
# Requires: gh (authenticated), jq
REPOS=(
  wala/WALA
  soot-oss/soot
  soot-oss/SootUp
  plast-lab/doop
  SVF-tools/SVF
  facebook/infer
  cs-au-dk/TAJS
  cs-au-dk/jelly
  vitsalis/PyCG
  SMAT-Lab/Scalpel
  sourcegraph/scip-typescript
  rust-lang/rust-analyzer
  golang/tools
)
printf "%-30s %-12s %-22s %-12s %s\n" REPO STARS LAST_PUSH OPEN_ISSUES LATEST_RELEASE
for r in $REPOS; do
  j=$(gh api "repos/$r" 2>/dev/null) || { printf "%-30s %s\n" "$r" "NOT FOUND"; continue; }
  rel=$(gh api "repos/$r/releases/latest" --jq '.tag_name + " (" + (.published_at|split("T")[0]) + ")"' 2>/dev/null || echo "none")
  printf "%-30s %-12s %-22s %-12s %s\n" \
    "$r" \
    "$(echo $j | jq -r .stargazers_count)" \
    "$(echo $j | jq -r .pushed_at | cut -dT -f1)" \
    "$(echo $j | jq -r .open_issues_count)" \
    "$rel"
done
