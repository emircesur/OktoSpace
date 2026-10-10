#!/usr/bin/env python3
"""Writes OktoSpace's winget manifests for one release (the three files winget-pkgs expects).

    python3 packaging/winget/manifests.py --version 0.1.3 --sums SHA256SUMS.txt --out winget

The installers are the per-user Inno Setup builds; winget finds an installed OktoSpace by the
setup's uninstall entry (its AppId), so `winget upgrade` works however OktoSpace was installed.
"""

import argparse
import pathlib
import sys

IDENTIFIER = "emircesur.OktoSpace"
MANIFEST_VERSION = "1.9.0"
# AppId in packaging/windows/oktospace.iss; Inno Setup names the uninstall entry "<AppId>_is1".
PRODUCT_CODE = "{8C2F6E6A-5B1D-4C51-9F0B-6D3B2A9E7C41}_is1"
PUBLISHER = "OktoSpace contributors"
ARCHES = ["x64", "arm64"]


def schema(kind):
    return f"# yaml-language-server: $schema=https://aka.ms/winget-manifest.{kind}.{MANIFEST_VERSION}.schema.json\n\n"


def quote(s):
    return "'" + s.replace("'", "''") + "'"


def parse_sums(text):
    sums = {}
    for line in text.splitlines():
        parts = line.split()
        if len(parts) >= 2:
            sums[parts[-1].lstrip("*")] = parts[0]
    return sums


def manifests(version, tag, repo, sums):
    base = f"https://github.com/{repo}/releases/download/{tag}"
    home = f"https://github.com/{repo}"
    installers = []
    for arch in ARCHES:
        name = f"oktospace-{version}-windows-{arch}-setup.exe"
        if name not in sums:
            sys.exit(f"{name} isn't in the checksums; was it built?")
        installers.append(
            f"- Architecture: {arch}\n"
            f"  InstallerUrl: {base}/{name}\n"
            f"  InstallerSha256: {sums[name].upper()}\n"
        )
    head = f"PackageIdentifier: {IDENTIFIER}\nPackageVersion: {version}\n"
    return {
        f"{IDENTIFIER}.yaml": schema("version")
        + head
        + f"DefaultLocale: en-US\nManifestType: version\nManifestVersion: {MANIFEST_VERSION}\n",
        f"{IDENTIFIER}.installer.yaml": schema("installer")
        + head
        + "InstallerType: inno\n"
        + "Scope: user\n"
        + "InstallModes:\n- interactive\n- silent\n- silentWithProgress\n"
        + "UpgradeBehavior: install\n"
        + f"ProductCode: {quote(PRODUCT_CODE)}\n"
        + "AppsAndFeaturesEntries:\n"
        + f"- DisplayName: OktoSpace\n  Publisher: {PUBLISHER}\n  ProductCode: {quote(PRODUCT_CODE)}\n"
        + "Installers:\n"
        + "".join(installers)
        + f"ManifestType: installer\nManifestVersion: {MANIFEST_VERSION}\n",
        f"{IDENTIFIER}.locale.en-US.yaml": schema("defaultLocale")
        + head
        + "PackageLocale: en-US\n"
        + f"Publisher: {PUBLISHER}\n"
        + f"PublisherUrl: {home}\n"
        + f"PublisherSupportUrl: {home}/issues\n"
        + "PackageName: OktoSpace\n"
        + f"PackageUrl: {home}\n"
        + "License: MIT OR Apache-2.0\n"
        + f"LicenseUrl: {home}/blob/main/LICENSE-MIT\n"
        + "ShortDescription: Install, update and build apps from GitHub\n"
        + "Description: |-\n"
        + "  OktoSpace installs, updates and builds apps from GitHub: an App library of open-source\n"
        + "  apps and any repository you add, checksum-verified downloads, building from source,\n"
        + "  update channels and rollback, and tools for IT and classrooms.\n"
        + "Moniker: oktospace\n"
        + "Tags:\n- github\n- installer\n- updater\n- package-manager\n- open-source\n"
        + f"ReleaseNotesUrl: {home}/releases/tag/{tag}\n"
        + f"ManifestType: defaultLocale\nManifestVersion: {MANIFEST_VERSION}\n",
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", required=True, help="e.g. 0.1.3")
    ap.add_argument("--tag", help="the release tag (default: v<version>)")
    ap.add_argument("--repo", default="emircesur/OktoSpace")
    ap.add_argument("--sums", required=True, help="the release's SHA256SUMS.txt")
    ap.add_argument("--out", required=True, help="folder to write the manifests to")
    a = ap.parse_args()
    files = manifests(a.version, a.tag or f"v{a.version}", a.repo, parse_sums(pathlib.Path(a.sums).read_text()))
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    for name, text in files.items():
        (out / name).write_text(text)
        print(out / name)


if __name__ == "__main__":
    main()
