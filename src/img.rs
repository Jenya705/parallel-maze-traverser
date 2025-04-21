use std::collections::HashSet;

use image::{Rgb, RgbImage};

use crate::{instructions::collect_positions2d, Coordinate, Map};

pub fn image<const RESPECT_HOLES: bool>(maps: &[Map; 2], instructions: &Vec<[bool; 2]>) {
    for (i, map) in maps.iter().enumerate() {
        let img = gen_image(
            map,
            RESPECT_HOLES,
            5,
            5,
            &collect_positions2d::<RESPECT_HOLES>(
                instructions.iter().copied(),
                &maps[i],
                &mut [0; 2],
            ),
        );
        img.save(format!("map_{i}.png")).unwrap();
    }
}

fn gen_image(
    map: &Map,
    respect_holes: bool,
    tile_width: u32,
    tile_height: u32,
    highlight: &HashSet<[Coordinate; 2]>,
) -> RgbImage {
    let mut image = RgbImage::new(
        tile_width * map.width as u32,
        tile_height * map.height as u32,
    );

    const WALL_COLOR: Rgb<u8> = image::Rgb([0; 3]);

    for x in 0..map.width {
        for y in 0..map.height {
            let mut fill_color = if respect_holes && map.holes.contains(map.tile_index(x, y)) {
                image::Rgb([200, 10, 10])
            } else if (x + y) % 2 == 0 {
                image::Rgb([200; 3])
            } else {
                image::Rgb([255; 3])
            };

            if highlight.contains(&[x, y]) {
                fill_color.0[1] = 100;
            }

            for tx in 0..tile_width {
                for ty in 0..tile_height {
                    image.put_pixel(
                        x as u32 * tile_width + tx,
                        y as u32 * tile_height + ty,
                        fill_color,
                    );
                }
            }

            if map.vertical_walls.contains(map.vertical_wall_index(x, y)) {
                for ty in 0..tile_height {
                    image.put_pixel(
                        x as u32 * tile_width,
                        y as u32 * tile_height + ty,
                        WALL_COLOR,
                    );
                }
            }

            if map
                .horizontal_walls
                .contains(map.horizontal_wall_index(x, y))
            {
                for tx in 0..tile_width {
                    image.put_pixel(
                        x as u32 * tile_width + tx,
                        y as u32 * tile_height,
                        WALL_COLOR,
                    );
                }
            }

            if map
                .vertical_walls
                .contains(map.vertical_wall_index(x, y) + 1)
            {
                for ty in 0..tile_height {
                    image.put_pixel(
                        x as u32 * tile_width + tile_width - 1,
                        y as u32 * tile_height + ty,
                        WALL_COLOR,
                    );
                }
            }

            if map
                .horizontal_walls
                .contains(map.horizontal_wall_index(x, y) + 1)
            {
                for tx in 0..tile_width {
                    image.put_pixel(
                        x as u32 * tile_width + tx,
                        y as u32 * tile_height + tile_height - 1,
                        WALL_COLOR,
                    );
                }
            }
        }
    }

    image
}
