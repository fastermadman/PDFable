#!/bin/bash
# Builds PDFable.app (a drag-and-drop droplet) in the current directory.
set -euo pipefail
cd "$(dirname "$0")"
rm -rf PDFable.app
command -v cargo >/dev/null || { echo "build.sh needs Rust: https://rustup.rs" >&2; exit 1; }
cargo build --release --manifest-path rust/Cargo.toml
osacompile -o PDFable.app droplet.applescript
cp rust/target/release/pdfable PDFable.app/Contents/Resources/
# Tahoe (macOS 26) shows legacy icons inside a grey plate. assets/compiled/ holds the icon compiled
# from assets/AppIcon.icon by the "compile-icon" GitHub workflow (needs Xcode's actool).
R=PDFable.app/Contents/Resources
rm -f $R/droplet.icns
cp assets/compiled/Assets.car $R/Assets.car
cp assets/compiled/AppIcon.icns $R/AppIcon.icns
plutil -replace CFBundleIconFile -string AppIcon PDFable.app/Contents/Info.plist
plutil -replace CFBundleIconName -string AppIcon PDFable.app/Contents/Info.plist
codesign --force --deep -s - PDFable.app
echo "Built PDFable.app — drag it to ~/Applications"
