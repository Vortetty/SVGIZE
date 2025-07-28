// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{collections::HashMap, f32::consts::PI, fs::{self, File}, io::Cursor, path::{Path, PathBuf}, process::exit, u32};

use clap::Parser;
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use rand::prelude::*;
use image::{imageops::{self, resize, FilterType::{self, Lanczos3}}, ImageReader, Rgb, RgbImage, Rgba, RgbaImage};
use colored::Colorize;
use rayon::{prelude::*, ThreadPoolBuilder};
use regex::Regex;
use walkdir::WalkDir;
use xmltree::Element;

struct FragmentImage {
    pub im: RgbaImage,
    pub file: PathBuf,
    pub src_svg: PathBuf
}

struct ImageSetting { // the image pasted on and all the info abt it
    rotation: f32, // 0.0-2pi
    size: u32, // Pixel width
    color: [u8; 4], // Will substitute all pixels for this but preserve alpha of the original
    center_x: u32,
    center_y: u32,
    file: PathBuf,
    src_svg: PathBuf
}
struct ImageObj { // The image used
    im: RgbaImage,
    topleft_x_pos: i64,
    topleft_y_pos: i64,
    settings: ImageSetting
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
    /// Input file
    input: String,

    /// Output file, will output in the same folder by default. Should be an svg, if not an svg it will add the extension.
    #[arg(short, long)]
    output: Option<String>,

    /// Minumum number of shapes to place, depending on the image you may want more than default, set to 0 to disable
    #[arg(short, long, default_value_t=2000)]
    shapes: u32,

    /// Minimum match percentage (0.0-100.0), if used with --shapes will stop only when both conditions are met, 100% is impossible and normally 25-50% match is enough. This will need some trial and error.
    #[arg(short, long, value_parser=similarity_range)]
    matchscore: Option<f64>,

    /// Image width to use during comparison of image, larger images will be more similar at the cost of speed, smaller (even 256 or 512) will normally yield a fine result, that said larger images will allow more variation and thus accuracy
    #[arg(short, long, default_value_t=384)]
    cmpwidth: u32
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

    let mut rng = rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(rand::random());
    rayon::ThreadPoolBuilder::new().num_threads(24*4).build_global().unwrap();

    println!("Loading source image...");
    let input_image = {
        let im = ImageReader::open(source_image).unwrap().decode().unwrap().to_rgba8();
        resize(&im, args.cmpwidth, (args.cmpwidth as f32/im.width() as f32*im.height() as f32) as u32, FilterType::Triangle)
    };
    let avgcolor = {
        let tmp = resize(&input_image, 1, 1, FilterType::Triangle);
        tmp.get_pixel(0, 0).clone()
    }.0;
    let mut dest_image = RgbaImage::from_pixel(input_image.width(), input_image.height(), Rgba([avgcolor[0], avgcolor[1], avgcolor[2], 255]));
    println!("Loaded source image");

