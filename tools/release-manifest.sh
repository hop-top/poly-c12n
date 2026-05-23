#!/bin/sh
# release-manifest.sh — convert goreleaser's checksums.txt into the
# release-asset manifest.json shape consumed by the FFI-driven c12n-php
# Installer (see docs/adr/0002-c12n-php-ffi-binding.md §"Release-asset
# manifest.json contract").
#
# Input  : path to goreleaser's `dist/checksums.txt`
#          format per line: "<sha256-hex>  <filename>"  (two spaces)
# Output : JSON object on stdout, e.g.
#          {"libc12n_core-linux-x86_64.tar.gz":"sha256:<hex>", ...}
#
# Usage  : ./tools/release-manifest.sh dist/checksums.txt > dist/manifest.json
#
# POSIX-only; runs on minimal CI images (no bash-isms).

set -eu

if [ "$#" -ne 1 ]; then
    echo "usage: $0 <checksums.txt>" >&2
    exit 2
fi

input=$1
if [ ! -f "$input" ]; then
    echo "$0: not a file: $input" >&2
    exit 1
fi

printf '{'
first=1
while IFS= read -r line || [ -n "$line" ]; do
    # Skip blank lines and comments.
    case $line in
        '' | \#*) continue ;;
    esac
    # goreleaser format: "<hex>  <filename>" (two spaces). Tolerate one
    # or more whitespace runs between the two columns.
    hash=$(printf '%s\n' "$line" | awk '{print $1}')
    name=$(printf '%s\n' "$line" | awk '{print $2}')
    if [ -z "$hash" ] || [ -z "$name" ]; then
        continue
    fi
    if [ "$first" -eq 1 ]; then
        first=0
    else
        printf ','
    fi
    printf '"%s":"sha256:%s"' "$name" "$hash"
done <"$input"
printf '}\n'
