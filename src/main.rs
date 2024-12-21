mod scanner;

use fixedbitset::FixedBitSet;
use image::RgbImage;
use scanner::Scanner;

struct Map {
    horizontal_walls: FixedBitSet,
    vertical_walls: FixedBitSet,
    holes: FixedBitSet,
    holes_placement: Vec<(usize, usize)>,
    width: usize,
    height: usize,
}

impl Map {
    pub fn read(width: usize, height: usize, scanner: &mut Scanner<impl std::io::BufRead>) -> Self {
        let mut slf = Map {
            horizontal_walls: FixedBitSet::with_capacity(width * (height + 1)),
            vertical_walls: FixedBitSet::with_capacity((width + 1) * height),
            holes: FixedBitSet::with_capacity(width * height),
            holes_placement: vec![],
            width,
            height,
        };

        for y in 0..slf.height {
            slf.vertical_walls.insert(slf.vertical_wall_index(0, y));
            slf.vertical_walls
                .insert(slf.vertical_wall_index(slf.width, y));
        }

        for x in 0..slf.width {
            slf.horizontal_walls.insert(slf.horizontal_wall_index(x, 0));
            slf.horizontal_walls
                .insert(slf.horizontal_wall_index(x, slf.height));
        }

        for y in 0..slf.height {
            for x in 1..slf.width {
                slf.vertical_walls
                    .set(slf.vertical_wall_index(x, y), scanner.read::<u32>() != 0);
            }
        }

        for y in 1..slf.height {
            for x in 0..slf.width {
                slf.horizontal_walls
                    .set(slf.horizontal_wall_index(x, y), scanner.read::<u32>() != 0);
            }
        }

        for _ in 0..scanner.read::<u32>() {
            let x = scanner.read::<usize>();
            let y = scanner.read::<usize>();
            slf.holes.insert(slf.tile_index(x, y));
            slf.holes_placement.push((x, y));
        }

        slf
    }

    fn horizontal_wall_index(&self, x: usize, y: usize) -> usize {
        x * (self.height + 1) + y
    }

    fn vertical_wall_index(&self, x: usize, y: usize) -> usize {
        y * (self.width + 1) + x
    }

    fn tile_index(&self, x: usize, y: usize) -> usize {
        self.width * y + x
    }

