#!/bin/bash
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

rm -rf images_png
mkdir -p images_png

# Get list of all SVGs
mapfile -t svgs < <(find images -type f -name '*.svg')
total=${#svgs[@]}
count=0

for svg in "${svgs[@]}"; do
    ((count++))
    relpath="${svg#images/}"
    outdir="images_png/$(dirname "$relpath")"
    mkdir -p "$outdir"
    outfile="${relpath%.svg}.png"
    
    percent=$(( count * 100 / total ))
    printf "%3d%% (%d/%d): %s\n" "$percent" "$count" "$total" "$relpath"

    rsvg-convert "$svg" -w 512 -h 512 -f png > "$outdir/$(basename "$outfile")"
done
