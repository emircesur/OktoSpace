#!/usr/bin/env python3
"""Build OktoSpace's App library (catalog.json) from curated lists of open-source apps.

    GITHUB_TOKEN=... python3 tools/library.py LIST_URL... [--catalog catalog.json]

Each LIST_URL is a Markdown list (an "awesome list": `- [Name](link) - description` under `##`
headings), given as a raw file URL or as a GitHub repository (its README is read). An entry's
link is its GitHub repository, or a page that links to it (the repository linked most often on
that page, not counting links every page has).

An app joins the library when its latest release has something OktoSpace can install on Windows,
macOS or Linux, or when it can be built from source into a program (Rust, Go, C/C++, Swift, C#,
Zig projects with a known build system). Entries marked `"curated": true` in the existing
catalog are kept as they are. The `##` headings become the categories.
"""
import argparse
import collections
import concurrent.futures as cf
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request

TOKEN = os.environ.get("GITHUB_TOKEN", "")
GH = re.compile(r"https?://github\.com/([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+)")
NOT_OWNERS = {"sponsors", "features", "about", "topics", "orgs", "marketplace", "apps", "login", "settings",
              "collections", "trending", "pricing", "enterprise", "site", "contact", "readme", "search"}
SKIP_SECTIONS = {"sponsors", "contents", "contributing", "footnotes", "license", "related", "resources"}
ENTRY = re.compile(r"^\s*[-*] \[(.+?)\]\((.+?)\)\s*[-–—:]\s*(.*?)\s*((?:`[^`]*`\s*)*)$")
NATIVE = {"Rust", "Go", "C", "C++", "Swift", "C#", "Zig", "Nim", "Odin", "D", "Haskell", "OCaml", "Crystal"}
BUILD_FILES = [
    ("Cargo.toml", "cargo"), ("go.mod", "go"), ("CMakeLists.txt", "cmake"), ("meson.build", "meson"),
    ("build.zig", "zig"), ("Package.swift", "swift"), ("Makefile", "make"), ("package.json", "npm"),
]
PALETTE = ["#3B82F6", "#10B981", "#F59E0B", "#EF4444", "#8B5CF6", "#EC4899", "#14B8A6", "#F97316"]


def get(url, api=False, tries=3):
    headers = {"User-Agent": "oktospace-library"}
    if api:
        headers["Accept"] = "application/vnd.github+json"
        if TOKEN:
            headers["Authorization"] = f"Bearer {TOKEN}"
    for attempt in range(tries):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=30) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if e.code in (404, 410, 451):
                return None
            if e.code in (403, 429) and api:
                reset = int(e.headers.get("x-ratelimit-reset", "0") or 0)
                time.sleep(min(max(reset - time.time(), 5), 120))
                continue
            time.sleep(2 * (attempt + 1))
        except Exception:
            time.sleep(2 * (attempt + 1))
    return None


def api(path):
    data = get("https://api.github.com/" + path, api=True)
    return json.loads(data) if data else None


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")


def list_text(url):
    m = GH.match(url)
    if m and "raw.githubusercontent.com" not in url:
        readme = api(f"repos/{m[1]}/{m[2]}/readme")
        if readme and readme.get("download_url"):
            url = readme["download_url"]
    data = get(url)
    if not data:
        sys.exit(f"couldn't read the list {url}")
    return data.decode("utf-8", "replace")


def parse(md):
    top, out = None, []
    for line in md.splitlines():
        if line.startswith("## "):
            top = re.sub(r"[^\w&,'+ -]", "", line[3:]).strip()
        m = ENTRY.match(line)
        if m and top and slug(top) not in SKIP_SECTIONS:
            out.append({"name": m[1].strip(), "url": m[2].strip(), "desc": m[3].strip(), "section": top})
    return out


def repos_on_page(html):
    found = collections.Counter()
    for owner, repo in GH.findall(html):
        repo = re.sub(r"\.git$", "", repo)
        if owner.lower() not in NOT_OWNERS and not repo.startswith("."):
            found[f"{owner}/{repo}"] += 1
    return found


def platform_of(name):
    n = name.lower()
    if n.endswith((".appimage", ".deb")) or (n.endswith(".rpm") and not n.endswith(".src.rpm")):
        return "linux"
    if n.endswith((".tar.gz", ".tgz")) and "linux" in n:
        return "linux"
    if n.endswith(".msi") or (n.endswith(".exe") and not n.endswith(".blockmap")):
        return "windows"
    if n.endswith(".zip") and (re.search(r"windows|win64|win32", n) or "win" in re.split(r"[^a-z0-9]+", n)
                               or n.endswith("-portable.zip")):
        return "windows"
    if n.endswith(".dmg"):
        return "macos"
    return None


