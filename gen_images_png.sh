#!/bin/bash

rm -rf images_png
mkdir -p images_png

find images -type f -name '*.svg' | while read -r svg; do
    relpath="${svg#images/}"
    flatname=$(echo "$relpath" | tr '/ ' '__' | sed 's/\.svg$//')
    inkscape "$svg" --export-type=png --export-filename="images_png/${flatname}.png" -w 512 -h 512
done