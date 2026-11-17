#!/usr/bin/env python3
"""Write LICENSES.txt, the notices file that ships in the download zip.

It holds this project's licence, NickelHook's, and the licence of every Rust
crate in cli/Cargo.lock (the sync program links them statically). Crate
licence texts are read from the published .crate archives on crates.io, so no
Rust toolchain is needed. Run it again whenever cli/Cargo.lock changes:

    python3 scripts/third_party_licenses.py
"""
import io
import re
import sys
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "LICENSES.txt"
LICENCE_FILE = re.compile(r"^[^/]+/(LICEN[CS]E|COPYING|NOTICE|UNLICENSE|COPYRIGHT)[^/]*$", re.I)
RULE = "=" * 78


def crates():
    lock = (ROOT / "cli" / "Cargo.lock").read_text(encoding="utf-8")
    found = []
    for block in lock.split("[[package]]")[1:]:
        name = re.search(r'^name = "([^"]+)"', block, re.M).group(1)
        version = re.search(r'^version = "([^"]+)"', block, re.M).group(1)
        if 'source = "registry+' in block:  # skips the CLI itself
            found.append((name, version))
    return sorted(set(found))


def fetch(name, version):
    url = f"https://static.crates.io/crates/{name}/{name}-{version}.crate"
    request = urllib.request.Request(url, headers={"User-Agent": "license-notices-builder"})
    with urllib.request.urlopen(request, timeout=60) as response:
        data = response.read()
    expression, texts = "", []
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for member in archive:
            if not member.isfile():
                continue
            if re.match(r"^[^/]+/Cargo\.toml$", member.name):
                manifest = archive.extractfile(member).read().decode("utf-8", "replace")
                match = re.search(r'^license\s*=\s*"([^"]+)"', manifest, re.M)
                expression = match.group(1) if match else "see licence text"
            elif LICENCE_FILE.match(member.name) and member.size < 200_000:
                text = archive.extractfile(member).read().decode("utf-8", "replace")
                texts.append(text.replace("\r\n", "\n").strip())
    return expression, texts


def main():
    project = (ROOT / "LICENSE").read_text(encoding="utf-8").strip()
    nickelhook = (ROOT / "hook" / "NickelHook" / "LICENSE").read_text(encoding="utf-8").strip()
    listing, by_text, missing = [], {}, []
    todo = crates()
    for index, (name, version) in enumerate(todo, 1):
        print(f"[{index}/{len(todo)}] {name} {version}", file=sys.stderr)
        expression, texts = fetch(name, version)
        listing.append(f"  {name} {version}  ({expression})")
        if not texts:
            missing.append(f"{name} {version}")
        for text in texts:
            by_text.setdefault(text, []).append(f"{name} {version}")

    out = [
        "Bookshelf Kobo Sync - licences and notices",
        "",
        "This download contains software under the licences below.",
        "Bookshelf Kobo Sync is not affiliated with or endorsed by Rakuten Kobo.",
        "",
        RULE,
        "Bookshelf Kobo Sync (a fork of NickelHardcover by StrayRose)",
        RULE,
        "",
        project,
        "",
        RULE,
        "NickelHook",
        RULE,
        "",
        nickelhook,
        "",
        RULE,
        "Rust libraries built into the sync program",
        RULE,
        "",
        "The list comes from the project's lock file, so it also names libraries",
        "that are only used when building for other systems.",
        "",
        *listing,
        "",
    ]
    if missing:
        out += ["Published without a licence file (the licence named above applies):", ""]
        out += [f"  {entry}" for entry in missing] + [""]
    for text, users in sorted(by_text.items(), key=lambda item: (-len(item[1]), item[1][0])):
        out += [RULE, "Applies to: " + ", ".join(users), RULE, "", text, ""]
    OUT.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes, {len(todo)} crates, "
          f"{len(by_text)} distinct licence texts, {len(missing)} without a file)", file=sys.stderr)


if __name__ == "__main__":
    main()