def inspect(repo):
    info = api(f"repos/{repo}")
    if not info or info.get("archived") or info.get("disabled") or info.get("fork"):
        return None
    full = info["full_name"]
    releases = api(f"repos/{full}/releases?per_page=8") or []
    rel = next((r for r in releases if not r.get("draft") and not r.get("prerelease")), None) or next(
        (r for r in releases if not r.get("draft")), None)
    platforms = sorted({p for a in (rel or {}).get("assets", []) for p in [platform_of(a["name"])] if p})
    root = api(f"repos/{full}/contents") or []
    names = {e.get("name") for e in root if isinstance(e, dict)}
    build = next((b for f, b in BUILD_FILES if f in names), None)
    if build is None and any(n and n.endswith((".sln", ".csproj")) for n in names):
        build = "dotnet"
    return {
        "repo": full,
        "description": (info.get("description") or "").strip(),
        "stars": info.get("stargazers_count", 0),
        "homepage": (info.get("homepage") or "").strip() or None,
        "license": (info.get("license") or {}).get("spdx_id") if (info.get("license") or {}).get("spdx_id") != "NOASSERTION" else None,
        "language": info.get("language"),
        "platforms": platforms,
        "build": build,
        "owner": info["owner"]["login"],
    }


def code_for(name):
    words = re.findall(r"[A-Za-z0-9]+", name)
    if len(words) >= 2:
        return (words[0][0] + words[1][0]).title()
    w = words[0] if words else "Ap"
    return (w[0].upper() + (w[1:2] or "p").lower())


def category_name(section):
    return {"Apps": "Desktop apps"}.get(section, section)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("lists", nargs="+")
    ap.add_argument("--catalog", default="catalog.json")
    args = ap.parse_args()

    catalog = json.load(open(args.catalog))
    curated = [a for a in catalog["apps"] if a.get("curated")]
    entries = []
    for url in args.lists:
        entries += parse(list_text(url))
    print(f"{len(entries)} entries", file=sys.stderr)

    # Entries that link to a page instead of a repository: the repository that page links to.
    pages = [e for e in entries if not GH.match(e["url"])]
    with cf.ThreadPoolExecutor(8) as ex:
        found = list(ex.map(lambda e: repos_on_page((get(e["url"]) or b"").decode("utf-8", "replace")), pages))
    everywhere = collections.Counter(r for c in found for r in c)
    common = {r for r, n in everywhere.items() if n > max(3, len(pages) * 0.05)}
    for e, c in zip(pages, found):
        best = [(n, r) for r, n in c.items() if r not in common]
        e["repo"] = max(best)[1] if best else None
    for e in entries:
        if "repo" not in e:
            m = GH.match(e["url"])
            e["repo"] = f"{m[1]}/{re.sub(r'.git$', '', m[2])}"
    entries = [e for e in entries if e.get("repo")]

    with cf.ThreadPoolExecutor(8) as ex:
        details = list(ex.map(lambda e: inspect(e["repo"]), entries))

    taken = {a["id"] for a in curated}
    seen = {a["repo"].lower() for a in curated}
    apps, sections = [], []
    for e, d in sorted(zip(entries, details), key=lambda x: -(x[1] or {}).get("stars", 0)):
        if not d or d["repo"].lower() in seen:
            continue
        buildable = d["build"] and d["language"] in NATIVE
        if not d["platforms"] and not buildable:
            continue
        seen.add(d["repo"].lower())
        base = slug(d["repo"].split("/")[1])
        app_id = base if base not in taken else slug(d["repo"].replace("/", "-"))
        taken.add(app_id)
        category = slug(category_name(e["section"]))
        if category not in [s for s, _ in sections]:
            sections.append((category, category_name(e["section"])))
        h = sum(d["repo"].encode()) % len(PALETTE)
        tagline = e["desc"].rstrip(".") or d["description"].rstrip(".") or d["repo"]
        app = {
            "id": app_id,
            "name": e["name"].strip(),
            "code": code_for(e["name"]),
            "repo": d["repo"],
            "category": category,
            "tagline": tagline[:120],
            "description": d["description"],
            "colors": {"bg": PALETTE[h], "fg": "#FFFFFF"},
            "icon": f"https://github.com/{d['owner']}.png?size=128",
            "stars": d["stars"],
            "platforms": d["platforms"],
        }
        for key in ("homepage", "license", "build"):
            if d[key]:
                app[key] = d[key]
        apps.append(app)

    categories = [c for c in catalog.get("categories", []) if any(a["category"] == c["id"] for a in curated)]
    for cid, name in sections:
        if cid not in [c["id"] for c in categories]:
            categories.append({"id": cid, "name": name})
    catalog["apps"] = curated + apps
    catalog["categories"] = categories
    catalog["revision"] = int(catalog.get("revision", 0)) + 1
    with open(args.catalog, "w") as f:
        json.dump(catalog, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"{len(apps)} apps from the lists, {len(curated)} curated, {len(categories)} categories", file=sys.stderr)


if __name__ == "__main__":
    main()
