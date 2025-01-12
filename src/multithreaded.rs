use std::{
    cell::SyncUnsafeCell,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
};

use crate::{apply_instructions, visited_index_fn, Coordinate, Direction, InputData, Map, State};

struct AtomicBitSet {
    visited: Vec<AtomicU64>,
    bits: Vec<AtomicU64>,
}

impl AtomicBitSet {
    fn new(len: usize) -> Self {
        let mut visited = vec![];
        visited.resize_with(len / 64 + 1, || AtomicU64::new(0));
        let mut bits = vec![];
        bits.resize_with(len / 16 + 1, || AtomicU64::new(0));
        Self { visited, bits }
    }

    fn visited_index(index: usize) -> (usize, usize) {
        (index / 64, index % 64)
    }

    fn bit_index(index: usize) -> (usize, usize) {
        (index / 16, (index % 16) * 4)
    }

    pub fn set(&self, index: usize, value: u64) -> bool {
        let (vindex, vbit) = Self::visited_index(index);
        
        if self.visited[vindex].fetch_or(1 << vbit, Ordering::Relaxed) & (1 << vbit) == 0 {
            let (index, bit) = Self::bit_index(index);
            self.bits[index].fetch_xor(value << bit, Ordering::Relaxed);
            false
        } else {
            true
        }
    }

    pub fn set_bits(&self, index: usize, bits: [[bool; 2]; 2]) -> bool {
        let value = 0u64
            | (bits[0][0] as u64 * (1 << 0))
            | (bits[0][1] as u64 * (1 << 1))
            | (bits[1][0] as u64 * (1 << 2))
            | (bits[1][1] as u64 * (1 << 3));
        self.set(index, value)
    }

    pub fn get(&self, index: usize) -> u64 {
        let (index, bit) = Self::bit_index(index);
        (self.bits[index].load(Ordering::Relaxed) >> bit) & 0b1111
    }

    pub fn get_bits(&self, index: usize) -> [[bool; 2]; 2] {
        let value = self.get(index);
        [
            [value & 1 != 0, (value >> 1) & 1 != 0],
            [(value >> 2) & 1 != 0, (value >> 3) & 1 != 0],
        ]
    }

    pub fn visited(&self, index: usize) -> bool {
        let (index, bit) = Self::visited_index(index);
        self.visited[index].load(Ordering::Relaxed) & (1 << bit) != 0
    }
}

fn thread_func<const RESPECT_HOLES: bool>(
    input: &SyncUnsafeCell<Vec<Vec<State>>>,
    output: &SyncUnsafeCell<Vec<Vec<State>>>,
    bit_set: &AtomicBitSet,
    maps: &[Map; 2],
    visited_index: &impl for<'a> Fn(&'a [[Coordinate; 2]; 2]) -> usize,
    i: usize,
    sender: &mpsc::Sender<()>,
    receiver: mpsc::Receiver<()>,
    end: usize,
) {
    'outer: loop {
        if let Err(_) = receiver.recv() {
            return;
        }

        let output = unsafe { &mut output.get().as_mut().unwrap()[i] };
        let input = unsafe { &mut input.get().as_mut().unwrap()[i] };

        if let Some(to_reserve) = (input.len() * 3).checked_sub(output.capacity()) {
            output.reserve(to_reserve);
        }

        for state in input.drain(..) {
            for dir in Direction::ALL {
                let not_blocked: [_; 2] =
                    std::array::from_fn(|i| !dir.blocked(state.positions[i], &maps[i]));

                if not_blocked.into_iter().any(|v| v) {
                    let mut positions = std::array::from_fn(|i| {
                        if not_blocked[i] {
                            dir.apply(state.positions[i])
                        } else {
                            state.positions[i]
                        }
                    });

                    let unadjusted_visited_i = visited_index(&positions);

                    if RESPECT_HOLES {
                        for (map, position) in maps.iter().zip(positions.iter_mut()) {
                            if map.holes.contains(map.tile_index(position[0], position[1])) {
                                *position = [0; 2];
                            }
                        }
                    }

                    let visited_i = visited_index(&positions);

                    let val = [not_blocked, dir.bits()];

                    if bit_set.set_bits(visited_i, val) {
                        continue;
                    }

                    bit_set.set_bits(unadjusted_visited_i, val);

                    let new_state = State { positions };

                    if visited_i == end {
                        sender.send(()).unwrap();
                        continue 'outer;
                    }

                    output.push(new_state);
                }
            }
        }
        sender.send(()).unwrap();
    }
}

