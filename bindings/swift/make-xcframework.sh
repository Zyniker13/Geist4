#!/bin/sh
# Builds static geist4-ffi libraries with cargo and assembles them, with
# the current generated header, into bindings/swift/CGeist4.xcframework.
#
# The libraries are built with `cargo rustc --crate-type staticlib`: a
# static library is archived, never linked, so no Apple SDK is needed —
# not even for the iOS triples; the script runs wherever the Rust
# toolchain does.
#
# No xcodebuild involved: the XCFramework layout (slice directories, one
# Info.plist per slice, a top-level Info.plist listing the slices) is
# written directly.
#
# With no arguments it builds the host triple only, which is what the
# gate and `swift run Geist4Contract` on macOS need. Pass iOS triples for
# a full framework:
#   make-xcframework.sh aarch64-apple-ios aarch64-apple-ios-sim \
#                       x86_64-apple-ios
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
dest="$here/CGeist4.xcframework"
libname=libgeist4_ffi.a

# The triple the local toolchain reports; the default slice.
host=$(rustc -vV | sed -n 's/^host: //p')
targets=${*:-"$host"}

# Group the targets into slices: one per (platform, variant). Two
# simulator triples share one slice, merged with lipo.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

slice_name() {
    case $1 in
    *-darwin) printf 'macos' ;;
    *-apple-ios-sim) printf 'ios-sim' ;;
    *-apple-ios) printf 'ios' ;;
    *) printf 'unknown' ;;
    esac
}
arch_of() {
    case $1 in
    aarch64-*) printf 'arm64' ;;
    x86_64-*) printf 'x86_64' ;;
    *) printf 'unknown' ;;
    esac
}

# Build every requested target first, so a failure costs nothing. A
# staticlib is archived, never linked: rustc needs no Apple SDK even for
# the iOS triples, only the rustup std for each target.
for triple in $targets; do
    rustup target add "$triple" >/dev/null 2>&1 || true
    (cd "$root" && cargo rustc -p geist4-ffi --release --target "$triple" \
        --crate-type staticlib)
done

# Assemble.
rm -rf "$dest"
mkdir -p "$dest"
for triple in $targets; do
    lib="$root/target/$triple/release/$libname"
    [ -f "$lib" ] || { echo "no $libname for $triple" >&2; exit 1; }
done

slices=$(for triple in $targets; do slice_name "$triple"; done | sort -u)
for slice in $slices; do
    case $slice in
    macos) platform=macos; dirprefix=macos; variant='' ;;
    ios) platform=ios; dirprefix=ios; variant='' ;;
    ios-sim) platform=ios; dirprefix=ios; variant=simulator ;;
    esac
    # The triples in this slice, in a stable order.
    triples=''
    for triple in $targets; do
        [ "$(slice_name "$triple")" = "$slice" ] || continue
        case " $triples " in *" $triple "*) continue ;; esac
        triples="$triples $triple"
    done
    # Slice directory: platform, then the architectures.
    archlist=''
    for triple in $triples; do
        archlist="$archlist $(arch_of "$triple")"
    done
    archlist=$(printf '%s' "$archlist" | sed 's/^ //; s/ /_/g')
    dir="$dirprefix-$archlist"
    [ -n "$variant" ] && dir="$dir-$variant"

    mkdir -p "$dest/$dir/Headers"
    # One static library, universal when the slice has several arches.
    first=
    for triple in $triples; do
        lib="$root/target/$triple/release/$libname"
        if [ -z "$first" ]; then
            cp "$lib" "$dest/$dir/$libname"
            first=1
        else
            lipo -create "$dest/$dir/$libname" "$lib" \
                -output "$dest/$dir/$libname"
        fi
    done
    # The header is the generated, checked-in one; the module map
    # exposes it to Swift as CGeist4.
    cp "$root/crates/geist4-ffi/include/geist4.h" "$dest/$dir/Headers/"
    cat >"$dest/$dir/Headers/module.modulemap" <<'EOF'
