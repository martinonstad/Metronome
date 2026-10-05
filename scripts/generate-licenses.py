#!/usr/bin/env python3
"""Collects the license notices of the libraries that ship inside the app into
android/app/src/main/assets/licenses.txt, which the About screen shows.

  scripts/generate-licenses.sh           rewrite the file
  scripts/generate-licenses.sh --check   fail if it is out of date (used by scripts/test-all.sh)

Rust crates: the license files in each crate's package (cargo metadata, Android target, normal
dependencies of metronom-ffi). Android libraries: the license named in each one's POM, from
Gradle's cache (releaseRuntimeClasspath). Identical texts are written once.
Format of the file: blocks that start with a line `## title`; the text follows.
"""
import collections
import glob
import json
import os
import pathlib
import re
import subprocess
import sys
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "android/app/src/main/assets/licenses.txt"
LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|UNLICENSE)([-._A-Za-z0-9]*)$", re.I)
APACHE_MARK = "TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION"
# JetBrains publishes its libraries under the Apache License 2.0; Gradle's cache does not always
# keep their POMs, so the license cannot be read from there.
APACHE_PUBLISHERS = ("org.jetbrains:", "org.jetbrains.kotlin:", "org.jetbrains.kotlinx:")
MPL_TEXT = ROOT / "scripts/licenses/MPL-2.0.txt"  # verbatim from github.com/mozilla/uniffi-rs/LICENSE


def clean(text: str) -> str:
    return "\n".join(line.rstrip() for line in text.replace("\r\n", "\n").strip().split("\n"))


