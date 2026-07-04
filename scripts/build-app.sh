#!/usr/bin/env bash
# Build Notypo as a signed (.app) macOS bundle.
#
#   scripts/build-app.sh           # release build + ad-hoc signed .app
#
# Output: dist/Notypo.app
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="Notypo"
BUNDLE_ID="com.notypo.app"
BIN_NAME="notypo"
DIST="dist"

echo "==> cargo build --release"
cargo build --release

BINARY="target/release/${BIN_NAME}"
if [[ ! -x "${BINARY}" ]]; then
  echo "error: built binary not found at ${BINARY}" >&2
  exit 1
fi

APP_ROOT="${DIST}/${APP_NAME}.app"
CONTENTS="${APP_ROOT}/Contents"
MACOS_DIR="${CONTENTS}/MacOS"
RESOURCES="${CONTENTS}/Resources"

echo "==> assembling ${APP_ROOT}"
rm -rf "${APP_ROOT}"
mkdir -p "${MACOS_DIR}" "${RESOURCES}"

# Executable
cp "${BINARY}" "${MACOS_DIR}/${BIN_NAME}"

# TypeMark editor assets -> Contents/Resources/assets/TypeMark
# The runtime resolver in src/main.rs looks for them there.
mkdir -p "${RESOURCES}/assets"
cp -R assets/TypeMark "${RESOURCES}/assets/TypeMark"

# App icon: build a .icns from assets/app-icon.png and place it in Resources.
# Also copy the raw PNG so the runtime icon loader (set_app_icon) can find it.
echo "==> generating AppIcon.icns"
ICON_SRC="assets/app-icon.png"
if [[ ! -f "${ICON_SRC}" ]]; then
  echo "error: icon source not found at ${ICON_SRC}" >&2
  exit 1
fi
ICONSET_TMP="$(mktemp -d -t notypo-iconset-XXXX)"
ICONSET="${ICONSET_TMP}/AppIcon.iconset"
mkdir -p "${ICONSET}"
sips -z 16 16     "${ICON_SRC}" --out "${ICONSET}/icon_16x16.png"      >/dev/null
sips -z 32 32     "${ICON_SRC}" --out "${ICONSET}/icon_16x16@2x.png"   >/dev/null
sips -z 32 32     "${ICON_SRC}" --out "${ICONSET}/icon_32x32.png"      >/dev/null
sips -z 64 64     "${ICON_SRC}" --out "${ICONSET}/icon_32x32@2x.png"   >/dev/null
sips -z 128 128   "${ICON_SRC}" --out "${ICONSET}/icon_128x128.png"    >/dev/null
sips -z 256 256   "${ICON_SRC}" --out "${ICONSET}/icon_128x128@2x.png" >/dev/null
sips -z 256 256   "${ICON_SRC}" --out "${ICONSET}/icon_256x256.png"    >/dev/null
sips -z 512 512   "${ICON_SRC}" --out "${ICONSET}/icon_256x256@2x.png" >/dev/null
sips -z 512 512   "${ICON_SRC}" --out "${ICONSET}/icon_512x512.png"    >/dev/null
sips -z 1024 1024 "${ICON_SRC}" --out "${ICONSET}/icon_512x512@2x.png" >/dev/null
iconutil -c icns "${ICONSET}" -o "${RESOURCES}/AppIcon.icns"
rm -rf "$(dirname "${ICONSET}")"
cp "${ICON_SRC}" "${RESOURCES}/app-icon.png"

# Info.plist
cat > "${CONTENTS}/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>${BUNDLE_ID}</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleExecutable</key>
  <string>${BIN_NAME}</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSPrincipalClass</key>
  <string>NSApplication</string>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key>
      <string>Markdown Document</string>
      <key>CFBundleTypeRole</key>
      <string>Editor</string>
      <key>LSItemContentTypes</key>
      <array>
        <string>net.daringfireball.markdown</string>
        <string>public.plain-text</string>
      </array>
      <key>LSHandlerRank</key>
      <string>Default</string>
    </dict>
  </array>
</dict>
</plist>
PLIST

echo "==> ad-hoc codesign"
codesign --sign - --force --deep "${APP_ROOT}"

echo "==> verifying signature"
codesign --verify --verbose=2 "${APP_ROOT}"

echo
echo "Done: ${APP_ROOT}"
echo "Open it with:  open ${APP_ROOT}"
