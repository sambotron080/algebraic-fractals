#!/bin/bash
# Builds the program for macOS 11 or later, as universal binaries that run natively on
# both Apple silicon and Intel Macs, into dist-mac/:
#   Algebraic Fractals.app   the app (the explorer and the video window)
#   algfrac                  the command-line renderer
# and zips them with the guides as dist-mac/AlgebraicFractals-<version>-macos.zip.
#
# Usage, on a Mac, from this folder:   bash build-mac.sh
# Needs Rust (https://rustup.rs) and Apple's command line tools (xcode-select --install).
set -euo pipefail
cd "$(dirname "$0")"

command -v cargo >/dev/null || { echo "Rust not found: install it from https://rustup.rs"; exit 1; }
xcode-select -p >/dev/null 2>&1 || { echo "Apple's command line tools not found: run  xcode-select --install"; exit 1; }

version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
targets="aarch64-apple-darwin x86_64-apple-darwin"
rustup target add $targets
export MACOSX_DEPLOYMENT_TARGET=11.0 # also used when compiling the C++ video encoder
for t in $targets; do
    cargo build --release --target "$t"
done

out=dist-mac
rm -rf "$out"
mkdir -p "$out"

# one binary containing both architectures
universal() {
    lipo -create -output "$2" "target/aarch64-apple-darwin/release/$1" "target/x86_64-apple-darwin/release/$1"
}

app="$out/Algebraic Fractals.app"
mkdir -p "$app/Contents/MacOS"
universal algebraic-fractals "$app/Contents/MacOS/algebraic-fractals"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Algebraic Fractals</string>
    <key>CFBundleDisplayName</key><string>Algebraic Fractals</string>
    <key>CFBundleIdentifier</key><string>io.github.sambotron080.algebraic-fractals</string>
    <key>CFBundleExecutable</key><string>algebraic-fractals</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
# the icon (the lotus flower, assets/icon.png from examples/make_icon.rs), at every size macOS uses
iconset="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$iconset" "$app/Contents/Resources"
for s in 16 32 128 256 512; do
    sips -z $s $s assets/icon.png --out "$iconset/icon_${s}x${s}.png" >/dev/null
    sips -z $((s * 2)) $((s * 2)) assets/icon.png --out "$iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"
# ad-hoc signature (no Apple developer account): enough to run on this Mac, and on
# others after allowing it once (see docs/MAC.md)
codesign --force --deep --sign - "$app"
universal algfrac "$out/algfrac"
codesign --force --sign - "$out/algfrac"
cp docs/USER-GUIDE.md docs/MAC.md "$out/"
(cd "$out" && zip -qry "AlgebraicFractals-$version-macos.zip" "Algebraic Fractals.app" algfrac USER-GUIDE.md MAC.md)

echo
echo "Built (shareable copies in $out/):"
ls -1 "$out"
