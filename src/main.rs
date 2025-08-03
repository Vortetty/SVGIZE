// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
#![feature(f16)]

use std::{borrow::Cow, collections::HashMap, f32::consts::PI, fs, io::Cursor, path::{Path, PathBuf}, process::exit, u32};

use clap::Parser;
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use rand::{prelude::*, rngs::OsRng, TryRngCore};
use image::{buffer::ConvertBuffer, imageops::{self, resize, FilterType::{self, Lanczos3}}, GrayImage, ImageReader, Luma, Rgb, RgbImage, Rgba, RgbaImage};
use colored::Colorize;
use rand_xoshiro::Xoshiro256PlusPlus;
use rayon::prelude::*;
use regex::Regex;
use walkdir::WalkDir;
use xmltree::Element;
use image_compare::{utils::Decompose, yuv_hybrid_compare};

struct FragmentImage {
    pub im: GrayImage,
    pub src_svg: PathBuf
}

struct ImageSetting<'a> { // the image pasted on and all the info abt it
    src_svg: Cow<'a, PathBuf>,
    size: u32, // Pixel width
    center_x: u32,
    center_y: u32,
    color: [u8; 3], // Will substitute all pixels for this but preserve alpha of the original
    rotation: f16, // 0.0-2pi
}
struct ImageObj<'a> { // The image used
    im: GrayImage, // All we actually need is alpha since we have settings.color
    topleft_x_pos: i64,
    topleft_y_pos: i64,
    settings: ImageSetting<'a>
}

fn blend_yuv_yuva(a: (u8, u8, u8), b: (u8, u8, u8, u8)) -> (u8, u8, u8) { // Blend a yuv pixel with a yuva pixel
    let (ay, au, av) = a;
    let (by, bu, bv, ba) = b;

    let alpha = ba as f32 / 255.0;
    let inverse_alpha = 1.0 - alpha;

    let blend = |ac: u8, bc: u8| -> u8 {
        ((bc as f32 * alpha) + (ac as f32 * inverse_alpha)).round().clamp(0.0, 255.0) as u8
    };

    (
        blend(ay, by),
        blend(au, bu),
        blend(av, bv),
    )
}

fn rgb_to_yuv(rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    let py = 0. + (0.299 * rgb.0 as f32) + (0.587 * rgb.1 as f32) + (0.114 * rgb.2 as f32);
    let pu = 128. - (0.168736 * rgb.0 as f32) - (0.331264 * rgb.1 as f32) + (0.5 * rgb.2 as f32);
    let pv = 128. + (0.5 * rgb.0 as f32) - (0.418688 * rgb.1 as f32) - (0.081312 * rgb.2 as f32);
    (py as u8, pu as u8, pv as u8)
}

fn yuv_to_rgb(yuv: (u8, u8, u8)) -> (u8, u8, u8) {
    let r = yuv.0 as f32 + (1.402 * (yuv.2 as f32 - 128.));
    let g = yuv.0 as f32 - (0.344136 * (yuv.1 as f32 - 128.)) - (0.714136 * (yuv.2 as f32 - 128.));
    let b = yuv.0 as f32 + (1.772 * (yuv.1 as f32 - 128.));
    (r as u8, g as u8, b as u8)
}

