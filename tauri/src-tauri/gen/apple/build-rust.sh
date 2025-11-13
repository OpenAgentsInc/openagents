#!/bin/bash
set -e

# This wrapper script handles both dev and archive builds
# For archive builds (ACTION=install), it directly builds the Rust library
# For dev builds, it uses the Tauri CLI which requires a WebSocket connection

if [ "$ACTION" = "install" ]; then
    echo "Archive build detected - checking for pre-built library"

    # For archive builds, we expect the library to have been built already
    # by running 'bun tauri ios build' before archiving

    # SRCROOT is at gen/apple, go up 2 levels to src-tauri
    TAURI_ROOT="$SRCROOT/../.."

    # Determine target arch
    if [ "$ARCHS" = "arm64" ]; then
        TARGET="aarch64-apple-ios"
        ARCH="arm64"
    elif [ "$ARCHS" = "x86_64" ]; then
        TARGET="x86_64-apple-ios"
        ARCH="x86_64"
    else
        TARGET="aarch64-apple-ios"
        ARCH="arm64"
    fi

    # Check if library already exists
    SOURCE_LIB="$TAURI_ROOT/target/$TARGET/release/libopenagents_lib.a"
    DEST_LIB="$SRCROOT/Externals/$ARCH/$CONFIGURATION/libapp.a"

    if [ -f "$SOURCE_LIB" ]; then
        echo "Found pre-built library at $SOURCE_LIB"
        mkdir -p "$SRCROOT/Externals/$ARCH/$CONFIGURATION"
        cp "$SOURCE_LIB" "$DEST_LIB"
        echo "Copied library to $DEST_LIB"
    else
        echo "ERROR: Library not found at $SOURCE_LIB"
        echo "Please run 'bun tauri ios build' first to build the iOS library"
        exit 1
    fi

    echo "Archive build complete"
else
    echo "Dev build detected - using Tauri CLI"

    # Use the Tauri CLI for dev builds (requires WebSocket server)
    /Users/christopherdavid/.bun/bin/bun tauri ios xcode-script -v \
        --platform ${PLATFORM_DISPLAY_NAME:?} \
        --sdk-root ${SDKROOT:?} \
        --framework-search-paths "${FRAMEWORK_SEARCH_PATHS:?}" \
        --header-search-paths "${HEADER_SEARCH_PATHS:?}" \
        --gcc-preprocessor-definitions "${GCC_PREPROCESSOR_DEFINITIONS:-}" \
        --configuration ${CONFIGURATION:?} \
        ${FORCE_COLOR} \
        ${ARCHS:?}
fi
