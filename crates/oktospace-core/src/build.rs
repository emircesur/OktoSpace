//! Building an app from its source code: for apps with nothing to download for this computer, or
//! for anyone who wants to build it themselves.
//!
//! The source of a release (or of the default branch, for apps without releases) is downloaded
//! as a tarball, its build system is recognised from the files at its root, and the toolchain it
//! needs is checked before anything runs. The build's output is streamed line by line. The program
//! it produced is then packed like a release download and installed the usual way, so built apps
//! get shortcuts, updates, rollback and uninstall like any other.
//!
//! | Found | Runs | Program from |
//! |---|---|---|
//! | `Cargo.toml` | `cargo build --release` | `target/release` |
//! | `go.mod` | `go build -o .oktospace-out/` (each `cmd/*`, or `.`) | `.oktospace-out` |
//! | `build.zig` | `zig build -Doptimize=ReleaseSafe` | `zig-out/bin` |
//! | `CMakeLists.txt` | `cmake -B build`, `cmake --build build --config Release` | `build` |
//! | `meson.build` | `meson setup build`, `meson compile -C build` | `build` |
//! | `Package.swift` | `swift build -c release` | `.build/release` |
//! | `*.sln`, `*.csproj` | `dotnet publish -c Release -o .oktospace-out` | `.oktospace-out` |
//! | `package.json` | `npm ci` (or `npm install`), then its `tauri build`, `dist`, `package`, `make` or `build` script | `src-tauri/target/release`, `dist`, `release`, `out` |
//! | `Makefile` | `make` | the source folder |

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;

use crate::download::{Cancelled, Progress};
use crate::platform::Os;

/// The build systems OktoSpace knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum System {
    Cargo,
    Go,
    Zig,
    CMake,
    Meson,
    Swift,
    Dotnet,
    Npm,
    Make,
}

impl System {
    pub fn label(self) -> &'static str {
        match self {
            System::Cargo => "Rust (Cargo)",
            System::Go => "Go",
            System::Zig => "Zig",
            System::CMake => "CMake",
            System::Meson => "Meson",
            System::Swift => "Swift",
            System::Dotnet => ".NET",
            System::Npm => "Node.js (npm)",
            System::Make => "Make",
        }
    }
}

/// A program a build needs, and where to get it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    pub program: &'static str,
    pub name: &'static str,
    pub url: &'static str,
}

const CARGO: Tool = Tool { program: "cargo", name: "Rust", url: "https://rustup.rs" };
const GO: Tool = Tool { program: "go", name: "Go", url: "https://go.dev/dl/" };
const ZIG: Tool = Tool { program: "zig", name: "Zig", url: "https://ziglang.org/download/" };
const CMAKE: Tool = Tool { program: "cmake", name: "CMake", url: "https://cmake.org/download/" };
const MESON: Tool = Tool { program: "meson", name: "Meson", url: "https://mesonbuild.com/Getting-meson.html" };
const NINJA: Tool = Tool { program: "ninja", name: "Ninja", url: "https://ninja-build.org" };
const SWIFT: Tool = Tool { program: "swift", name: "Swift", url: "https://www.swift.org/install/" };
const DOTNET: Tool = Tool { program: "dotnet", name: ".NET SDK", url: "https://dotnet.microsoft.com/download" };
const NPM: Tool = Tool { program: "npm", name: "Node.js", url: "https://nodejs.org" };
const MAKE: Tool = Tool { program: "make", name: "Make", url: "https://www.gnu.org/software/make/" };

/// One command of a build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub program: String,
    pub args: Vec<String>,
}

impl Step {
    fn new(program: &str, args: &[&str]) -> Step {
        Step { program: program.into(), args: args.iter().map(|a| a.to_string()).collect() }
    }

    /// As it would be typed (for the log).
    pub fn display(&self) -> String {
        std::iter::once(self.program.clone()).chain(self.args.iter().cloned()).collect::<Vec<_>>().join(" ")
    }
}

/// How to build a source folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub system: System,
    pub steps: Vec<Step>,
    /// Folders (relative to the source) where the program ends up.
    pub outputs: Vec<PathBuf>,
    /// Copy the program's whole folder (it needs the files beside it), not just the program.
    pub whole_folder: bool,
    pub tools: Vec<Tool>,
}

/// A tag as a folder name.
pub fn safe_name(tag: &str) -> String {
    tag.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '-' }).collect()
}