impl<'a> ImageObj<'a> {
    pub fn paste(&self, result: &mut [GrayImage; 3]) {
        let size = self.settings.size as i64;
        let x0 = self.topleft_x_pos;
        let y0 = self.topleft_y_pos;
        let width = result[0].width() as i64;
        let height = result[0].height() as i64;

        let x_size = (x0 + size).min(width).saturating_sub(x0.max(0));
        let y_size = (y0 + size).min(height).saturating_sub(y0.max(0));
        let x_offset = 0.max(-x0);
        let y_offset = 0.max(-y0);

        let [ref mut y_img, ref mut u_img, ref mut v_img] = *result;

        let y_buf = y_img.as_flat_samples_mut().samples;
        let u_buf = u_img.as_flat_samples_mut().samples;
        let v_buf = v_img.as_flat_samples_mut().samples;
        let alpha_buf = self.im.as_flat_samples().samples;
        let base_x = (x0 + x_offset) as u32;
        let base_y = (y0 + y_offset) as u32;

        for y in 0..y_size {
            let row = (base_y + y as u32) as usize * width as usize;
            for x in 0..x_size {
                let idx = row + (base_x + x as u32) as usize;

                let blended = blend_yuv_yuva(
                    (y_buf[idx], u_buf[idx], v_buf[idx]),
                    (
                        self.settings.color[0],
                        self.settings.color[1],
                        self.settings.color[2],
                        alpha_buf[(y_offset + y) as usize * x_size as usize + (x_offset + x) as usize],
                    )
                );

                y_buf[idx] = blended.0;
                u_buf[idx] = blended.1;
                v_buf[idx] = blended.2;
            }
        }
    }

    pub fn restore(&self, source: &[GrayImage; 3], result: &mut [GrayImage; 3]) {
        let size = self.settings.size as i64;
        let x0 = self.topleft_x_pos;
        let y0 = self.topleft_y_pos;
        let width = result[0].width() as i64;
        let height = result[0].height() as i64;

        let x_size = (x0 + size).min(width).saturating_sub(x0.max(0));
        let y_size = (y0 + size).min(height).saturating_sub(y0.max(0));
        let x_offset = 0.max(-x0);
        let y_offset = 0.max(-y0);

        let [ref y_img, ref u_img, ref v_img] = *source;
        let y_in_buf = y_img.as_flat_samples().samples;
        let u_in_buf = u_img.as_flat_samples().samples;
        let v_in_buf = v_img.as_flat_samples().samples;

        let [ref mut y_img, ref mut u_img, ref mut v_img] = *result;
        let y_out_buf = y_img.as_flat_samples_mut().samples;
        let u_out_buf = u_img.as_flat_samples_mut().samples;
        let v_out_buf = v_img.as_flat_samples_mut().samples;

        let base_x = (x0 + x_offset) as u32;
        let base_y = (y0 + y_offset) as u32;

        for y in 0..y_size {
            let row = (base_y + y as u32) as usize * width as usize;
            for x in 0..x_size {
                let idx = row + (base_x + x as u32) as usize;

                y_out_buf[idx] = y_in_buf[idx];
                u_out_buf[idx] = u_in_buf[idx];
                v_out_buf[idx] = v_in_buf[idx];
            }
        }
    }
}

