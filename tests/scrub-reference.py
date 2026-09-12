#!/usr/bin/env python3
"""Normalise a captured command's output for comparison across a change.

Every rule here exists because the value genuinely differs between two runs of
an unchanged tool. Nothing else is touched: counts, ordering, site identities
and wording are the things under comparison, and normalising any of them would
hide exactly what the capture exists to catch.
"""
import re
import sys

# The continuation lines of an argparse usage block are indented to the width
# of the program name, so two names of different lengths shift every wrapped
# line sideways. That is the name's length, not its content.
USAGE_CONTINUATION_INDENT = "  "

# The old name is deliberate and must survive a future rename: this has to
# normalise both names for a comparison that spans one. A bulk substitution ate
# the equivalent line in the shell version once already, leaving the pattern
# matching one name twice.
TOOL_NAMES = re.compile(r"codesweep|resweep")

SUBSTITUTIONS = [
    # Wall-clock time.
    (re.compile(r"\d{4}-\d{2}-\d{2}T[\d:+-]+"), "TIMESTAMP"),
    # Temporary directories, absolute and relative.
    (re.compile(r"/var/folders/[^ \"]*"), "TMP"),
    (re.compile(r"/private/tmp/[^ \"]*"), "TMP"),
    (re.compile(r"\.\./tmp\.[A-Za-z0-9]+"), "TMPREL"),
    # The ledger filename ends in a hash of the repository's absolute path,
    # which lands in a fresh temporary directory on every run. The scheme
    # around it is left visible, because the port has to reproduce that.
    (re.compile(r"-[0-9a-f]{8}\.db"), "-PATHHASH.db"),
]


def main() -> int:
    paths = dict(pair.split("=", 1) for pair in sys.argv[1:])
    text = sys.stdin.read()
    for value, name in sorted(paths.items(), key=lambda kv: -len(kv[0])):
        text = text.replace(value, name)
    for pattern, replacement in SUBSTITUTIONS:
        text = pattern.sub(replacement, text)
    text = TOOL_NAMES.sub("TOOLNAME", text)

    out = []
    in_usage = False
    for line in text.split("\n"):
        if line.startswith("usage:"):
            in_usage = True
            out.append(line)
            continue
        if in_usage and line[:1] == " ":
            out.append(USAGE_CONTINUATION_INDENT + line.lstrip(" "))
            continue
        in_usage = False
        out.append(line)
    sys.stdout.write("\n".join(out))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
