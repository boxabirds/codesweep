#!/bin/zsh
# OpenAlex title search. Usage: ./oa.sh "paper title"
curl -sL --get "https://api.openalex.org/works" \
  --data-urlencode "filter=title.search:$1" \
  -d 'per-page=5' --data-urlencode 'mailto=julian.harris@gmail.com' \
| python3 -c "
import json,sys
d=json.load(sys.stdin)
for w in d.get('results',[]):
    print('---')
    print(w.get('publication_year'),'|',(w.get('primary_location') or {}).get('source',{}) and ((w.get('primary_location') or {}).get('source') or {}).get('display_name'),'|',w.get('title'))
    print('  doi:',w.get('doi'),' oa:',(w.get('best_oa_location') or {}).get('pdf_url'))
    inv=w.get('abstract_inverted_index')
    if inv:
        n=max(max(v) for v in inv.values())+1
        arr=['']*n
        for k,vs in inv.items():
            for v in vs: arr[v]=k
        print('  abs:',' '.join(arr)[:1100])
"
