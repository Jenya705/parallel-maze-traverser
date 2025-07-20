use std::collections::HashSet;

use crate::{
    bfs::Callback, calculate_visited_index, delta_list::DeltaList, end_state, Coordinate, Map,
};

pub const ALL_INSTRUCTIONS: [[bool; 2]; 4] =
    [[false, false], [true, false], [false, true], [true, true]];

/// Gibt eine Menge der besuchenden Positionen eines Gängers zurück
pub fn collect_positions2d<const RESPECT_HOLES: bool>(
    instructions: impl Iterator<Item = [bool; 2]>,
    map: &Map,
    pos: &mut [Coordinate; 2],
) -> HashSet<[Coordinate; 2]> {
    let mut visited = HashSet::new();

    for instruction in instructions {
        apply_instruction::<RESPECT_HOLES>(instruction, map, pos, true);
        visited.insert(*pos);
    }

    visited
}

/// Gibt eine geordnete Liste der besuchenden Zuständen zurück
pub fn collect_positions4d<const RESPECT_HOLES: bool>(
    instructions: impl Iterator<Item = [bool; 2]>,
    maps: &[Map; 2],
    pos: &mut [[Coordinate; 2]; 2],
) -> Vec<[Coordinate; 4]> {
    let mut visited = vec![];

    for instruction in instructions {
        apply_instruction::<RESPECT_HOLES>(instruction, &maps[0], &mut pos[0], true);
        apply_instruction::<RESPECT_HOLES>(instruction, &maps[1], &mut pos[1], true);
        visited.push([pos[0][0], pos[0][1], pos[1][0], pos[1][1]]);
    }

    visited
}

/// Wendet die gegebene Instruktion auf die gegebene Position an. Falls end_lock true ist, dann wird die Regel, dass
/// ein Gänger am Ende bleibt, ignoriert.
pub fn apply_instruction<const RESPECT_HOLES: bool>(
    instruction: [bool; 2],
    map: &Map,
    pos: &mut [Coordinate; 2],
    end_lock: bool,
) -> bool {
    if end_lock && *pos == [map.width - 1, map.height - 1] {
        return false;
    }

    let [x_dimension, direction] = instruction;

    let dimension = if x_dimension { 0 } else { 1 };
    let epsilon = if direction { 1 } else { 0 };

    let blocked = if x_dimension {
        map.vertical_walls
            .contains(map.vertical_wall_index(pos[0] + epsilon, pos[1]))
    } else {
        map.horizontal_walls
            .contains(map.horizontal_wall_index(pos[0], pos[1] + epsilon))
    };

    if !blocked {
        pos[dimension] += if direction { 1 } else { -1 };
    }

    if RESPECT_HOLES && map.holes.contains(map.tile_index(pos[0], pos[1])) {
        *pos = [0; 2];
        true
    } else {
        false
    }
}

/// Wendet alle Instruktionen auf die gegebene Position an
pub fn apply_instructions<const RESPECT_HOLES: bool>(
    dirs: impl Iterator<Item = [bool; 2]>,
    map: &Map,
    pos: &mut [Coordinate; 2],
) {
    for instruction in dirs {
        apply_instruction::<RESPECT_HOLES>(instruction, map, pos, true);
    }
}

/// Gibt die maximale Anzahl der Instruktion in einer der optimalen Lösungen zurück
pub fn maximum_instructions(maps: &[Map; 2]) -> usize {
    2 * maps[0].width as usize * maps[0].height as usize
        - 2
        - maps[0].holes_placement.len()
        - maps[1].holes_placement.len()
}

#[derive(Default)]
pub struct InstructionsOutputCallback<const RESPECT_HOLES: bool> {
    pub instructions: Vec<[bool; 2]>,
    /// Anzahl der Bewegungen (nicht Instruktionen). Also falls
    /// nur ein Gänger sich bei einer Instruktion bewegte, dann wird die
    /// Zahl nur um 1 erhöht, sonst um 2.
    pub moves: usize,
}

fn output_dir(dir: [bool; 2], style: usize) {
    let to_output = match dir {
        [true, true] => [">", "→"],
        [true, false] => ["<", "←"],
        [false, true] => ["v", "↓"],
        [false, false] => ["^", "↑"],
    };

    print!("{}", to_output[style]);
}

pub fn output(instructions: &Vec<[bool; 2]>, moves: usize, style: usize) {
    if instructions.is_empty() {
        println!("No solution found.");
        return;
    }

    for &dir in instructions {
        output_dir(dir, style);
    }

    println!();
    if moves != 0 {
        print!("Moves: {}, ", moves);
    }
    println!("Instructions: {}", instructions.len());
}

impl<const RESPECT_HOLES: bool> Callback for InstructionsOutputCallback<RESPECT_HOLES> {
    fn callback(
        &mut self,
        width: usize,
        height: usize,
        tiles_count: usize,
        maps: &[Map; 2],
        list: &impl DeltaList,
    ) {
        let mut dirs = vec![];
        let mut state = end_state(width as Coordinate, height as Coordinate);

        while state != [0; 4] {
            let delta_i = list.get_bits(calculate_visited_index(state, width, tiles_count));

            if delta_i == [false; 4] {
                return;
            }

            let mut delta = [0; 4];

            // Richtung
            let r = if delta_i[3] { 1 } else { -1 };

            // Achsen: wenn delta_i[2] true ist, dann werden die X-Achsen für die Gänger gewählt (sonst Y-Achsen)
            let i1 = if delta_i[2] { 0 } else { 1 };
            let i2 = if delta_i[2] { 2 } else { 3 };

            // Falls der erste Gänger sich bewegte.
            if delta_i[0] {
                delta[i1] = r;
            }
            // Falls der zweite Gänger...
            if delta_i[1] {
                delta[i2] = r;
            }

            if RESPECT_HOLES {
                for i in 0..2 {
                    if state[i * 2] == 0 && state[i * 2 + 1] == 0 {
                        for &[x, y] in maps[i].holes_placement.iter() {
                            let mut new_state = state;
                            new_state[i * 2] = x;
                            new_state[i * 2 + 1] = y;
                            if list.get_bits(calculate_visited_index(new_state, width, tiles_count))
                                == delta_i
                            {
                                state[i * 2] = x;
                                state[i * 2 + 1] = y;
                                break;
                            }
                        }
                    }
                }
            }

            for i in 0..4 {
                // maximal 2 Zahlen in der Matrix sind nicht 0
                if delta[i] != 0 {
                    self.moves += 1;
                }
                state[i] -= delta[i];
            }

            dirs.push([delta_i[2], delta_i[3]]);
        }

        self.instructions.reserve(dirs.len());
        for dir in dirs.into_iter().rev() {
            self.instructions.push(dir);
        }

        let mut s0 = [0; 2];
        let mut s1 = [0; 2];
        apply_instructions::<RESPECT_HOLES>(self.instructions.iter().cloned(), &maps[0], &mut s0);
        apply_instructions::<RESPECT_HOLES>(self.instructions.iter().cloned(), &maps[1], &mut s1);

        // Überprüfen, dass die Instruktionen wirklich richtig sind.
        println!(
            "0 valid: {}, 1 valid: {}",
            s0 == [width as Coordinate - 1, height as Coordinate - 1],
            s1 == [width as Coordinate - 1, height as Coordinate - 1]
        );
    }
}