module CGeist4 {
    header "geist4.h"
    export *
}
EOF

    # Slice Info.plist. XML on purpose: SwiftPM decodes these with
    # PropertyListDecoder, and Foundation only reads XML or binary
    # plists (JSON plists are write-only).
    tab=$(printf '\t')
    archtags=$(for a in $(printf '%s' "$archlist" | tr '_' ' '); do
        printf '%s%s\n' "$tab$tab$tab$tab" "<string>$a</string>"
    done)
    {
        printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>'
        printf '%s\n' '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">'
        printf '%s\n' '<plist version="1.0">'
        printf '%s\n' '<dict>'
        printf '%s\n' "$tab<key>HeadersPath</key>"
        printf '%s\n' "$tab<string>Headers</string>"
        printf '%s\n' "$tab<key>LibraryPath</key>"
        printf '%s\n' "$tab<string>$libname</string>"
        printf '%s\n' "$tab<key>SupportedArchitectures</key>"
        printf '%s\n' "$tab<array>"
        printf '%s' "$archtags"
        printf '%s\n' "$tab</array>"
        printf '%s\n' "$tab<key>SupportedPlatform</key>"
        printf '%s\n' "$tab<string>$platform</string>"
        if [ -n "$variant" ]; then
            printf '%s\n' "$tab<key>SupportedPlatformVariant</key>"
            printf '%s\n' "$tab<string>$variant</string>"
        fi
        printf '%s\n' '</dict>'
        printf '%s\n' '</plist>'
    } >"$dest/$dir/Info.plist"

    # Top-level plist entry. SwiftPM's XCFrameworkMetadata requires
    # SupportedPlatform and SupportedArchitectures on every entry, not
    # just the slice plists.
    {
        printf '%s\n' "$tab$tab<dict>"
        printf '%s\n' "$tab$tab$tab<key>HeadersPath</key>"
        printf '%s\n' "$tab$tab$tab<string>Headers</string>"
        printf '%s\n' "$tab$tab$tab<key>LibraryIdentifier</key>"
        printf '%s\n' "$tab$tab$tab<string>$dir</string>"
        printf '%s\n' "$tab$tab$tab<key>LibraryPath</key>"
        printf '%s\n' "$tab$tab$tab<string>$libname</string>"
        printf '%s\n' "$tab$tab$tab<key>SupportedArchitectures</key>"
        printf '%s\n' "$tab$tab$tab<array>"
        printf '%s' "$archtags"
        printf '%s\n' "$tab$tab$tab</array>"
        printf '%s\n' "$tab$tab$tab<key>SupportedPlatform</key>"
        printf '%s\n' "$tab$tab$tab<string>$platform</string>"
        if [ -n "$variant" ]; then
            printf '%s\n' "$tab$tab$tab<key>SupportedPlatformVariant</key>"
            printf '%s\n' "$tab$tab$tab<string>$variant</string>"
        fi
        printf '%s\n' "$tab$tab</dict>"
    } >>"$tmp/top-entries"
done

{
    printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>'
    printf '%s\n' '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">'
    printf '%s\n' '<plist version="1.0">'
    printf '%s\n' '<dict>'
    printf '%s\n' "$tab<key>AvailableLibraries</key>"
    printf '%s\n' "$tab<array>"
    cat "$tmp/top-entries" 2>/dev/null || true
    printf '%s\n' "$tab</array>"
    printf '%s\n' "$tab<key>CFBundlePackageType</key>"
    printf '%s\n' "$tab<string>XFWK</string>"
    printf '%s\n' "$tab<key>XCFrameworkFormatVersion</key>"
    printf '%s\n' "$tab<string>1.0</string>"
    printf '%s\n' '</dict>'
    printf '%s\n' '</plist>'
} >"$dest/Info.plist"

echo "assembled $dest:" "$slices"
