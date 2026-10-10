# Fedora / Copr package for OktoSpace. The source tarball includes vendored crates
# (`cargo vendor`), so it builds without network access; .copr/Makefile makes it.

%global debug_package %{nil}

Name:           oktospace
Version:        0.1.0
Release:        1%{?dist}
Summary:        Install, update and build apps from GitHub
License:        MIT OR Apache-2.0
URL:            https://github.com/emircesur/OktoSpace
Source0:        %{name}-%{version}-vendored.tar.gz

BuildRequires:  cargo >= 1.85
BuildRequires:  rust >= 1.85
BuildRequires:  gcc
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib

# The window uses OpenGL through winit (X11 or Wayland), loaded at run time.
# The window system libraries are loaded at run time, so rpm doesn't find them by itself.
Requires:       libglvnd-glx
Requires:       libglvnd-egl
Requires:       libxkbcommon
Requires:       libxkbcommon-x11
Requires:       libX11
Requires:       libX11-xcb
Requires:       libXcursor
Requires:       libXrandr
Requires:       libXi
Requires:       libwayland-client
Recommends:     polkit
Recommends:     fontconfig

%description
OktoSpace installs, updates and builds apps from GitHub: an App library of open-source apps,
any repository you add, checksum-verified downloads, building from source, update channels
and rollback. Installed from this package, OktoSpace is updated by dnf; it can also install
apps that publish RPMs as system packages.

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release --offline --locked -p oktospace -p oktospace-cli

%install
install -Dm755 target/release/oktospace %{buildroot}%{_bindir}/oktospace
install -Dm755 target/release/oktospace-cli %{buildroot}%{_bindir}/oktospace-cli
install -Dm644 packaging/linux/oktospace.desktop %{buildroot}%{_datadir}/applications/oktospace.desktop
install -Dm644 assets/oktospace-256.png %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/oktospace.png
install -Dm644 packaging/linux/io.github.emircesur.oktospace.metainfo.xml %{buildroot}%{_metainfodir}/io.github.emircesur.oktospace.metainfo.xml

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/oktospace.desktop
appstream-util validate-relax --nonet %{buildroot}%{_metainfodir}/io.github.emircesur.oktospace.metainfo.xml
cargo test --release --offline --locked -p oktospace-core

%files
%license LICENSE-MIT LICENSE-APACHE
%doc README.md
%{_bindir}/oktospace
%{_bindir}/oktospace-cli
%{_datadir}/applications/oktospace.desktop
%{_datadir}/icons/hicolor/256x256/apps/oktospace.png
%{_metainfodir}/io.github.emircesur.oktospace.metainfo.xml

%changelog
* Fri Oct 09 2026 OktoSpace contributors - 0.1.0-1
- First release
