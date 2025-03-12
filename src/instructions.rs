use std::collections::HashSet;

use crate::{Coordinate, Map};

pub const ALL_INSTRUCTIONS: [[bool; 2]; 4] =
    [[false, false], [true, false], [false, true], [true, true]];

pub fn collect_positions2d<const RESPECT_HOLES: bool>(
    instructions: impl Iterator<Item = [bool; 2]>,
    map: &Map,
    pos: &mut [Coordinate; 2],
) -> HashSet<[Coordinate; 2]> {
    let mut visited = HashSet::new();

    for instruction in instructions {
        apply_instruction::<RESPECT_HOLES>(instruction, map, pos);
        visited.insert(*pos);
    }

    visited
}

pub fn collect_instructions4d<const RESPECT_HOLES: bool>(
    instructions: impl Iterator<Item = [bool; 2]>,
    maps: &[Map; 2],
    pos: &mut [[i16; 2]; 2],
) -> Vec<[Coordinate; 4]> {
    let mut visited = vec![];

    for instruction in instructions {
        apply_instruction::<RESPECT_HOLES>(instruction, &maps[0], &mut pos[0]);
        apply_instruction::<RESPECT_HOLES>(instruction, &maps[1], &mut pos[1]);
        visited.push([pos[0][0], pos[0][1], pos[1][0], pos[1][1]]);
    }

    visited
}

pub fn apply_instruction<const RESPECT_HOLES: bool>(
    instruction: [bool; 2],
    map: &Map,
    pos: &mut [Coordinate; 2],
) {
    let [x_dimension, pos_direction] = instruction;

    let dimension = if x_dimension { 0 } else { 1 };
    let epsilon = if pos_direction { 1 } else { 0 };

    let blocked = if x_dimension {
        map.vertical_walls
            .contains(map.vertical_wall_index(pos[0] + epsilon, pos[1]))
    } else {
        map.horizontal_walls
            .contains(map.horizontal_wall_index(pos[0], pos[1] + epsilon))
    };

    if !blocked {
        pos[dimension] += if pos_direction { 1 } else { -1 };
    }

    if RESPECT_HOLES && map.holes.contains(map.tile_index(pos[0], pos[1])) {
        *pos = [0; 2];
    }
}

pub fn apply_instructions<const RESPECT_HOLES: bool>(
    dirs: impl Iterator<Item = [bool; 2]>,
    map: &Map,
    pos: &mut [Coordinate; 2],
) {
    for instruction in dirs {
        apply_instruction::<RESPECT_HOLES>(instruction, map, pos);
    }
}
