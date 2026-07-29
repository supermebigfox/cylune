#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/../../../.." && pwd)
cd "$root"

test -f src-tauri/native/mac/tiyda/BlackHole.metal
test -f src-tauri/native/mac/tiyda/MetalBlackHoleView.m
grep -Fq '03e74a5' src-tauri/native/mac/tiyda/THIRD_PARTY_NOTICES.md
grep -Fq 'MIT License' src-tauri/native/mac/tiyda/LICENSE
grep -Fq 'ghostty-blackhole' src-tauri/native/mac/tiyda/THIRD_PARTY_NOTICES.md
grep -Fq 'float2 center;' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'ingestProgress' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'ejectProgress' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'successJetProgress' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'blackHoleSuccessJetProgress' \
  src-tauri/native/mac/tiyda/BlackHoleDesktop.h
grep -Fq 'successJetProgress' \
  src-tauri/native/mac/tiyda/MetalBlackHoleView.m
grep -Fq 'float2 center = clamp(P.center' src-tauri/native/mac/tiyda/BlackHole.metal
! grep -Fq '0.57+0.19*sin' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'P.time' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'inwardAccretionFlow' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'spiralInflow' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'inflowContour' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'fullSurfaceEnvelope' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'float flowGain' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'clockwiseTangent' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'float2 tangent=' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'diskLuminousFlow' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'diskOcclusion' src-tauri/native/mac/tiyda/BlackHole.metal
! grep -Fq 'flowBoundary' src-tauri/native/mac/tiyda/BlackHole.metal
! grep -Fq 'streamMask' src-tauri/native/mac/tiyda/BlackHole.metal
! grep -Fq 'strandWeight' src-tauri/native/mac/tiyda/BlackHole.metal
! grep -Fq 'bg*(1.0-diskAbsorption)+diskLight' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'diskTintForStyle' src-tauri/native/mac/tiyda/BlackHole.metal
! grep -Fq 'dynamicSpacetimeFlow' src-tauri/native/mac/tiyda/BlackHole.metal
! grep -Fq 'radialWave' src-tauri/native/mac/tiyda/BlackHole.metal
! grep -Fq 'lensPulse' src-tauri/native/mac/tiyda/BlackHole.metal
