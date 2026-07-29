#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/../../../.." && pwd)
cd "$root"

test -f src-tauri/native/mac/tiyda/BlackHole.metal
test -f src-tauri/native/mac/tiyda/MetalBlackHoleView.m
grep -Fq '03e74a5' src-tauri/native/mac/tiyda/THIRD_PARTY_NOTICES.md
grep -Fq 'MIT License' src-tauri/native/mac/tiyda/LICENSE
grep -Fq 'ghostty-blackhole' src-tauri/native/mac/tiyda/THIRD_PARTY_NOTICES.md
