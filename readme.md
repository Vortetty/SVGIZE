# SVGIZE

An image filter

|Before | After|
|-------|------|
| <img src="/test_images/japanese_street_1.jpg" width="1024" alt="An image of a street in japan at night"> | <img src="/results/japanese_street_1.png" width="1024" alt="An image of a street in japan at night"> |
| <img src="/test_images/ratbird_shot.png" width="1024" alt="An image of a sky rat"> | <img src="/results/skyrat.png" width="1024" alt="An image of a sky rat"> |

<sub>The image of the street in japan is licensed under [Creative Commons Attribution 2.0 Generic](https://creativecommons.org/licenses/by/2.0/deed.en) originally posted to [Flickr on March 27, 2013 at 9:58:19 AM PDT by whitefield_d](https://flickr.com/photos/49968453@N02/8594761813)</sub>

<sub>The image of the bird is taken by Wintersys/Vortetty, licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).</sub>

---

The japanese street one is about 9000 (about 8100 succeeded) iterations and the skyrat one is about 36000 (about 22000 succeeded) iterations. By no means is this fast but it isn't slow either for what it is and how naive the approach used, each image having been completed in about an hour.

All images produced by this program should be considered derivatives of the original as they are transformative in a way that would be arguable in a court as a derivative. Please keep this in mind when using this program.

Requires nasm due to the image library, this is to get as much speed as one can manage to from the program.

rsvg-convert is recommended to convert the svg to a png, inkscape dislikes the generated SVGs. I can also verify they render well in firefox. (Also needed anyway for `gen_images_png.sh` which you need to run to get the `images_png` dir)

---

### TODO

- [ ] Add hd PNG output mode, maybe use resvg? though that imports a skia reimplementation... Would be better than what i have now where you have to manually convert
- [ ] Guide placement by error and thresholding
- [x] Implement a function to allow better use of the image compare lib, done along with all the yuv overhead
- [x] Find and fix source of code deadlocks (turned out to be nested rayon calls)
- [x] Consecutive fails allowed setting to prevent going for an accuracy or number of images that is impossible to reach, 100 seems reasonable?
- [x] Reuse elements instead of repeating them, these SVG sizes are getting out of control

---

### Usage

Important note prior to running, please make sure you have `rsvg-convert` (from `librsvg`), `git`, and `nasm` installed.

Once those are installed, clone the repo

```sh
git clone https://github.com/Vortetty/SVGIZE
cd SVGIZE
git submodule init
git submodule update
```

Next step depends on your OS

**Linux/Mac:**

run `gen_images_png.sh`

<sub>*This does use `rm`, `mkdir`, `find`, and `printf`. If these are for some reason not present on your system refer to the windows method.*</sub>

**Windows/Others:**

You sadly don't have a script to do this right now, but make a copy of `images` called `images_png` and convert all of the SVGs to 512x512 PNGs in-place

Lastly, compile

```sh
cargo build --release
```

This will place your executable at `target/release/image_evo_filter`

For help with usage, you can run

```sh
image_evo_filter -h
```

or refer to below:

```
Usage: image_evo_filter [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input file, supported formats: .jpg, .jpeg, .jpe, .png, .webp, .avif, .bmp, .tiff, .tif, .qoi

Options:
  -o, --output <OUTPUT>          Output file, will output in the same folder by default. Should be an svg, if not an svg it will add the extension
  -s, --shapes <SHAPES>          Minumum number of shapes to place, depending on the image you may want more than default, set to 0 to disable [default: 500]
  -m, --matchscore <MATCHSCORE>  Minimum match percentage (0.0-100.0), if used with --shapes will stop only when both conditions are met, 100% is impossible and normally 25-50% match is enough. If set to 100% it will run until it fails the number of times specified by --failmax
  -c, --cmpwidth <CMPWIDTH>      Image width to use during comparison of image, larger images will be more similar at the cost of speed, smaller (even 256 or 512) will normally yield a fine result, that said larger images will allow more variation and thus accuracy [default: 384]
  -f, --failmax <FAILMAX>        Max number of failed iterations before the image is output as-is. This overrides cmpwidth and matchscore so it will need set very high to work [default: 100]
  -i, --imgcnt <IMGCNT>          Number of images to try in each iteration, more will be slower but choose more optimal images and have failed iterations less often [default: 16]
  -h, --help                     Print help
  -V, --version                  Print version
```
