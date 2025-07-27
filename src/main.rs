// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{f32::consts::PI, fs, path::PathBuf, u32};

use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use rand::prelude::*;
use image::{imageops::{self, resize, FilterType::{self, Lanczos3}}, ImageReader, Rgb, RgbImage, Rgba, RgbaImage};
use colored::Colorize;
use rayon::{prelude::*, ThreadPoolBuilder};

struct FragmentImage {
    pub im: RgbaImage,
    pub file: PathBuf
}

struct ImageSetting { // the image pasted on and all the info abt it
    rotation: f32, // 0.0-2pi
    size: u32, // Pixel width
    color: [u8; 4], // Will substitute all pixels for this but preserve alpha of the original
    center_x: u32,
    center_y: u32,
    file: PathBuf
}
struct ImageObj { // The image used
    im: RgbaImage,
    topleft_x_pos: i64,
    topleft_y_pos: i64,
    settings: ImageSetting
}

fn main() {
    let source_image: &str = "test_images/ratbird_shot.png";

    let mut rng = rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(rand::random());
    rayon::ThreadPoolBuilder::new().num_threads(48).build_global().unwrap();

    println!("Loading source image...");
    let input_image = ImageReader::open(source_image).unwrap().decode().unwrap().to_rgba8();
    let avgcolor = {
        let tmp = resize(&input_image, 1, 1, FilterType::Triangle);
        tmp.get_pixel(0, 0).clone()
    }.0;
    let mut dest_image = RgbaImage::from_pixel(input_image.width(), input_image.height(), Rgba([avgcolor[0], avgcolor[1], avgcolor[2], 255]));
    println!("Loaded source image");

    println!("Loading fragment images...");
    let mut images: Vec<FragmentImage> = fs::read_dir("images_png").unwrap().par_bridge().filter_map(|path| {
        let p = path.unwrap();
        let im = ImageReader::open(p.path()).unwrap().decode().unwrap();
        println!("{}{}", "Loaded fragment image: ".italic().bright_black(), format!("{}", p.path().display()).italic().bright_black());

        Some(FragmentImage {
            im: im.to_rgba8(),
            file: p.path()
        })
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
                size: rand_size_rotated,
                color: pos_color,
                center_x: rand_center_x,
                center_y: rand_center_y,
                file: images[im_index].file.clone()
            }
        }
    };

    let mut curr_score = (image_compare::rgba_blended_hybrid_compare((&input_image).into(), (&dest_image).into(), Rgb([avgcolor[0], avgcolor[1], avgcolor[2]])).unwrap().score * 10000.0).floor() / 10000.0;

    let mut success = 0;
    let mut failure = 0;

    while curr_score < 0.9 {
        println!("{}/{}", success.to_string().bright_green(), failure.to_string().bright_red());
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
                        //println!(" Image {} success ({:.06} > {:.06})", pasteover.0, newscore, curr_score);
                        Some((pasteover.1, newscore, pasteover.0))
                    } else {
                        //println!(" Image {} failure ({:.06} < {:.06})", pasteover.0, newscore, curr_score);
                        None
                    }
                }
            )
            .max_by_key(|x| (x.1 * 1000000.0) as i32);

        if im_best_result.is_some() {
            let im = im_best_result.unwrap();
            curr_score = im.1;
            imageops::overlay(&mut dest_image, &im.0.im, im.0.topleft_x_pos, im.0.topleft_y_pos);
            println!("Image success ({:.06} > {:.06})", im.1, curr_score);
            dest_image.save(format!("out/{:.06}.png", im.1));
            success += 1;
            continue;
        }
        failure += 1;
        println!("16 images failed");
    }
}
