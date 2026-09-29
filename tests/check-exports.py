#!/usr/bin/env python3
"""The shared library exports its C API and nothing else."""

import subprocess
import sys

lib = sys.argv[1]
out = subprocess.run(
    ["nm", "-D", "--defined-only", lib], check=True, capture_output=True, text=True
).stdout
names = [line.split()[-1] for line in out.splitlines() if line.strip()]
bad = [n for n in names if not n.startswith("rotulus_")]
if bad:
    print("exported beyond the C API:", *bad[:20], sep="\n  ")
    sys.exit(1)
if "rotulus_view_new" not in names:
    print("rotulus_view_new is not exported")
    sys.exit(1)
print(len(names), "symbols, all rotulus_*")
