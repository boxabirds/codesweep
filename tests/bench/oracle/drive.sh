#!/bin/bash
S=/private/tmp/claude-501/-Users-julian-expts-codesweep/5a3451a3-08d1-4274-aa47-4287622f5489/scratchpad
cd $S/bench
for slug in graphhopper/graphhopper apache/drill mockito/mockito; do
  name=$(basename $slug)
  [ -d "$name" ] || timeout 900 git clone --quiet https://github.com/$slug.git $name 2>/dev/null || { echo "CLONE-FAIL $slug"; continue; }
  python3 -c "
import json
d=json.load(open('$S/bench/renames.json'))
seen=set()
for x in d:
    if '$slug' not in x['repo']: continue
    k=(x['sha'],x['old'],x['new'])
    if k in seen: continue
    seen.add(k); print(x['sha'],x['old'])
" | while read sha old; do
    out=$($S/bench/run-case.sh $S/bench/$name $sha $old 2>/dev/null) && echo "$name|$out" || echo "$name|$old|SKIP"
  done
done
