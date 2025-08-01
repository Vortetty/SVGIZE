|Before | After|
|-------|------|
|![An image of a street in japan at night](/test_images/japanese_street_1.jpg) | ![An image of a street in japan at night](/results/japanese_street_1.png)|
|![An image of a sky rat](/test_images/ratbird_shot.png) | ![An image of a sky rat](/results/skyrat.png)|

<sub>The image of the street in japan is licensed under [Creative Commons Attribution 2.0 Generic](https://creativecommons.org/licenses/by/2.0/deed.en) originally posted to [Flickr on March 27, 2013 at 9:58:19 AM PDT by whitefield_d](https://flickr.com/photos/49968453@N02/8594761813)</sub>

<sub>The image of the bird is taken by Wintersys/Vortetty, licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/).</sub>

---

The japanese street one is about 9000 (about 8100 succeeded) iterations and the skyrat one is about 36000 (about 22000 succeeded) iterations. By no means is this fast but it isn't slow either for what it is and how naive the approach used, each image having been completed in about an hour.

All images produced by this program should be considered derivatives of the original as they are transformative in a way that would be arguable in a court as a derivative. Please keep this in mind when using this program.

Requires nasm due to the image library, this is to get as much speed as one can manage to from the program.

rsvg-convert is recommended to convert the svg to a png, inkscape dislikes the generated SVGs. I can also verify they render well in firefox.

---

todo:

- [x] Find and fix source of code deadlocks (turned out to be nested rayon calls)
- [x] Consecutive fails allowed setting to prevent going for an accuracy or number of images that is impossible to reach, 100 seems reasonable?
- [ ] Reuse elements instead of repeating them, these SVG sizes are getting out of control
- [ ] Add hd PNG output mode, maybe use resvg? though that imports a skia reimplementation... Would be better than what i have now where you have to manually convert
- [ ] Guide placement by error and thresholding