    println!("Loading fragment images...");
    let mut images: Vec<FragmentImage> = WalkDir::new("images_png").into_iter().par_bridge().filter_map(|e| e.ok()).filter_map(|path| {
        if path.metadata().unwrap().is_file() {
            let im = ImageReader::open(path.path()).ok()?.decode().ok()?;
            println!("{}{}", "Loaded fragment image: ".italic().bright_black(), format!("{}", path.path().display()).italic().bright_black());

            Some(FragmentImage {
                im: im.to_rgba8(),
                file: path.path().to_path_buf(),
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
    println!("Loaded {} fragment images successfully", images.len());

    let mut gen_rand_im = || -> ImageObj {
        let im_index = rng.random_range(0..images.len()) as usize;
        let rand_center_x = rng.random_range(0..input_image.width());
        let rand_center_y = rng.random_range(0..input_image.height());
        let mut rand_size = (0..4).map(|_| rng.random_range(0..input_image.width().max(input_image.height()))).min().unwrap();
        if rand_size < 1 {
            rand_size += 1;
        }
        let mut rand_size_rotated = (rand_size as f32*rand_size as f32 * 2.0).sqrt().ceil() as u32; // Assuming a square, this is the size it would be at 45deg rotation and means the image will always fit
        if rand_size_rotated % 2 != rand_size % 2 {
            rand_size_rotated += 1;
        }
        let rand_rot = rng.next_u32() as f32 / u32::MAX as f32 * (PI*2.0);

        let pos_color = input_image.get_pixel(rand_center_x, rand_center_y).0;
        let paste_offset = (rand_size_rotated as f32/2.0).floor() as u32 - (rand_size as f32/2.0).floor() as u32;
        let src_resized = resize(&images[im_index].im, rand_size, rand_size, Lanczos3);
        let mut im_tmp = RgbaImage::from_pixel(rand_size_rotated, rand_size_rotated, Rgba([pos_color[0], pos_color[1], pos_color[2], 0]));

        for x in 0..rand_size {
            for y in 0..rand_size {
                im_tmp.get_pixel_mut(x+paste_offset, y+paste_offset)[3] = src_resized.get_pixel(x, y)[3];
            }
        }

        ImageObj {
            im: rotate_about_center(&im_tmp, rand_rot, Interpolation::Bicubic, Rgba([pos_color[0], pos_color[1], pos_color[2], 0])),
            topleft_x_pos: rand_center_x as i64 - (rand_size_rotated as f32/2.0).floor() as i64,
            topleft_y_pos: rand_center_y as i64 - (rand_size_rotated as f32/2.0).floor() as i64,
            settings: ImageSetting {
                rotation: rand_rot,
                size: rand_size,
                color: pos_color,
                center_x: rand_center_x,
                center_y: rand_center_y,
                file: images[im_index].file.clone(),
                src_svg: images[im_index].src_svg.clone()
            }
        }
    };

    let mut curr_score = (image_compare::rgba_blended_hybrid_compare((&input_image).into(), (&dest_image).into(), Rgb([avgcolor[0], avgcolor[1], avgcolor[2]])).unwrap().score * 10000.0).floor() / 10000.0;

    let mut success = 0;
    let mut failure = 0;
    let mut placed: Vec<ImageSetting> = vec![];

    while curr_score < target_score || success < target_shapes {
        let im_best_result = (0..16)
            .map(|_| gen_rand_im())
            .enumerate()
            .par_bridge()
            .filter_map(
                |pasteover| -> Option<(ImageObj, f64, usize)> {
                    let mut desttmp = dest_image.clone();
                    imageops::overlay(&mut desttmp, &pasteover.1.im, pasteover.1.topleft_x_pos, pasteover.1.topleft_y_pos);
                    let newscore = (image_compare::rgba_blended_hybrid_compare((&input_image).into(), (&desttmp).into(), Rgb([avgcolor[0], avgcolor[1], avgcolor[2]])).unwrap().score * 1000000.0).floor() / 1000000.0;

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
            imageops::overlay(&mut dest_image, &im.0.im, im.0.topleft_x_pos, im.0.topleft_y_pos);
            //dest_image.save(format!("out/{:.06}.png", im.1)); // Disabled for production, good for debug tho
            placed.push(im.0.settings);
            success += 1;
            println!("Image success ({:.04}% > {:.04}%)", im.1*100.0, curr_score*100.0);
            println!("{}/{} (placed/failed)", success.to_string().bright_green(), failure.to_string().bright_red());
            continue;
        }
        failure += 1;
        println!("16 images failed");
        println!("{}/{} (placed/failed)", success.to_string().bright_green(), failure.to_string().bright_red());
    }

    println!("Image finished!\nSaving... This may take a while");
    let mut output = format!("<svg viewBox=\"0 0 {} {}\" xmlns=\"http://www.w3.org/2000/svg\"><rect x=\"0\" y=\"0\" width=\"100%\" height=\"100%\" fill=\"rgb({}, {}, {})\"/><clipPath id=\"clipView\"><rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\"/></clipPath><g clip-path=\"url(#clipView)\">", input_image.width(), input_image.height(), avgcolor[0], avgcolor[1], avgcolor[2], input_image.width(), input_image.height());
    let mut svg_cache: HashMap<PathBuf, Element> = HashMap::new();
    let fill_regex = Regex::new(r"fill:.+?;").unwrap();
    let style_regex = Regex::new(r"(?s)<style\b[^>]*>.*?</style>").unwrap();
    let none = "none".to_string();
    for img in placed {
        if !svg_cache.contains_key(&img.src_svg) {
            let svg = Element::parse(fs::read_to_string(img.src_svg.clone()).unwrap().as_bytes()).unwrap();
            svg_cache.insert(img.src_svg.clone(), svg);
        }
        let mut svg = svg_cache.get(&img.src_svg).unwrap().clone();
        svg.attributes.insert("width".to_string(), img.size.to_string());
        svg.attributes.insert("height".to_string(), img.size.to_string());
        svg.attributes.insert("x".to_string(), "0".to_string());
        svg.attributes.insert("y".to_string(), "0".to_string());
        svg.attributes.insert("fill".to_string(), format!("rgb({},{},{})", img.color[0], img.color[1], img.color[2]));
        if svg.attributes.get("stroke").unwrap_or_else(|| &none).to_string() != none {
            svg.attributes.insert("stroke".to_string(), format!("rgb({},{},{})", img.color[0], img.color[1], img.color[2]));
        }
        let mut buffer = Cursor::new(Vec::new());
        svg.write(&mut buffer);
        let svgtext = String::from_utf8(buffer.into_inner()).unwrap();
        let tmp = fill_regex.replace_all(svgtext.as_ref(), format!("fill:rgb({},{},{});", img.color[0], img.color[1], img.color[2]));
        let outstr = style_regex.replace_all(tmp.as_ref(), "");
        output += format!("<g transform=\"translate({} {}) rotate({} {} {})\">{}</g>", img.center_x as i32 - (img.size as f32/2.0) as i32, img.center_y as i32 - (img.size as f32/2.0) as i32, img.rotation * (180.0/PI), img.size as f32/2.0, img.size as f32/2.0, outstr.as_ref()).replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>", "").as_str();
    }
    output += "</g></svg>";

    fs::write(outfile.clone(), output);
    dest_image.save(outfile + ".png");
}