fn similarity_range(s: &str) -> Result<f64, String> {
    let sim: f64  = s.parse().map_err(|_| format!("{s} is not a number"))?;

    if sim <= 100.0 && sim >= 0.0 {
        Ok(sim)
    } else {
        Err(format!("{} is not in the range 0.0-100.0 inclusive", s))
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input file, supported formats: .jpg, .jpeg, .jpe, .png, .webp, .avif, .bmp, .tiff, .tif, .qoi
    input: String,

    /// Output file, will output in the same folder by default. Should be an svg, if not an svg it will add the extension.
    #[arg(short, long)]
    output: Option<String>,

    /// Minumum number of shapes to place, depending on the image you may want more than default, set to 0 to disable
    #[arg(short, long, default_value_t=500)]
    shapes: u32,

    /// Minimum match percentage (0.0-100.0), if used with --shapes will stop only when both conditions are met, 100% is impossible and normally 25-50% match is enough. If set to 100% it will run until it fails the number of times specified by --failmax
    #[arg(short, long, value_parser=similarity_range)]
    matchscore: Option<f64>,

    /// Image width to use during comparison of image, larger images will be more similar at the cost of speed, smaller (even 256 or 512) will normally yield a fine result, that said larger images will allow more variation and thus accuracy
    #[arg(short, long, default_value_t=384)]
    cmpwidth: u32,

    /// Max number of failed iterations before the image is output as-is. This overrides cmpwidth and matchscore so it will need set very high to work
    #[arg(short, long, default_value_t=100)]
    failmax: u32,

    /// Number of images to try in each iteration, more will be slower but choose more optimal images and have failed iterations less often
    #[arg(short, long, default_value_t=16)]
    imgcnt: u32,
}

fn main() {
    let args = Args::parse();
    let source_image: String = args.input;
    let target_score = args.matchscore.unwrap_or_else(|| 0.0) as f64 / 100.0;
    let target_shapes = args.shapes;
    let mut outfile = args.output.unwrap_or_else(|| source_image.clone() + ".svg");
    if !outfile.ends_with(".svg") {
        outfile += ".svg"
    }

    if target_score <= 0.0 && target_shapes <= 0 {
        println!("Without a target score or target shape count, the image will be blank. Please provide one.");
        exit(0);
    }

    let mut seed_bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut seed_bytes);
    let mut rng = Xoshiro256PlusPlus::from_seed(seed_bytes);

    println!("Loading source image...");
    let input_image = {
        let im = ImageReader::open(source_image.clone()).unwrap().decode().unwrap().to_rgba8();
        resize(&im, args.cmpwidth, (args.cmpwidth as f32/im.width() as f32*im.height() as f32) as u32, FilterType::Triangle).convert() as RgbImage
    }.split_to_yuv();
    let avgcolor = {
        let im = ImageReader::open(source_image.clone()).unwrap().decode().unwrap().to_rgba8();
        let im1 = resize(&im, args.cmpwidth, (args.cmpwidth as f32/im.width() as f32*im.height() as f32) as u32, FilterType::Triangle).convert() as RgbImage;
        let tmp = resize(&im1, 1, 1, FilterType::Triangle);
        let rgb = tmp.get_pixel(0, 0);
        rgb_to_yuv((rgb[0], rgb[1], rgb[2]))
    };
    let mut dest_image = [
        GrayImage::from_pixel(input_image[0].width(),input_image[0].height(), Luma([avgcolor.0])),
        GrayImage::from_pixel(input_image[1].width(),input_image[1].height(), Luma([avgcolor.1])),
        GrayImage::from_pixel(input_image[2].width(),input_image[2].height(), Luma([avgcolor.2]))
    ];
    let mut desttmp = dest_image.clone(); // Clone once, we need desttmp for temporary edits and dest_image for the cache of all edits done so far.
    println!("Loaded source image");

    println!("Loading fragment images...");
    let images: Vec<FragmentImage> = WalkDir::new("images_png").into_iter().par_bridge().filter_map(|e| e.ok()).filter_map(|path| {
        if path.metadata().unwrap().is_file() {
            let im = ImageReader::open(path.path()).ok()?.decode().ok()?;
            println!("{}{}", "Loaded fragment image: ".italic().bright_black(), format!("{}", path.path().display()).italic().bright_black());

            Some(FragmentImage {
                im: GrayImage::from_raw(im.width(), im.height(), im.as_flat_samples_u8().unwrap().samples.iter().skip(3).step_by(4).map(|x| x.clone()).collect()).unwrap(),
                src_svg: {
                    let mut f = path.path().to_path_buf();
                    f.set_extension("svg");
                    Path::new("images/").join(f.strip_prefix("images_png").ok().unwrap()).to_path_buf()
                }
            })
        } else {
            None
        }
    }).collect();
    if images.len() > 0 {
        println!("Loaded {} fragment images successfully", images.len());
    } else {
        println!("Could not find any images in images_png, please run `gen_images_png.sh`");
        exit(1);
    }

    let mut gen_rand_im = || -> ImageObj {
        let im_index = rng.random_range(0..images.len()) as usize;
        let rand_center_x = rng.random_range(0..input_image[0].width());
        let rand_center_y = rng.random_range(0..input_image[0].height());
        let mut rand_size = (0..4).map(|_| rng.random_range(0..input_image[0].width().max(input_image[0].height()))).min().unwrap();
        if rand_size < 1 {
            rand_size += 1;
        }
        let mut rand_size_rotated = (rand_size as f32*rand_size as f32 * 2.0).sqrt().ceil() as u32; // Assuming a square, this is the size it would be at 45deg rotation and means the image will always fit
        if rand_size_rotated % 2 != rand_size % 2 {
            rand_size_rotated += 1;
        }
        let rand_rot = rng.next_u32() as f32 / u32::MAX as f32 * (PI*2.0);

        let paste_offset = (rand_size_rotated as f32/2.0).floor() as u32 - (rand_size as f32/2.0).floor() as u32;
        let src_resized = resize(&images[im_index].im, rand_size, rand_size, Lanczos3);
        let mut im_tmp = GrayImage::from_pixel(rand_size_rotated, rand_size_rotated, Luma([0]));

        let dst_buf = im_tmp.as_flat_samples_mut().samples;
        let src_buf = src_resized.as_flat_samples().samples;

        let dst_width = rand_size_rotated as usize;
        let src_width = rand_size as usize;
        let offset = paste_offset as usize;

        for y in 0..src_width {
            let dst_row = (y + offset) * dst_width;
            let src_row = y * src_width;

            for x in 0..src_width {
                dst_buf[dst_row + x + offset] = src_buf[src_row + x];
            }
        }

        ImageObj {
            im: rotate_about_center(&im_tmp, rand_rot, Interpolation::Bicubic, Luma([0])),
            topleft_x_pos: rand_center_x as i64 - (rand_size_rotated as f32/2.0).floor() as i64,
            topleft_y_pos: rand_center_y as i64 - (rand_size_rotated as f32/2.0).floor() as i64,
            settings: ImageSetting {
                rotation: rand_rot as f16,
                size: rand_size,
                color: [
                    input_image[0].get_pixel(rand_center_x, rand_center_y)[0],
                    input_image[1].get_pixel(rand_center_x, rand_center_y)[0],
                    input_image[2].get_pixel(rand_center_x, rand_center_y)[0]
                ],
                center_x: rand_center_x,
                center_y: rand_center_y,
                src_svg: Cow::Borrowed(&images[im_index].src_svg)
            }
        }
    };

    let mut curr_score = (yuv_hybrid_compare(&input_image, &dest_image).unwrap().score * 10000.0).floor() / 10000.0;

    let mut success = 0;
    let mut failure = 0;
    let mut consec_fails = 0;
    let mut placed: Vec<ImageSetting> = vec![];

    while (curr_score < target_score || success < target_shapes) && consec_fails < args.failmax {
        let im_best_result = (0..args.imgcnt)
            .map(|_| gen_rand_im())
            .enumerate()
            .filter_map(
                |pasteover| -> Option<(ImageObj, f64, usize)> {
                    pasteover.1.paste(&mut desttmp);
                    let newscore = (yuv_hybrid_compare(&input_image, &desttmp).unwrap().score * 10000.0).floor() / 10000.0;
                    pasteover.1.restore(&dest_image, &mut desttmp);

                    if newscore > curr_score {
                        Some((pasteover.1, newscore, pasteover.0))
                    } else {
                        None
                    }
                }
            )
            .max_by_key(|x| (x.1 * 1000000.0) as i32);

        if im_best_result.is_some() {
            let im = im_best_result.unwrap();
            curr_score = im.1;
            im.0.paste(&mut dest_image);
            im.0.paste(&mut desttmp);
            //dest_image.save(format!("out/{:.06}.png", im.1)); // Disabled for production, good for debug tho
            placed.push(im.0.settings);
            success += 1;
            consec_fails = 0;
            println!("Image success ({:.04}% > {:.04}%)", im.1*100.0, curr_score*100.0);
            println!("{}/{}/{}/{} (placed/failed/consecutive fails/score)", success.to_string().bright_green(), failure.to_string().bright_red(), consec_fails.to_string().bright_yellow(), format!("{:.04}", curr_score * 100.0).bright_magenta());
            continue;
        }
        failure += 1;
        consec_fails += 1;
        println!("{} images failed", args.imgcnt);
        println!("{}/{}/{}/{} (placed/failed/consecutive fails/score)", success.to_string().bright_green(), failure.to_string().bright_red(), consec_fails.to_string().bright_yellow(), format!("{:.04}", curr_score * 100.0).bright_magenta());
    }

    println!("Image finished!\nSaving... This may take a while");
    let bg_color = yuv_to_rgb((avgcolor.0, avgcolor.1, avgcolor.2));
    let mut output = format!("<svg viewBox=\"0 0 {} {}\" xmlns=\"http://www.w3.org/2000/svg\"><rect x=\"0\" y=\"0\" width=\"100%\" height=\"100%\" fill=\"#{:06X}\"/><clipPath id=\"clipView\"><rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\"/></clipPath><g clip-path=\"url(#clipView)\">", input_image[0].width(), input_image[0].height(), (bg_color.0 as u32) << 16 | (bg_color.1 as u32) << 8 | bg_color.2 as u32, input_image[0].width(), input_image[0].height());
    let mut svg_cache: HashMap<PathBuf, String> = HashMap::new();
    let style_prop_regex = Regex::new(r"(fill|color):.+?;").unwrap();
    let tag_regex = Regex::new(r#"(?s)(<(style|metadata)\b[^>]*>.*?</(style|metadata)>|<\s*(metadata|g)\b[^>]*\/\s*>|(class|version)\s*=\s*"(.*?)"|(class|version)\s*=\s*'(.*?)'|xmlns(:\w+)?\s*=\s*"[^"]*"|xmlns(:\w+)?\s*=\s*'[^']*'|<\?xml\b[^?]*\?>)"#).unwrap(); // All style, metadata, and empty g tags, as well as all class tags and xmlns tags and xml declarations
    let space_regex = Regex::new(r"\s+").unwrap();
    let none = "none".to_string();
    for img in placed {
        if !svg_cache.contains_key(img.src_svg.as_ref()) {
            let mut svg = Element::parse(fs::read_to_string(img.src_svg.as_ref()).unwrap().as_bytes()).unwrap();
            svg.name = "symbol".to_string();
            svg.attributes.insert("id".to_string(), format!("{}", svg_cache.len()));
            svg.attributes.insert("fill".to_string(), "currentColor".to_string());
            if svg.attributes.get("stroke").unwrap_or_else(|| &none).to_string() != none { // Some use stroke, we don't like them but have to support it
                svg.attributes.insert("stroke".to_string(), "currentColor".to_string());
            } else {
                svg.attributes.insert("stroke".to_string(), "none".to_string());
            }
            let mut buffer = Cursor::new(Vec::new());
            svg.write(&mut buffer);
            let svgtext = String::from_utf8(buffer.into_inner()).unwrap();
            let tmp = style_prop_regex.replace_all(svgtext.as_ref(), "fill:currentColor;".to_string()); // Replace other fills, like style tags
            let outstr = tag_regex.replace_all(tmp.as_ref(), ""); // Remove styles unless they are inline
            let outstr_nospace = space_regex.replace_all(outstr.as_ref(), " ");
            output += "<defs>"; // Defs prevents rendering
            output += outstr_nospace.as_ref(); // These just cause errors, idk why the xml library includes them by default.
            output += "</defs>";

            svg_cache.insert(img.src_svg.as_ref().clone(), format!("{}", svg_cache.len()));
        }
        let svgid = svg_cache.get(img.src_svg.as_ref()).unwrap();
        let color = yuv_to_rgb((img.color[0], img.color[1], img.color[2]));
        output += format!("<use x=\"0\" y=\"0\" transform=\"translate({} {}) rotate({:.03} {} {})\" width=\"{}\" height=\"{}\" color=\"#{:06X}\" href=\"#{}\" />",
            img.center_x as i32 - (img.size as f32/2.0) as i32,
            img.center_y as i32 - (img.size as f32/2.0) as i32,
            img.rotation as f32 * (180.0/PI),
            img.size as f32/2.0,
            img.size as f32/2.0,
            img.size,
            img.size,
            (color.0 as u32) << 16 | (color.1 as u32) << 8 | color.2 as u32,
            svgid
        ).as_str();
    }
    output += "</g></svg>";

    fs::write(outfile.clone(), output);
}