/// Where Go and .NET builds put their programs.
const OUT: &str = ".oktospace-out";

/// Recognise how `src` builds.
pub fn detect(src: &Path, os: Os) -> Option<Recipe> {
    let has = |name: &str| src.join(name).is_file();
    let exe = if os == Os::Windows { ".exe" } else { "" };
    if has("Cargo.toml") {
        return Some(Recipe {
            system: System::Cargo,
            steps: vec![Step::new("cargo", &["build", "--release"])],
            outputs: vec!["target/release".into()],
            whole_folder: false,
            tools: vec![CARGO],
        });
    }
    if has("go.mod") {
        // Each `cmd/<name>` is a program; otherwise the module root is.
        let mut steps = Vec::new();
        if let Ok(entries) = std::fs::read_dir(src.join("cmd")) {
            let mut names: Vec<String> = entries
                .filter_map(Result::ok)
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            for name in names {
                let out = format!("{OUT}/{name}{exe}");
                steps.push(Step::new("go", &["build", "-trimpath", "-o", &out, &format!("./cmd/{name}")]));
            }
        }
        if steps.is_empty() {
            steps.push(Step::new("go", &["build", "-trimpath", "-o", &format!("{OUT}/"), "."]));
        }
        return Some(Recipe {
            system: System::Go,
            steps,
            outputs: vec![OUT.into()],
            whole_folder: false,
            tools: vec![GO],
        });
    }
    if has("build.zig") {
        return Some(Recipe {
            system: System::Zig,
            steps: vec![Step::new("zig", &["build", "-Doptimize=ReleaseSafe"])],
            outputs: vec!["zig-out/bin".into()],
            whole_folder: true,
            tools: vec![ZIG],
        });
    }
    if has("CMakeLists.txt") {
        return Some(Recipe {
            system: System::CMake,
            steps: vec![
                Step::new("cmake", &["-S", ".", "-B", "build", "-DCMAKE_BUILD_TYPE=Release"]),
                Step::new("cmake", &["--build", "build", "--config", "Release", "--parallel"]),
            ],
            outputs: vec!["build/Release".into(), "build/bin".into(), "build".into()],
            whole_folder: false,
            tools: vec![CMAKE],
        });
    }
    if has("meson.build") {
        return Some(Recipe {
            system: System::Meson,
            steps: vec![
                Step::new("meson", &["setup", "build", "--buildtype=release"]),
                Step::new("meson", &["compile", "-C", "build"]),
            ],
            outputs: vec!["build".into()],
            whole_folder: false,
            tools: vec![MESON, NINJA],
        });
    }
    if has("Package.swift") {
        return Some(Recipe {
            system: System::Swift,
            steps: vec![Step::new("swift", &["build", "-c", "release"])],
            outputs: vec![".build/release".into()],
            whole_folder: false,
            tools: vec![SWIFT],
        });
    }
    let dotnet = std::fs::read_dir(src)
        .ok()?
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().ends_with(".sln"))
        .or_else(|| {
            std::fs::read_dir(src)
                .ok()?
                .filter_map(Result::ok)
                .find(|e| e.file_name().to_string_lossy().ends_with(".csproj"))
        });
    if let Some(project) = dotnet {
        let project = project.file_name().to_string_lossy().into_owned();
        return Some(Recipe {
            system: System::Dotnet,
            steps: vec![Step::new("dotnet", &["publish", &project, "-c", "Release", "-o", OUT])],
            outputs: vec![OUT.into()],
            whole_folder: true,
            tools: vec![DOTNET],
        });
    }
    if has("package.json") {
        let manifest: serde_json::Value =
            std::fs::read(src.join("package.json")).ok().and_then(|b| serde_json::from_slice(&b).ok())?;
        let scripts = manifest.get("scripts").and_then(|s| s.as_object()).cloned().unwrap_or_default();
        let install = if has("package-lock.json") { "ci" } else { "install" };
        let mut steps = vec![Step::new("npm", &[install])];
        let tauri = src.join("src-tauri").is_dir();
        if tauri && scripts.contains_key("tauri") {
            steps.push(Step::new("npm", &["run", "tauri", "build"]));
        } else {
            let script = ["dist", "package", "make", "build"].into_iter().find(|s| scripts.contains_key(*s))?;
            steps.push(Step::new("npm", &["run", script]));
        }
        let mut tools = vec![NPM];
        if tauri {
            tools.push(CARGO);
        }
        return Some(Recipe {
            system: System::Npm,
            steps,
            outputs: vec![
                "src-tauri/target/release".into(),
                "dist".into(),
                "release".into(),
                "out".into(),
                "build".into(),
            ],
            whole_folder: !tauri,
            tools,
        });
    }
    if has("Makefile") || has("makefile") || has("GNUmakefile") {
        return Some(Recipe {
            system: System::Make,
            steps: vec![Step::new("make", &[])],
            outputs: vec![".".into(), "bin".into(), "build".into()],
            whole_folder: false,
            tools: vec![MAKE],
        });
    }
    None
}

