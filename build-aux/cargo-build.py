#!/usr/bin/env python3
"""Build the rotulus crate as a static library for Meson to link.

The crate itself is rlib-only, so that an application which bundles its
Rust into a staticlib of its own doesn't get a second copy of the C
exports. `cargo rustc --crate-type staticlib` asks for the archive for
this build alone.
"""

import argparse
import filecmp
import os
import shutil
import subprocess
import sys

p = argparse.ArgumentParser()
p.add_argument("--cargo", required=True)
p.add_argument("--source-root", required=True)
p.add_argument("--target-dir", required=True)
p.add_argument("--release", action="store_true")
p.add_argument("--features", default="")
p.add_argument("--output", required=True)
a = p.parse_args()

cmd = [
    a.cargo, "rustc",
    "--manifest-path", os.path.join(a.source_root, "Cargo.toml"),
    "--package", "rotulus",
    "--lib",
    "--crate-type", "staticlib",
    "--locked",
    "--target-dir", a.target_dir,
]
if a.release:
    cmd.append("--release")
if a.features:
    cmd += ["--features", a.features]

# From the source root, so rustup finds rust-toolchain.toml.
r = subprocess.run(cmd, cwd=a.source_root)
if r.returncode != 0:
    sys.exit(r.returncode)

built = os.path.join(a.target_dir, "release" if a.release else "debug", "librotulus.a")
# Copy only a changed archive, so a no-op cargo run doesn't relink.
if not (os.path.exists(a.output) and filecmp.cmp(built, a.output, shallow=False)):
    shutil.copyfile(built, a.output)
