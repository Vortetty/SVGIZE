#!/bin/bash
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

rm -rf images_png
mkdir -p images_png

find images -type f -name '*.svg' | while read -r svg; do
    relpath="${svg#images/}"
    flatname=$(echo "$relpath" | tr '/ ' '__' | sed 's/\.svg$//')
    inkscape "$svg" --export-type=png --export-filename="images_png/${flatname}.png" -w 512 -h 512
done