pub(crate) fn solve_multithreaded<const RESPECT_HOLES: bool>(
    data: InputData,
    threads: usize,
    unicode: bool,
) {
    let InputData {
        width,
        height,
        maps,
    } = data;

    let maps = Arc::new(maps);

    let mut threads_input: Vec<Vec<State>> = vec![vec![]; threads];
    let threads_output: Vec<Vec<State>> = vec![vec![]; threads];

    threads_input[0].push(State::START);

    let threads_input = Arc::new(SyncUnsafeCell::new(threads_input));
    let threads_output = Arc::new(SyncUnsafeCell::new(threads_output));

    let tiles_count = width as usize * height as usize;
    let states_count = tiles_count.pow(2);

    let bit_set = Arc::new(AtomicBitSet::new(states_count));

    let mut thread_notifiers = vec![];
    let (done_sender, done_receiver) = mpsc::channel::<()>();

    let visited_index = visited_index_fn(width, height, tiles_count);

    let end_state = State {
        positions: [[width - 1, height - 1]; 2],
    };
    let end = visited_index(&end_state.positions);

    for i in 0..threads {
        let (notifier_sender, notifier_receiver) = mpsc::channel::<()>();
        thread_notifiers.push(notifier_sender);
        let maps = Arc::clone(&maps);
        let bit_set = Arc::clone(&bit_set);
        let threads_input = Arc::clone(&threads_input);
        let threads_output = Arc::clone(&threads_output);
        let done_sender = done_sender.clone();
        std::thread::spawn(move || {
            thread_func::<RESPECT_HOLES>(
                &threads_input,
                &threads_output,
                &bit_set,
                &maps,
                &visited_index_fn(width, height, tiles_count),
                i,
                &done_sender,
                notifier_receiver,
                end,
            );
        });
    }

    let mut failed = false;
    while !bit_set.visited(end) {
        // let mut start = std::time::Instant::now();

        for i in 0..threads {
            thread_notifiers[i].send(()).unwrap();
        }

        for _ in 0..threads {
            done_receiver.recv().unwrap();
        }

        let outputs = unsafe { &mut threads_output.get().as_mut().unwrap() };
        let inputs = unsafe { &mut threads_input.get().as_mut().unwrap() };

        let mut len = 0;

        for i in 0..threads {
            std::mem::swap(&mut inputs[i], &mut outputs[i]);
            len += inputs[i].len();
        }

        if len == 0 {
            failed = true;
            break;
        }

        // println!("solve time elapsed: {:?}", start.elapsed());
        // start = std::time::Instant::now();

        let avg_len = len / threads;

        for i in 0..threads {
            if inputs[i].len() >= avg_len {
                continue;
            }
            for j in 0..threads {
                if i == j {
                    continue;
                }
                if inputs[j].len() <= avg_len {
                    continue;
                }
                let [copy_in, copy_from] = inputs.get_many_mut([i, j]).unwrap();
                let l = (copy_from.len() - (avg_len - copy_in.len())).max(avg_len);
                copy_in.extend(&copy_from[l..]);
                copy_from.resize(l, State::START);
                if copy_in.len() >= avg_len {
                    break;
                }
            }
        }
        // println!("adjust time elapsed: {:?}. avg: {avg_len}", start.elapsed());
        // inputs.iter().map(|v| v.len()).for_each(|v| print!("{v}, "));
        // println!();
    }

    if failed {
        println!("It's impossible");
        return;
    }

    let mut current_state = end_state;
    let mut dirs = vec![];
    let mut moves_amount = 0usize;

    while current_state != State::START {
        let visited_i = visited_index(&current_state.positions);

        let bits = bit_set.get_bits(visited_i);

        assert_ne!(bits[0], [false; 2]);

        let dir = Direction::from_bits(bits[1]);

        dirs.push(dir);

        if RESPECT_HOLES {
            for i in 0..2 {
                if current_state.positions[i] != [0; 2] {
                    continue;
                }

                let mut found = false;

                for &hole in maps[i].holes_placement.iter() {
                    current_state.positions[i] = hole;

                    let unadjusted_visited_i = visited_index(&current_state.positions);

                    let unadjusted_bits = bit_set.get_bits(unadjusted_visited_i);

                    if unadjusted_bits[0] == bits[0] {
                        found = true;
                        break;
                    }
                }

                if !found {
                    current_state.positions[i] = [0; 2];
                }

                break;
            }
        }

        let dir = dir.opposite();

        current_state = State {
            positions: std::array::from_fn(|i| {
                if bits[0][i] {
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
        apply_instructions::<true>([0; 2], &maps[0], &dirs) == end_state.positions[0],
        apply_instructions::<true>([0; 2], &maps[1], &dirs) == end_state.positions[1],
    );
}
