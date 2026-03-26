#!/usr/bin/env bash
set -euo pipefail

CRATE_NAME="suggestions_processor"

NAME=SuggestionsProcessorRust
CRATE_LIB=libsuggestions_processor.a
MIN_MACOS=11.3
MIN_IOS=15.0

DIST_DIR="dist/apple"

# Clean output folders
rm -rf "${DIST_DIR}"

# Ensure targets
rustup target add aarch64-apple-darwin x86_64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios

# Build all targets
MACOSX_DEPLOYMENT_TARGET="$MIN_MACOS" cargo build --release \
  --config profile.release.debug=true \
  --target aarch64-apple-darwin

MACOSX_DEPLOYMENT_TARGET="$MIN_MACOS" cargo build --release \
  --config profile.release.debug=true \
  --target x86_64-apple-darwin

IPHONEOS_DEPLOYMENT_TARGET="$MIN_IOS" cargo build --release \
  --config profile.release.debug=true \
  --target aarch64-apple-ios

IPHONEOS_DEPLOYMENT_TARGET="$MIN_IOS" cargo build --release \
  --config profile.release.debug=true \
  --target aarch64-apple-ios-sim

IPHONEOS_DEPLOYMENT_TARGET="$MIN_IOS" cargo build --release \
  --config profile.release.debug=true \
  --target x86_64-apple-ios

# Header
INCLUDE_DIR_ROOT="${DIST_DIR}/include"
INCLUDE_DIR="${INCLUDE_DIR_ROOT}/SuggestionsProcessorRust"
mkdir -p "$INCLUDE_DIR"
if ! command -v cbindgen >/dev/null 2>&1; then cargo install cbindgen; fi
cbindgen --config cbindgen.toml --crate ${CRATE_NAME} --output "${INCLUDE_DIR}/ddg_suggestions_processor.h"
cat > "${INCLUDE_DIR}/module.modulemap" <<-EOF
module SuggestionsProcessorRust {
  header "ddg_suggestions_processor.h"
  export *
}
EOF

# Universal binaries
mkdir -p "${DIST_DIR}/macos-arm64_x86_64"
lipo -create \
  target/x86_64-apple-darwin/release/${CRATE_LIB} \
  target/aarch64-apple-darwin/release/${CRATE_LIB} \
  -output "${DIST_DIR}/macos-arm64_x86_64/${CRATE_LIB}"

mkdir -p "${DIST_DIR}/ios-arm64_x86_64-simulator"
lipo -create \
  target/x86_64-apple-ios/release/${CRATE_LIB} \
  target/aarch64-apple-ios-sim/release/${CRATE_LIB} \
  -output "${DIST_DIR}/ios-arm64_x86_64-simulator/${CRATE_LIB}"

mkdir -p "${DIST_DIR}/ios-arm64"
cp -f "target/aarch64-apple-ios/release/${CRATE_LIB}" "${DIST_DIR}/ios-arm64/${CRATE_LIB}"

# Weaken rust_eh_personality to avoid duplicate symbol when linked alongside other Rust staticlibs
LLVM_OBJCOPY="$(brew --prefix llvm)/bin/llvm-objcopy"
if [ ! -x "$LLVM_OBJCOPY" ]; then
  echo "❌ llvm-objcopy not found. Install with: brew install llvm" >&2
  exit 1
fi
for lib in "${DIST_DIR}"/macos-*/${CRATE_LIB} "${DIST_DIR}"/ios-*/${CRATE_LIB}; do
  "$LLVM_OBJCOPY" --weaken-symbol=_rust_eh_personality "$lib"
done

# Create xcframework
rm -rf "${DIST_DIR}/${NAME}.xcframework"
xcodebuild -create-xcframework \
  -library "${DIST_DIR}/macos-arm64_x86_64/${CRATE_LIB}" -headers "${INCLUDE_DIR_ROOT}" \
  -library "${DIST_DIR}/ios-arm64/${CRATE_LIB}" -headers "${INCLUDE_DIR_ROOT}" \
  -library "${DIST_DIR}/ios-arm64_x86_64-simulator/${CRATE_LIB}" -headers "${INCLUDE_DIR_ROOT}" \
  -output "${DIST_DIR}/${NAME}.xcframework"

echo "✅ Built ${DIST_DIR}/${NAME}.xcframework"
