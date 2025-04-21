#![feature(sync_unsafe_cell)]

mod astar;
mod bfs;
mod delta_list;
mod graph;
mod img;
mod instructions;
mod scanner;

use std::{io::Write, process::Command, sync::Arc};

use astar::{
    DisparityPunishableManhattanDistancePriorityQueue, ManhattanDistancePriorityQueue,
    SingleBFSDistancePriorityQueue,
};
use bfs::launch_bfs_2d;
use clap::{Parser, ValueEnum};
use delta_list::FourBitDeltaListKind;
use fixedbitset::FixedBitSet;
use scanner::Scanner;

pub(crate) struct InputData {
    width: Coordinate,
    height: Coordinate,
    maps: [Map; 2],
}

impl InputData {
    pub fn any_holes(&self) -> bool {
        self.maps.iter().any(|v| !v.holes_placement.is_empty())
    }

    pub fn read(scanner: &mut Scanner<impl std::io::BufRead>) -> Self {
        let width = scanner.read::<Coordinate>();
        let height = scanner.read::<Coordinate>();
        Self {
            width,
            height,
            maps: std::array::from_fn(|_| Map::read(width, height, scanner)),
        }
    }
}

pub struct Map {
    horizontal_walls: FixedBitSet,
    vertical_walls: FixedBitSet,
    holes: FixedBitSet,
    holes_placement: Vec<[Coordinate; 2]>,
    width: Coordinate,
    height: Coordinate,
}

impl Map {
    pub fn read(
        width: Coordinate,
        height: Coordinate,
        scanner: &mut Scanner<impl std::io::BufRead>,
    ) -> Self {
        let mut slf = Map {
            horizontal_walls: FixedBitSet::with_capacity(width as usize * (height as usize + 1)),
            vertical_walls: FixedBitSet::with_capacity((width as usize + 1) * height as usize),
            holes: FixedBitSet::with_capacity(width as usize * height as usize),
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
            let x = scanner.read::<Coordinate>();
            let y = scanner.read::<Coordinate>();
            slf.holes.insert(slf.tile_index(x, y));
            slf.holes_placement.push([x, y]);
        }

        slf
    }

    fn horizontal_wall_index(&self, x: Coordinate, y: Coordinate) -> usize {
        Self::horizontal_wall_index_with(x, y, self.height as usize)
    }

    pub fn horizontal_wall_index_with(x: Coordinate, y: Coordinate, h: usize) -> usize {
        let (x, y) = (x as usize, y as usize);
        x * (h + 1) + y
    }

    fn vertical_wall_index(&self, x: Coordinate, y: Coordinate) -> usize {
        Self::vertical_wall_index_with(x, y, self.width as usize)
    }

    pub fn vertical_wall_index_with(x: Coordinate, y: Coordinate, w: usize) -> usize {
        let (x, y) = (x as usize, y as usize);
        y * (w + 1) + x
    }

    fn tile_index(&self, x: Coordinate, y: Coordinate) -> usize {
        Self::tile_index_with(x, y, self.width as _)
    }

    pub fn tile_index_with(x: Coordinate, y: Coordinate, w: usize) -> usize {
        let (x, y) = (x as usize, y as usize);
        w * y + x
    }

    pub fn tile_index_with_vec(pos: [Coordinate; 2], w: usize) -> usize {
        Self::tile_index_with(pos[0], pos[1], w)
    }
}

/// Definiert wie groß eine Koordinate eines Gängers sein kann
pub type Coordinate = i16;

/// Gibt den Endzustand zurück
pub fn end_state(width: Coordinate, height: Coordinate) -> [Coordinate; 4] {
    [width - 1, height - 1, width - 1, height - 1]
}

