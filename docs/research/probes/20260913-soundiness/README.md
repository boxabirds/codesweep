# Probe scripts for docs/research/20260913-soundiness-and-reporting-the-unresolved.md

Run from this directory. Requires: gh (authenticated), jq, python3, pdftotext, npx.

- `repo-status.sh`   maintenance status of every analyser named in the doc, via GitHub API
- `oa.sh "<title>"`  OpenAlex title search; prints year, venue, DOI, OA pdf url, abstract
- `jellyprobe/probe.js`, `jellyprobe/probe2.js`
                     the two JavaScript files used to measure jelly's residual reporting.
                     Reproduce with:
                       npx --yes @cs-au-dk/jelly@latest --zeros --warnings-unsupported \
                         --callgraph-json cg.json probe.js
                       npx --yes @cs-au-dk/jelly@latest --zeros --callgraph-json cg2.json probe2.js

Fetched sources kept here for reference (all from raw.githubusercontent.com @ master on
2026-09-13): wala-*.java, jelly-*.ts, fragmentstate.ts, diagnostics.ts, tajs-*.java,
doop-soundmay.dl, svf-*.cpp, infer-skipped.ml, gopls-rename.go, gopls-transform.md,
ra-*.rs, uvb.properties, uvp.java, usageinfo.java, brp.java, rse.java,
jdt-refactoring.properties, soot-scene.java, soot-ofcgb.java.
Papers: Soundiness-CACM.pdf, fm2012.pdf, vmcai15.pdf, soap17.pdf.

## What was removed before committing

This directory originally sat under an untracked `scratchpad/` inside the repo
and held 6.7MB, most of it downloaded PDFs of the cited papers plus extracted
text and a directory tree dump. The papers are copyrighted and re-fetchable
from the URLs cited in the research note, so only the scripts, probe sources
and small JSON captures were kept. `scratchpad/` is now in `.gitignore`.
