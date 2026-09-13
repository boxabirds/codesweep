#!/bin/bash
# score one rename case: $1=repodir $2=sha $3=oldname
set -e
S="$(dirname "$0")"; R="$1"; SHA="$2"; OLD="$3"
cd "$R" || exit 3
git checkout -q "${SHA}^" 2>/dev/null || exit 3
# ground truth: parent-side lines the commit removed that mention OLD
git diff "${SHA}^" "$SHA" -U0 -- '*.java' 2>/dev/null | OLD="$OLD" python3 -c "
import sys,re,os
old=os.environ['OLD']; f=None; ln=0; out=[]
for line in sys.stdin:
    if line.startswith('--- a/'): f=line[6:].strip()
    elif line.startswith('@@'):
        m=re.search(r'-(\d+)', line); ln=int(m.group(1)) if m else ln
    elif line.startswith('-') and not line.startswith('---'):
        if re.search(r'\b'+re.escape(old)+r'\b', line): out.append(f'{f}:{ln}')
        ln+=1
    elif not line.startswith('+'): ln+=1
print('\n'.join(sorted(set(out))))
" > /tmp/t.txt
[ -s /tmp/t.txt ] || exit 4
grep -rnw "$OLD" --include='*.java' . 2>/dev/null | sed 's|^\./||' | cut -d: -f1,2 | sort -u > /tmp/g.txt
cat > /tmp/r.yml <<EOF
id: c
language: java
rule:
  any:
    - pattern: \$R.$OLD(\$\$\$A)
    - pattern: $OLD(\$\$\$A)
    - kind: method_declaration
      has: {field: name, regex: "^$OLD\$"}
EOF
rm -rf /tmp/rsroot && mkdir -p /tmp/rsroot
~/expts/codesweep/target/release/resweep census c --rule /tmp/r.yml --scope "$PWD" --root /tmp/rsroot >/dev/null 2>&1 || exit 5
~/expts/codesweep/target/release/resweep manifest c --root /tmp/rsroot 2>/dev/null | python3 -c "
import sys,re
f=None;out=[]
for l in sys.stdin:
    l=l.rstrip()
    if l.endswith('.java'): f=l.strip()
    m=re.match(r'\s+[0-9a-f]{16}\s+(\d+)-', l)
    if m and f: out.append(f'{f}:{m.group(1)}')
print('\n'.join(sorted(set(out))))
" | BASE="$(basename $PWD)" python3 -c "
import sys,os
b='/'+os.environ['BASE']+'/'
for l in sys.stdin:
    l=l.strip()
    i=l.find(b)
    print(l[i+len(b):] if i>=0 else l)
" > /tmp/rs.txt
python3 -c "
def L(p):
    try: return [x.strip() for x in open(p) if x.strip()]
    except: return []
def parse(xs):
    out=[]
    for x in xs:
        try:
            f,l=x.rsplit(':',1); out.append((f,int(l)))
        except: pass
    return out
t=parse(L('/tmp/t.txt')); g=parse(L('/tmp/g.txt')); r=parse(L('/tmp/rs.txt'))
TOL=3
def score(got):
    # greedy 1-to-1 match within same file and +/-TOL lines
    remaining=list(t); hit=0
    for f,l in sorted(got):
        best=None
        for i,(tf,tl) in enumerate(remaining):
            if tf==f and abs(tl-l)<=TOL:
                if best is None or abs(tl-l)<abs(remaining[best][1]-l): best=i
        if best is not None:
            remaining.pop(best); hit+=1
    return hit, len(got)-hit, len(t)-hit
gh,gw,gm=score(g); rh,rw,rm=score(r)
print(f'$OLD|{len(t)}|{gh}|{gw}|{gm}|{rh}|{rw}|{rm}')
"
