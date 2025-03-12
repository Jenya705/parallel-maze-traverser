use crate::{instructions::collect_positions2d, Map};

pub fn image<const RESPECT_HOLES: bool>(maps: &[Map; 2], instructions: &Vec<[bool; 2]>) {
    for (i, map) in maps.iter().enumerate() {
        let img = map.image(
            RESPECT_HOLES,
            5,
            5,
            collect_positions2d::<RESPECT_HOLES>(instructions.iter().copied(), &maps[i], &mut [0; 2]),
        );
        img.save(format!("map_{i}.png")).unwrap();
    }
}