/// Gibt den Index des Zustandes zurück
pub fn calculate_visited_index(state: [Coordinate; 4], width: usize, tiles_count: usize) -> usize {
    // dbg!(state);
    (state[1] as usize * width + state[0] as usize) * tiles_count
        + (state[3] as usize * width + state[2] as usize)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum PathGenerator {
    /// Breadth First Search Multi Threaded with Compare-and-Swap Bit Set
    BFSMTCSBS,
    /// Breadth First Search Multi Threaded with Atomic Bit Set
    BFSMTABS,
    /// Breadth First Search Single Threaded with Bit Set
    BFSSTBS,
    /// Breadth First Search Single Threaded with Lazy Hash Map (extremely useless)
    BFSSTLHM,
    /// A* with Manhattan Distance priority queue
    ASMD,
    /// A* with Disparity Punishable Manhattan Distance priority queue (useless)
    ASDPMD,
    /// A* with 2D BFS calculated distances priority queue
    AS2DBFS,
    /// Breadth First Search in 2-Dimensions
    BFS2D,
    /// No path will be generated
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum OutputType {
    /// Saves images of both mazes, map_0.png and map_1.png
    Image,
    /// Saves graph.dot file of the **bfs** search
    Graph,
    /// Saves graph.dot file and tries to compile it using Dot utility
    GraphCmp,
    /// Prints instructions into the console
    Instructions,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct App {
    #[arg(short, long, default_value_t = false)]
    exclude_holes: bool,
    #[arg(short, long, value_enum, default_value_t = PathGenerator::BFSSTBS)]
    path_gen: PathGenerator,
    #[arg(short, long, value_enum, default_value_t = OutputType::Instructions)]
    output: OutputType,
    #[arg(short = 'u', long, default_value_t = false)]
    unicode: bool,
    #[arg(short = 't', long, default_value_t = 4)]
    threads: usize,
    #[arg(short = 'm', long, default_value_t = false)]
    memory_optimization: bool,
    #[arg()]
    input_file: String,
}

fn main() {
    let app = App::parse();

    let file = std::fs::File::open(&app.input_file).unwrap();
    let mut scanner = Scanner::new(std::io::BufReader::new(file));
    let data = InputData::read(&mut scanner);
    let respect_holes = !app.exclude_holes && data.any_holes();

    let InputData {
        width,
        height,
        maps,
    } = data;

    let maps = Arc::new(maps);

    macro_rules! launch_bfs {
        ($kind: expr) => {
            if respect_holes {
                let mut callback = instructions::InstructionsOutputCallback::<true>::default();
                bfs::launch_bfs::<true>(
                    width,
                    height,
                    Arc::clone(&maps),
                    app.threads,
                    $kind,
                    &mut callback,
                );
                (callback.instructions, callback.moves)
            } else {
                let mut callback = instructions::InstructionsOutputCallback::<false>::default();
                bfs::launch_bfs::<false>(
                    width,
                    height,
                    Arc::clone(&maps),
                    app.threads,
                    $kind,
                    &mut callback,
                );
                (callback.instructions, callback.moves)
            }
        };
    }

    macro_rules! launch_astar {
        ($queue: ty) => {
            if respect_holes {
                let mut callback = instructions::InstructionsOutputCallback::<true>::default();
                astar::launch_astar::<$queue, true>(
                    width,
                    height,
                    &maps,
                    &mut callback,
                    app.memory_optimization,
                );
                (callback.instructions, callback.moves)
            } else {
                let mut callback = instructions::InstructionsOutputCallback::<false>::default();
                astar::launch_astar::<$queue, false>(
                    width,
                    height,
                    &maps,
                    &mut callback,
                    app.memory_optimization,
                );
                (callback.instructions, callback.moves)
            }
        };
    }

    let (instructions, moves) = match app.path_gen {
        PathGenerator::BFSMTCSBS => launch_bfs!(FourBitDeltaListKind::CompareAndSwapAtomicBitSet),
        PathGenerator::BFSSTLHM => launch_bfs!(FourBitDeltaListKind::LazyHashMap),
        PathGenerator::BFSMTABS => launch_bfs!(FourBitDeltaListKind::AtomicBitSet),
        PathGenerator::BFSSTBS => launch_bfs!(FourBitDeltaListKind::BitSet),
        PathGenerator::ASMD => launch_astar!(ManhattanDistancePriorityQueue),
        PathGenerator::AS2DBFS => {
            if respect_holes {
                launch_astar!(SingleBFSDistancePriorityQueue::<true>)
            } else {
                launch_astar!(SingleBFSDistancePriorityQueue::<false>)
            }
        }
        PathGenerator::ASDPMD => launch_astar!(DisparityPunishableManhattanDistancePriorityQueue),
        PathGenerator::BFS2D => (
            if respect_holes {
                launch_bfs_2d::<true>(width, height, &maps)
            } else {
                launch_bfs_2d::<false>(width, height, &maps)
            },
            0,
        ),
        PathGenerator::None => (vec![], 0),
    };
    match app.output {
        OutputType::Image => (if respect_holes {
            img::image::<true>
        } else {
            img::image::<false>
        })(&maps, &instructions),
        OutputType::Graph | OutputType::GraphCmp => {
            (if respect_holes {
                graph::graph::<true>
            } else {
                graph::graph::<false>
            })(width, height, &maps, &instructions);

            if matches!(app.output, OutputType::GraphCmp) {
                let process = Command::new("dot")
                    .arg("-Tsvg")
                    .arg("graph.dot")
                    .output()
                    .expect("GraphViz's Dot utility wasn't found");

                let mut svg_file = std::fs::File::create("graph.svg").unwrap();
                svg_file.write_all(&process.stdout).unwrap();
            }
        }
        OutputType::Instructions => {
            instructions::output(&instructions, moves, if app.unicode { 1 } else { 0 });
        }
    }
}