/// The tool a library entry's `build` hint (`cargo`, `go`…) needs.
pub fn tool_for(hint: &str) -> Option<Tool> {
    Some(match hint {
        "cargo" => CARGO,
        "go" => GO,
        "zig" => ZIG,
        "cmake" => CMAKE,
        "meson" => MESON,
        "swift" => SWIFT,
        "dotnet" => DOTNET,
        "npm" => NPM,
        "make" => MAKE,
        _ => return None,
    })
}

/// The tools a recipe needs that aren't installed.
pub fn missing_tools(recipe: &Recipe) -> Vec<Tool> {
    recipe.tools.iter().filter(|t| find_program(t.program).is_none()).cloned().collect()
}

/// Where `program` is on the PATH (with Windows' `.exe`, `.cmd` and `.bat`).
pub fn find_program(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let names: Vec<String> = if cfg!(windows) {
        ["exe", "cmd", "bat"].iter().map(|e| format!("{program}.{e}")).collect()
    } else {
        vec![program.to_string()]
    };
    std::env::split_paths(&path).flat_map(|dir| names.iter().map(move |n| dir.join(n))).find(|p| p.is_file())
}

/// Run a recipe's steps in `src`, sending each line of their output to `log`.
pub fn run(recipe: &Recipe, src: &Path, log: &(dyn Fn(String) + Sync), progress: &Progress) -> anyhow::Result<()> {
    for step in &recipe.steps {
        progress.check_cancelled()?;
        log(format!("$ {}", step.display()));
        let program = find_program(&step.program).unwrap_or_else(|| PathBuf::from(&step.program));
        let mut child = Command::new(&program)
            .args(&step.args)
            .current_dir(src)
            .env("CARGO_TERM_COLOR", "never")
            .env("NO_COLOR", "1")
            .env("CI", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("couldn't run {}: {e}", step.program))?;
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let status = std::thread::scope(|scope| {
            let pipe = |reader: Box<dyn std::io::Read + Send>| {
                scope.spawn(move || {
                    for line in BufReader::new(reader).lines().map_while(Result::ok) {
                        log(line);
                    }
                })
            };
            let out = pipe(Box::new(stdout));
            let err = pipe(Box::new(stderr));
            // Wait, stopping the build when it's cancelled.
            let status = loop {
                if let Some(status) = child.try_wait()? {
                    break Ok::<_, anyhow::Error>(status);
                }
                if progress.cancel.load(Ordering::Relaxed) {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(anyhow::Error::new(Cancelled));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            };
            let _ = out.join();
            let _ = err.join();
            status
        })?;
        anyhow::ensure!(status.success(), "`{}` failed ({status})", step.display());
    }
    Ok(())
}

/// The program a build produced: in the recipe's output folders, the program named most like
/// `hint` (or the newest one).
pub fn find_output(src: &Path, recipe: &Recipe, hint: &str, os: Os) -> Option<PathBuf> {
    let hint = hint.to_ascii_lowercase();
    let mut best: Option<(i64, std::time::SystemTime, PathBuf)> = None;
    for out in &recipe.outputs {
        let dir = src.join(out);
        if !dir.is_dir() {
            continue;
        }
        // Only the folder itself for Cargo, CMake and Make (their build trees hold helper
        // programs); a few levels for packaged apps.
        let depth = if recipe.whole_folder { 3 } else { 1 };
        for entry in walkdir::WalkDir::new(&dir).max_depth(depth).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            if !entry.file_type().is_file() || !is_program(path, os) {
                continue;
            }
            let name = path.file_stem().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if name.starts_with("build-script") || name.contains("test") || name.ends_with("-helper") {
                continue;
            }
            let score = if name == hint {
                100
            } else if name.contains(&hint) || hint.contains(&name) {
                50
            } else {
                0
            };
            let modified = entry.metadata().ok().and_then(|m| m.modified().ok()).unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(s, t, _)| score > *s || (score == *s && modified > *t)) {
                best = Some((score, modified, path.to_path_buf()));
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|(_, _, p)| p)
}

fn is_program(path: &Path, os: Os) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if os == Os::Windows {
        return name.ends_with(".exe");
    }
    if name.ends_with(".appimage") {
        return true;
    }
    if name.contains('.') && !name.ends_with(".bin") && !name.ends_with(".x86_64") {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    false
}

/// Pack what to install (the program, or its folder) as `<dest>` (a `.tar.gz`).
pub fn pack(program: &Path, whole_folder: bool, dest: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(dest)?;
    let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    tar.follow_symlinks(false);
    if whole_folder {
        let dir = program.parent().ok_or_else(|| anyhow::anyhow!("{} has no folder", program.display()))?;
        tar.append_dir_all("app", dir)?;
    } else {
        let name = program.file_name().ok_or_else(|| anyhow::anyhow!("{} has no name", program.display()))?;
        tar.append_path_with_name(program, Path::new("app").join(name))?;
    }
    tar.into_inner()?.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_build_systems() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path();
        assert!(detect(src, Os::Linux).is_none());

        std::fs::write(src.join("Makefile"), "all:\n").unwrap();
        assert_eq!(detect(src, Os::Linux).unwrap().system, System::Make);

        std::fs::write(src.join("package.json"), r#"{"scripts":{"build":"vite build","dist":"electron-builder"}}"#)
            .unwrap();
        let npm = detect(src, Os::Linux).unwrap();
        assert_eq!(npm.system, System::Npm);
        assert_eq!(npm.steps[0], Step::new("npm", &["install"]));
        assert_eq!(npm.steps[1], Step::new("npm", &["run", "dist"]));

        std::fs::write(src.join("go.mod"), "module x\n").unwrap();
        std::fs::create_dir_all(src.join("cmd/tool")).unwrap();
        let go = detect(src, Os::Windows).unwrap();
        assert_eq!(go.system, System::Go);
        assert_eq!(go.steps[0].args, ["build", "-trimpath", "-o", ".oktospace-out/tool.exe", "./cmd/tool"]);

        std::fs::write(src.join("Cargo.toml"), "[package]\n").unwrap();
        let cargo = detect(src, Os::Linux).unwrap();
        assert_eq!(cargo.system, System::Cargo);
        assert_eq!(cargo.steps[0].display(), "cargo build --release");
    }

    #[cfg(unix)]
    #[test]
    fn builds_finds_the_program_and_packs_it() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        // A Makefile that "builds" a program and a helper.
        std::fs::write(
            src.join("Makefile"),
            "all:\n\tprintf '#!/bin/sh\\necho hi\\n' > hello\n\tchmod +x hello\n\techo built\n",
        )
        .unwrap();
        let recipe = detect(&src, Os::Linux).unwrap();
        if !missing_tools(&recipe).is_empty() {
            return; // no make here
        }
        let lines = std::sync::Mutex::new(Vec::new());
        let log = |l: String| lines.lock().unwrap().push(l);
        crate::archive::tests::no_progress(|p| run(&recipe, &src, &log, p)).unwrap();
        let lines = lines.into_inner().unwrap();
        assert_eq!(lines[0], "$ make");
        assert!(lines.iter().any(|l| l == "built"), "{lines:?}");

        let program = find_output(&src, &recipe, "hello", Os::Linux).unwrap();
        assert_eq!(program, src.join("hello"));
        assert!(program.metadata().unwrap().permissions().mode() & 0o111 != 0);

        let packed = tmp.path().join("hello-1.0.0-built.tar.gz");
        pack(&program, false, &packed).unwrap();
        let out = tmp.path().join("out");
        crate::archive::tests::no_progress(|p| crate::archive::unpack(&packed, &out, p)).unwrap();
        assert!(out.join("hello").is_file());
    }

    #[test]
    fn a_failing_step_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let recipe = Recipe {
            system: System::Make,
            steps: vec![Step::new("oktospace-no-such-program", &[])],
            outputs: vec![],
            whole_folder: false,
            tools: vec![],
        };
        let log = |_l: String| {};
        let err = crate::archive::tests::no_progress(|p| run(&recipe, tmp.path(), &log, p)).unwrap_err();
        assert!(err.to_string().contains("couldn't run"), "{err}");
    }
}
