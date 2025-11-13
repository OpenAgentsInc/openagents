#!/bin/bash
set -e

# This wrapper script handles both dev and archive builds
# For archive builds (ACTION=install), it directly builds the Rust library
# For dev builds, it uses the Tauri CLI which requires a WebSocket connection

if [ "$ACTION" = "install" ]; then
    echo "Archive build detected - building Rust library directly"

    # SRCROOT is already at gen/apple, so go up 2 levels to get to src-tauri
    TAURI_ROOT="$SRCROOT/../.."

    # Build for the target architecture
    cd "$TAURI_ROOT"
    echo "Working directory: $(pwd)"

    # Determine target triple
    if [ "$ARCHS" = "arm64" ]; then
        TARGETS="aarch64-apple-ios"
    elif [ "$ARCHS" = "x86_64" ]; then
        TARGETS="x86_64-apple-ios"
    else
        TARGETS="aarch64-apple-ios"
    fi

    echo "Building for targets: $TARGETS"

    # Set up iOS SDK paths for cross-compilation
    export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"
    export SDKROOT="${SDK_DIR}"

    # Build the library
    for TARGET in $TARGETS; do
        echo "Building $TARGET with SDK: $SDKROOT..."

        # Set target-specific environment variables for the linker
        export CARGO_TARGET_AARCH64_APPLE_IOS_LINKER="$TOOLCHAIN_DIR/usr/bin/clang"
        export CARGO_TARGET_X86_64_APPLE_IOS_LINKER="$TOOLCHAIN_DIR/usr/bin/clang"

        cargo build \
            --target "$TARGET" \
            --release \
            --lib \
            --manifest-path "$TAURI_ROOT/Cargo.toml"

        # Extract architecture from target
        if [[ "$TARGET" == *"aarch64"* ]]; then
            ARCH="arm64"
        elif [[ "$TARGET" == *"x86_64"* ]]; then
            ARCH="x86_64"
        else
            ARCH="arm64"
        fi

        # Copy the built library to the expected location
        mkdir -p "$SRCROOT/Externals/$ARCH/$CONFIGURATION"
        cp "$TAURI_ROOT/target/$TARGET/release/libopenagents_lib.a" \
           "$SRCROOT/Externals/$ARCH/$CONFIGURATION/libapp.a"

        echo "Copied library to $SRCROOT/Externals/$ARCH/$CONFIGURATION/libapp.a"
    done

    echo "Rust library build complete"
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
