#!/usr/bin/env python3
"""
Project Nocturne (MK-IV) — Reproducible SBOM (CycloneDX 1.5) & SHA-256 Checksum Generator.
Section 4.6: Supply Chain, Code Signing & Release Integrity.
"""

import hashlib
import json
import os
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent

TRACKED_PATHS = [
    "Cargo.toml",
    "deny.toml",
    "package.json",
    "rust-toolchain.toml",
    "rustfmt.toml",
    ".cargo/config.toml",
    "src/index.html",
    "src/styles/main.css",
    "src/js/app.js",
    "src-tauri/Cargo.toml",
    "src-tauri/build.rs",
    "src-tauri/tauri.conf.json",
    "src-tauri/Entitlements.plist",
    "src-tauri/nocturne-airgap.sb",
    "src-tauri/capabilities/default.json",
    "src-tauri/src/main.rs",
    "src-tauri/src/lib.rs",
    "src-tauri/src/audio/mod.rs",
    "src-tauri/src/db/mod.rs",
    "src-tauri/src/display/mod.rs",
    "src-tauri/src/ipc/mod.rs",
    "src-tauri/src/panic/mod.rs",
    "src-tauri/src/security/mod.rs",
    "src-tauri/src/telemetry/mod.rs",
    "src-tauri/src/telemetry/profiler.rs",
    "src-tauri/src/tray/mod.rs",
]

CRATE_COMPONENTS = [
    {"name": "tauri", "version": "2.0.0", "purl": "pkg:cargo/tauri@2.0.0", "license": "Apache-2.0 OR MIT"},
    {"name": "tauri-build", "version": "2.0.0", "purl": "pkg:cargo/tauri-build@2.0.0", "license": "Apache-2.0 OR MIT"},
    {"name": "serde", "version": "1.0", "purl": "pkg:cargo/serde@1.0", "license": "MIT OR Apache-2.0"},
    {"name": "serde_json", "version": "1.0", "purl": "pkg:cargo/serde_json@1.0", "license": "MIT OR Apache-2.0"},
    {"name": "cpal", "version": "0.15", "purl": "pkg:cargo/cpal@0.15", "license": "Apache-2.0"},
    {"name": "rusqlite", "version": "0.31", "purl": "pkg:cargo/rusqlite@0.31", "license": "MIT"},
    {"name": "sysinfo", "version": "0.30", "purl": "pkg:cargo/sysinfo@0.30", "license": "MIT"},
]


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    checksum_entries = []
    file_components = []

    for rel_path in sorted(TRACKED_PATHS):
        full_path = ROOT_DIR / rel_path
        if not full_path.exists():
            print(f"[ERROR] Missing expected artifact: {rel_path}", file=sys.stderr)
            return 1
        digest = sha256_file(full_path)
        checksum_entries.append(f"{digest}  {rel_path}")
        file_components.append(
            {
                "type": "file",
                "name": rel_path,
                "hashes": [{"alg": "SHA-256", "content": digest}],
            }
        )

    # Write reproducible SHA256SUMS.txt
    sha256sums_path = ROOT_DIR / "SHA256SUMS.txt"
    sha256sums_content = "\n".join(checksum_entries) + "\n"
    sha256sums_path.write_text(sha256sums_content, encoding="utf-8")

    # Build deterministic CycloneDX 1.5 SBOM
    components = []
    for crate in CRATE_COMPONENTS:
        components.append(
            {
                "type": "library",
                "name": crate["name"],
                "version": crate["version"],
                "purl": crate["purl"],
                "licenses": [{"license": {"name": crate["license"]}}],
            }
        )
    components.extend(file_components)

    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "name": "nocturne-daemon",
                "version": "1.0.4",
                "description": "Project Nocturne (MK-IV) — Ambient Workstation Poltergeist Daemon",
            },
            "properties": [
                {"name": "nocturne:security:airgap", "value": "ZERO-EGRESS-AIR-GAP"},
                {"name": "nocturne:macos:hardened-runtime", "value": "true"},
                {"name": "nocturne:windows:cfg-aslr", "value": "DYNAMICBASE,HIGHENTROPYVA,CFG"},
            ],
        },
        "components": components,
    }

    sbom_path = ROOT_DIR / "sbom.cdx.json"
    sbom_path.write_text(json.dumps(sbom, indent=2) + "\n", encoding="utf-8")

    print(f"Generated {sha256sums_path.name} ({len(checksum_entries)} artifacts) and {sbom_path.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