    fn image(&self, with_holes: bool, tile_width: usize, tile_height: usize) -> RgbImage {
        let mut img = RgbImage::new(
            (self.width * tile_width) as u32,
            (self.height * tile_height) as u32,
        );

        let wall_color = image::Rgb([255; 3]);
        let hole_color = image::Rgb([127, 0, 0]);
        let odd_color = image::Rgb([50; 3]);
        let even_color = image::Rgb([0; 3]);

        for y in 0..self.height {
            for x in 0..self.width {
                let to_fill = if with_holes && self.holes.contains(self.tile_index(x, y)) {
                    Some(hole_color)
                } else if (x + y) % 2 == 1 {
                    Some(odd_color)
                } else {
                    Some(even_color)
                };

                if let Some(to_fill) = to_fill {
                    for tx in 0..tile_width {
                        for ty in 0..tile_height {
                            let tile_x = x * tile_width + tx;
                            let tile_y = y * tile_height + ty;
                            img.put_pixel(tile_x as u32, tile_y as u32, to_fill);
                        }
                    }
                }

                if Direction::Left.blocked((x, y), self) {
                    for ty in 0..tile_height {
                        let tile_x = x * tile_width;
                        let tile_y = y * tile_height + ty;
                        img.put_pixel(tile_x as u32, tile_y as u32, wall_color);
                    }
                }

                if Direction::Right.blocked((x, y), self) {
                    for ty in 0..tile_height {
                        let tile_x = (x + 1) * tile_width - 1;
                        let tile_y = y * tile_height + ty;
                        img.put_pixel(tile_x as u32, tile_y as u32, wall_color);
                    }
                }

                if Direction::Up.blocked((x, y), self) {
                    for tx in 0..tile_width {
                        let tile_x = x * tile_width + tx;
                        let tile_y = y * tile_height;
                        img.put_pixel(tile_x as u32, tile_y as u32, wall_color);
                    }
                }

                if Direction::Down.blocked((x, y), self) {
                    for tx in 0..tile_width {
                        let tile_x = x * tile_width + tx;
                        let tile_y = (y + 1) * tile_height - 1;
                        img.put_pixel(tile_x as u32, tile_y as u32, wall_color);
                    }
                }
            }
        }

        img
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct State {
    positions: [(usize, usize); 2],
}

impl State {
    pub const START: State = State {
        positions: [(0, 0); 2],
    };
}

#[derive(Clone, Copy, Debug)]
enum Direction {
    Right,
    Left,
    Up,
    Down,
}

impl Direction {
    pub const ALL: [Self; 4] = [Self::Right, Self::Left, Self::Up, Self::Down];

    pub fn apply(self, pos: (usize, usize)) -> (usize, usize) {
        match self {
            Self::Right => (pos.0 + 1, pos.1),
            Self::Left => (pos.0 - 1, pos.1),
            Self::Up => (pos.0, pos.1 - 1),
            Self::Down => (pos.0, pos.1 + 1),
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Right => Self::Left,
            Self::Left => Self::Right,
            Self::Up => Self::Down,
            Self::Down => Self::Up,
        }
    }

    pub fn bits(self) -> [bool; 2] {
        match self {
            Self::Right => [false, false],
            Self::Left => [false, true],
            Self::Up => [true, false],
            Self::Down => [true, true],
        }
    }

    pub fn from_bits(bits: [bool; 2]) -> Self {
        match bits {
            [false, false] => Self::Right,
            [false, true] => Self::Left,
            [true, false] => Self::Up,
            [true, true] => Self::Down,
        }
    }

    pub fn arrow_char(self, variant: usize) -> char {
        let styles = match self {
            Self::Right => ['>', '→'],
            Self::Left => ['<', '←'],
            Self::Up => ['^', '↑'],
            Self::Down => ['v', '↓'],
        };

        styles[variant]
    }

    pub fn blocked(self, pos: (usize, usize), map: &Map) -> bool {
        match self {
            Self::Right => map
                .vertical_walls
                .contains(map.vertical_wall_index(pos.0 + 1, pos.1)),
            Self::Left => map
                .vertical_walls
                .contains(map.vertical_wall_index(pos.0, pos.1)),
            Self::Up => map
                .horizontal_walls
                .contains(map.horizontal_wall_index(pos.0, pos.1)),
            Self::Down => map
                .horizontal_walls
                .contains(map.horizontal_wall_index(pos.0, pos.1 + 1)),
        }
    }

    pub fn from_movement(old_pos: (usize, usize), pos: (usize, usize)) -> Option<Self> {
        use std::cmp::Ordering;

        match (pos.0.cmp(&old_pos.0), pos.1.cmp(&old_pos.1)) {
            (Ordering::Greater, Ordering::Equal) => Some(Self::Right),
            (Ordering::Less, Ordering::Equal) => Some(Self::Left),
            (Ordering::Equal, Ordering::Less) => Some(Self::Up),
            (Ordering::Equal, Ordering::Greater) => Some(Self::Down),
            _ => None,
        }
    }
}

fn apply_instructions<const RESPECT_HOLES: bool>(
    start: (usize, usize),
    map: &Map,
    instructions: &[Direction],
) -> (usize, usize) {
    let mut pos = start;

    for &instruction in instructions.iter().rev() {
        if !instruction.blocked(pos, map) {
            pos = instruction.apply(pos);
            if RESPECT_HOLES && map.holes.contains(map.tile_index(pos.0, pos.1)) {
                pos = (0, 0);
            }
        }
    }

    pos
}

fn solve<const RESPECT_HOLES: bool>(
    scanner: &mut Scanner<impl std::io::BufRead>,
    save_images: bool,
    unicode: bool,
) {
    let width = scanner.read::<usize>();
    let height = scanner.read::<usize>();

    let maps: [_; 2] = std::array::from_fn(|_| Map::read(width, height, scanner));

    if save_images {
        for (i, map) in maps.iter().enumerate() {
            let img = map.image(RESPECT_HOLES, 5, 5);
            img.save(format!("map_{i}.png")).unwrap();
        }
        return;
    }

    let mut v1 = vec![State::START];
    let mut v2 = vec![];

    let states_count = (width * height).pow(2);

    let mut visited_dirs: [_; 2] =
        std::array::from_fn(|_| FixedBitSet::with_capacity(states_count));
    let mut visited_movement: [_; 2] =
        std::array::from_fn(|_| FixedBitSet::with_capacity(states_count));

    let visited_index = |pos: &[(usize, usize); 2]| -> usize {
        (pos[0].1 * width + pos[0].0) * (width * height) + (pos[1].1 * width + pos[1].0)
    };

    let end_state = State {
        positions: [(width - 1, height - 1); 2],
    };

    let end = visited_index(&end_state.positions);

    // BFS
    // dead end removal? not possible in 2d, because both states are "entangled"
    // - possible in 4d but that's overkill
    let failed = 'res: loop {
        if v1.is_empty() {
            break 'res true;
        }

        let max_capacity_needed = v1.len() * 3;
        if max_capacity_needed > v2.capacity() {
            v2.reserve(max_capacity_needed - v2.capacity());
        }
        for state in v1.drain(..) {
            for dir in Direction::ALL {
                let blocked: [_; 2] =
                    std::array::from_fn(|i| dir.blocked(state.positions[i], &maps[i]));

                // if at least one player is not blocked by a wall...
                if blocked.into_iter().any(|v| !v) {
                    let mut positions = std::array::from_fn(|i| {
                        if blocked[i] {
                            state.positions[i]
                        } else {
                            dir.apply(state.positions[i])
                        }
                    });

                    // state id for a position pair,
                    // that happens "inbetween of time" (i.e. before applying the hole's movement )
                    let unadjusted_visited_i = visited_index(&positions);

                    if RESPECT_HOLES {
                        for (map, position) in maps.iter().zip(positions.iter_mut()) {
                            // is the tile a hole?
                            if map.holes.contains(map.tile_index(position.0, position.1)) {
                                // then reset the position
                                *position = (0, 0);
                            }
                        }
                    }

                    let visited_i = visited_index(&positions);

                    // and the state wasn't already observed...
                    // (if both bits are set to false, then it it is not visited)
                    if visited_movement.iter().any(|v| v.contains(visited_i)) {
                        continue;
                    }

                    for (bit, bit_set) in dir.bits().into_iter().zip(visited_dirs.iter_mut()) {
                        bit_set.set(unadjusted_visited_i, bit);
                        bit_set.set(visited_i, bit);
                    }

                    for (bit, bit_set) in blocked.into_iter().zip(visited_movement.iter_mut()) {
                        bit_set.set(unadjusted_visited_i, !bit);
                        bit_set.set(visited_i, !bit);
                    }

                    // then proceed
                    let new_state = State { positions };

                    if visited_i == end {
                        // we've reached the end!
                        break 'res false;
                    }

                    v2.push(new_state);
                }
            }
        }

        std::mem::swap(&mut v1, &mut v2);
    };

    println!(
        "v1.capacity: {}, v2.capacity: {}",
        v1.capacity(),
        v2.capacity()
    );

    if failed {
        println!("It's impossible");
        return;
    }

    println!("It's possible");

    let mut current_state = end_state;
    let mut dirs = vec![];
    let mut moves_amount = 0usize;

    while current_state != State::START {
        let visited_i = visited_index(&current_state.positions);

        let movement: [_; 2] = std::array::from_fn(|i| visited_movement[i].contains(visited_i));

        assert_ne!(movement, [false; 2]);

        let dir =
            Direction::from_bits(std::array::from_fn(|i| visited_dirs[i].contains(visited_i)));

        dirs.push(dir);

        if RESPECT_HOLES {
            for i in 0..2 {
                if current_state.positions[i] != (0, 0) {
                    continue;
                }

                let mut found = false;

                for &hole in maps[i].holes_placement.iter() {
                    current_state.positions[i] = hole;

                    let unadjusted_visited_i = visited_index(&current_state.positions);

                    let unadjusted_movement: [_; 2] =
                        std::array::from_fn(|i| visited_movement[i].contains(unadjusted_visited_i));

                    if unadjusted_movement == movement {
                        found = true;
                        break;
                    }
                }

                if !found {
                    current_state.positions[i] = (0, 0);
                }

                break;
            }
        }

        let dir = dir.opposite();

        current_state = State {
            positions: std::array::from_fn(|i| {
                if movement[i] {
                    moves_amount += 1;
                    dir.apply(current_state.positions[i])
                } else {
                    current_state.positions[i]
                }
            }),
        };
    }

    for (_i, &dir) in dirs.iter().rev().enumerate() {
        print!("{}", dir.arrow_char(if unicode { 1 } else { 0 }));
    }

    println!();
    println!(
        "Instruction's count: {}, Move's amount: {}",
        dirs.len(),
        moves_amount,
    );

    println!(
        "Player 1 valid: {}, Player 2 valid: {}",
        apply_instructions::<RESPECT_HOLES>((0, 0), &maps[0], &dirs) == end_state.positions[0],
        apply_instructions::<RESPECT_HOLES>((0, 0), &maps[1], &dirs) == end_state.positions[1],
    );
}

fn main() {
    let mut respect_holes = true;
    let mut image = false;
    let mut unicode = false;

    for arg in std::env::args().skip(1) {
        if arg == "--r" {
            respect_holes = !respect_holes;
        } else if arg == "--png" {
            image = !image;
        } else if arg == "--uni" {
            unicode = !unicode;
        } else {
            let file = std::fs::File::open(&arg).unwrap();
            let mut scanner = Scanner::new(std::io::BufReader::new(file));

            let start = std::time::Instant::now();

            let solve = if respect_holes {
                solve::<true>
            } else {
                solve::<false>
            };

            solve(&mut scanner, image, unicode);

            println!("time elapsed: {:?}", start.elapsed());
        }
    }
}