def rust_crates():
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--filter-platform", "aarch64-linux-android"],
            cwd=ROOT / "core", check=True, capture_output=True, text=True,
        ).stdout
    )
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = next(i for i, p in packages.items() if p["name"] == "metronom-ffi")
    seen, stack = set(), [root]
    while stack:
        i = stack.pop()
        if i in seen:
            continue
        seen.add(i)
        for dep in nodes[i]["deps"]:
            if any(k.get("kind") is None for k in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    own = {"metronom-core", "metronom-ffi"}
    return sorted(
        (packages[i] for i in seen if packages[i]["name"] not in own),
        key=lambda p: (p["name"], p["version"]),
    )


def rust_texts(crates):
    """(text -> crates) for every license file, and crates that have none."""
    by_text = collections.defaultdict(list)
    without = []
    for crate in crates:
        folder = pathlib.Path(crate["manifest_path"]).parent
        files = sorted(f for f in folder.iterdir() if f.is_file() and LICENSE_FILE.match(f.name))
        if not files:
            without.append(crate)
        for f in files:
            by_text[clean(f.read_text(encoding="utf-8", errors="replace"))].append(f"{crate['name']} {crate['version']}")
    return by_text, without


def gradle_dependencies():
    out = subprocess.run(
        ["./gradlew", "--console=plain", "--quiet", ":app:dependencies", "--configuration", "releaseRuntimeClasspath"],
        cwd=ROOT / "android", check=True, capture_output=True, text=True,
    ).stdout
    found = {}
    for line in out.splitlines():
        m = re.search(r"[+\\]--- ([\w.\-]+):([\w.\-]+)(?::([\w.\-]+))?(?: -> ([\w.\-]+))?", line)
        if m:
            group, artifact, declared, resolved = m.groups()
            version = resolved or declared
            if version:
                found[(group, artifact)] = version
    return sorted((g, a, v) for (g, a), v in found.items())


def pom_licenses(group, artifact, version, depth=0):
    home = pathlib.Path(os.path.expanduser("~/.gradle/caches/modules-2/files-2.1"))
    poms = sorted(glob.glob(str(home / group / artifact / version / "*" / f"{artifact}-{version}.pom")))
    if not poms:
        return []
    root = ET.parse(poms[0]).getroot()
    ns = {"m": "http://maven.apache.org/POM/4.0.0"}
    names = [e.text.strip() for e in root.findall("m:licenses/m:license/m:name", ns) if e.text]
    if not names and depth < 3:
        parent = root.find("m:parent", ns)
        if parent is not None:
            g, a, v = (parent.findtext(f"m:{k}", namespaces=ns) for k in ("groupId", "artifactId", "version"))
            if g and a and v:
                return pom_licenses(g, a, v, depth + 1)
    return names


def build() -> str:
    crates = rust_crates()
    by_text, without = rust_texts(crates)
    apache_texts = [t for t in by_text if APACHE_MARK in t]
    apache = max(apache_texts, key=lambda t: len(by_text[t])) if apache_texts else ""

    blocks = []
    blocks.append(("Metronom", clean((ROOT / "LICENSE").read_text(encoding="utf-8"))))
    blocks.append((
        "About these notices",
        "Metronom is built with the open-source libraries listed here. Their licenses ask for these\n"
        "notices to come with the app. The file is made by scripts/generate-licenses.sh from the\n"
        "libraries' own license files.",
    ))

    android = gradle_dependencies()
    apache_libs, other, unknown = [], collections.defaultdict(list), []
    for g, a, v in android:
        names = pom_licenses(g, a, v)
        label = f"{g}:{a}:{v}"
        if not names and label.startswith(APACHE_PUBLISHERS):
            apache_libs.append(label)
        elif not names:
            unknown.append(label)
        elif any("apache" in n.lower() for n in names):
            apache_libs.append(label)  # dual-licensed ones (JNA) are used under Apache-2.0
        else:
            for n in names:
                other[n].append(label)
    if apache_libs:
        blocks.append((
            f"Android libraries, Apache License 2.0 ({len(apache_libs)})",
            "Used under the Apache License, Version 2.0:\n" + "\n".join(apache_libs) + "\n\n" + apache,
        ))
    for name, libs in sorted(other.items()):
        blocks.append((f"Android libraries, {name} ({len(libs)})", "License named by their packages:\n" + "\n".join(libs)))
    if unknown:
        blocks.append(("Android libraries, license not found", "\n".join(unknown)))

    groups = sorted(by_text.items(), key=lambda kv: (sorted(kv[1])[0], kv[0]))
    for text, users in groups:
        if text == apache:
            title = f"Rust libraries, Apache License 2.0 ({len(users)})"
            blocks.append((title, "Used by: " + ", ".join(sorted(users)) + "\n\n(The text is the one above.)"))
            continue
        first = next((l.strip() for l in text.split("\n") if l.strip()), "License")[:50]
        some = ", ".join(sorted(users)[:3]) + (", …" if len(users) > 3 else "")
        blocks.append((f"Rust libraries, {first} ({some})", "Used by: " + ", ".join(sorted(users)) + "\n\n" + text))
    mpl = [c for c in without if (c.get("license") or "") == "MPL-2.0"]
    rest = [c for c in without if c not in mpl]
    if mpl:
        blocks.append((
            f"Rust libraries, Mozilla Public License 2.0 ({len(mpl)})",
            "UniFFI, used unmodified: " + ", ".join(f"{c['name']} {c['version']}" for c in mpl) + "\n"
            "Their source code is available at https://github.com/mozilla/uniffi-rs\n"
            "and https://crates.io/crates/uniffi.\n\n" + clean(MPL_TEXT.read_text(encoding="utf-8")),
        ))
    if rest:
        blocks.append((
            "Rust libraries without a license file in their package",
            "The license named in their package metadata applies. Where it says MIT OR Apache-2.0, they\n"
            "are used under the Apache License 2.0 (the text is above).\n"
            + "\n".join(f"{c['name']} {c['version']}: {c.get('license') or 'not declared'}" for c in rest),
        ))
    return "\n\n".join(f"## {title}\n{body}" for title, body in blocks) + "\n"


def main():
    text = build()
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != text:
            sys.exit("android/app/src/main/assets/licenses.txt is out of date: run scripts/generate-licenses.sh")
        print(f"licenses.txt is up to date ({len(text)} bytes)")
        return
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)} ({len(text)} bytes, {text.count(chr(10) + '## ') + 1} blocks)")


if __name__ == "__main__":
    main()
