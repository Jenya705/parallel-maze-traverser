use std::{
    cell::SyncUnsafeCell,
    ops::Deref,
    sync::{Arc, Condvar, Mutex},
    time::Instant,
};

use crate::{
    calculate_visited_index,
    delta_list::{
        AsyncDeltaList, AsyncDeltaListAccessor, AtomicBitSetDeltaList, BitSetDeltaList,
        CompareAndSwapAtomicBitSetDeltaList, DeltaList, DeltaListKind, HashMapLazyDeltaList,
    },
    end_state,
    instructions::{apply_instruction, apply_instructions, ALL_INSTRUCTIONS},
    Coordinate, Map,
};

pub trait BFSCallback {
    fn callback(
        &mut self,
        width: usize,
        height: usize,
        tiles_count: usize,
        maps: &[Map; 2],
        list: &impl DeltaList,
    );
}

#[derive(Default)]
pub struct BFSInstructionsCallback<const RESPECT_HOLES: bool> {
    pub instructions: Vec<[bool; 2]>,
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
    println!("Moves: {}, Instructions: {}", moves, instructions.len());
}

impl<const RESPECT_HOLES: bool> BFSCallback for BFSInstructionsCallback<RESPECT_HOLES> {
    fn callback(
        &mut self,
        width: usize,
        height: usize,
        tiles_count: usize,
        maps: &[Map; 2],
        list: &impl DeltaList,
    ) {
        let mut dirs = vec![];
        let mut state = [
            width as Coordinate - 1,
            height as Coordinate - 1,
            width as Coordinate - 1,
            height as Coordinate - 1,
        ];

        while state != [0; 4] {
            let delta_i = list.get_bits(calculate_visited_index(state, width, tiles_count));

            if delta_i == [false; 4] {
                return;
            }

            let mut delta = [0; 4];

            let r = if delta_i[3] { 1 } else { -1 };

            let i1 = if delta_i[2] { 0 } else { 1 };
            let i2 = if delta_i[2] { 2 } else { 3 };

            if delta_i[0] {
                delta[i1] = r;
            }
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

        println!(
            "0 valid: {}, 1 valid: {}",
            s0 == [width as Coordinate - 1, height as Coordinate - 1],
            s1 == [width as Coordinate - 1, height as Coordinate - 1]
        );
    }
}

pub fn launch_bfs<const RESPECT_HOLES: bool>(
    width: Coordinate,
    height: Coordinate,
    maps: Arc<[Map; 2]>,
    threads: usize,
    kind: DeltaListKind,
    callback: &mut impl BFSCallback,
) {
    let start = Instant::now();

    let width_u = width as usize;
    let height_u = height as usize;
    let tiles_count = width_u * height_u;

    let end = calculate_visited_index(end_state(width, height), width_u, tiles_count);

    let states_count = tiles_count.pow(2);

    macro_rules! async_bit_set_launch {
        ($ty: ty) => {
            let list = Arc::new(<$ty>::new(states_count));
            #[cfg(feature = "written_count")]
            {
                crate::delta_list::written_start(states_count);
            }
            list.set(0, 1);
            multi_threaded_bfs::<RESPECT_HOLES>(
                width_u,
                height_u,
                tiles_count,
                Arc::clone(&maps),
                end,
                threads,
                Arc::clone(&list),
            );
            #[cfg(feature = "written_count")]
            {
                crate::delta_list::written_end_async(Arc::deref(&list));
            }
            println!("BFS time elapsed: {:?}", start.elapsed());
            callback.callback(
                width_u,
                height_u,
                tiles_count,
                &maps,
                &AsyncDeltaListAccessor { list: list.deref() },
            );
        };
    }

    macro_rules! bit_set_launch {
        ($ty: ty) => {
            let mut list = <$ty>::new(states_count);
            #[cfg(feature = "written_count")]
            {
                crate::delta_list::written_start(states_count);
            }
            list.set(0, 1);
            single_threaded_bfs::<RESPECT_HOLES>(
                width_u,
                height_u,
                tiles_count,
                &maps,
                end,
                &mut list,
            );
            #[cfg(feature = "written_count")]
            {
                crate::delta_list::written_end(&list);
            }
            println!("BFS time elapsed: {:?}", start.elapsed());
            callback.callback(width_u, height_u, tiles_count, &maps, &list)
        };
    }

    match kind {
        DeltaListKind::BitSet => {
            bit_set_launch!(BitSetDeltaList);
        }
        DeltaListKind::LazyHashMap => {
            bit_set_launch!(HashMapLazyDeltaList);
        }
        DeltaListKind::AtomicBitSet => {
            async_bit_set_launch!(AtomicBitSetDeltaList);
        }
        DeltaListKind::CompareAndSwapAtomicBitSet => {
            async_bit_set_launch!(CompareAndSwapAtomicBitSetDeltaList);
        }
    }
}

fn single_threaded_bfs<const RESPECT_HOLES: bool>(
    width: usize,
    height: usize,
    tiles_count: usize,
    maps: &[Map; 2],
    end: usize,
    list: &mut impl DeltaList,
) {
    let mut tasks = vec![];
    tasks.push([0, 0, 0, 0]);
    let mut output = vec![];

    while list.get(end) == 0 {
        single_layer_bfs::<RESPECT_HOLES>(
            &mut tasks,
            &mut output,
            &maps,
            width,
            height,
            tiles_count,
            list,
            end,
        );

        std::mem::swap(&mut tasks, &mut output);

        if tasks.is_empty() {
            return;
        }
    }
}

fn multi_threaded_bfs<const RESPECT_HOLES: bool>(
    width: usize,
    height: usize,
    tiles_count: usize,
    maps: Arc<[Map; 2]>,
    end: usize,
    threads: usize,
    list: Arc<impl AsyncDeltaList + Sync + Send + 'static>,
) {
    let mut thread_tasks = vec![];
    thread_tasks.resize_with(threads, || {
        SyncUnsafeCell::new(vec![[0 as Coordinate; 4]; 0])
    });
    let mut thread_outputs = vec![];
    thread_outputs.resize_with(threads, || {
        SyncUnsafeCell::new(vec![[0 as Coordinate; 4]; 0])
    });

    thread_tasks[0].get_mut().push([0; 4]);

    let thread_tasks = Arc::new(thread_tasks);
    let thread_outputs = Arc::new(thread_outputs);

    let mut notifiers = vec![];

    for i in 0..threads {
        let thread_tasks = Arc::clone(&thread_tasks);
        let thread_outputs = Arc::clone(&thread_outputs);

        let notifier = Arc::new((Mutex::new(false), Condvar::new()));

        notifiers.push(Arc::clone(&notifier));

        let maps = Arc::clone(&maps);
        let list = Arc::clone(&list);

        std::thread::spawn(move || loop {
            {
                drop(
                    notifier
                        .1
                        .wait_while(notifier.0.lock().unwrap(), |run| !*run)
                        .unwrap(),
                );
            }

            let tasks = unsafe { thread_tasks[i].get().as_mut().unwrap() };
            let output = unsafe { thread_outputs[i].get().as_mut().unwrap() };

            let mut accessor = AsyncDeltaListAccessor { list: list.deref() };

            single_layer_bfs::<RESPECT_HOLES>(
                tasks,
                output,
                &maps,
                width,
                height,
                tiles_count,
                &mut accessor,
                end,
            );

            {
                *notifier.0.lock().unwrap() = false;
                notifier.1.notify_all();
            }
        });
    }

    while list.get(end) == 0 {
        for notifier in &notifiers {
            let mut guard = notifier.0.lock().unwrap();
            *guard = true;
            notifier.1.notify_all();
        }

        for notifier in &notifiers {
            let guard = notifier.0.lock().unwrap();
            drop(notifier.1.wait_while(guard, |run| *run));
        }

        let mut len = 0;
        for i in 0..threads {
            let input = unsafe { thread_tasks[i].get().as_mut().unwrap() };
            let output = unsafe { thread_outputs[i].get().as_mut().unwrap() };
            std::mem::swap(input, output);
            len += input.len();
        }

        if len == 0 {
            return;
        }

        let avg_len = len / threads;

        let mut j = 0;

        for i in 0..threads {
            let input_i = unsafe { thread_tasks[i].get().as_mut().unwrap() };
            if input_i.len() >= avg_len {
                continue;
            }

            while j < threads {
                if j == i {
                    j += 1;
                    continue;
                }
                let input_j = unsafe { thread_tasks[j].get().as_mut().unwrap() };
                if input_j.len() <= avg_len {
                    j += 1;
                    continue;
                }
                let l = (input_j.len() - (avg_len - input_i.len())).max(avg_len);
                input_i.extend(&input_j[l..]);
                input_j.resize(l, [0; 4]);
                if input_i.len() >= avg_len {
                    break;
                }
            }
        }
    }
}

#[inline(always)]
pub fn validate_capacity(tasks: &Vec<[Coordinate; 4]>, output: &mut Vec<[Coordinate; 4]>) {
    output.reserve(tasks.len() * 4);
}

#[inline(always)]
pub fn single_layer_bfs<const RESPECT_HOLES: bool>(
    tasks: &mut Vec<[Coordinate; 4]>,
    output: &mut Vec<[Coordinate; 4]>,
    maps: &[Map; 2],
    width: usize,
    _height: usize,
    tiles_count: usize,
    delta_list: &mut impl DeltaList,
    _end: usize,
) {
    validate_capacity(tasks, output);

    for state in tasks.drain(..) {
        // SAFETY: validate_capacity was called
        unsafe {
            handle_single_state::<RESPECT_HOLES>(
                maps,
                width,
                tiles_count,
                state,
                output,
                delta_list,
            );
        }
    }
}

/// # Safety
/// the given state must be valid and the output vector must be large enough to fit 4 elements without any allocations
#[inline(always)]
pub unsafe fn handle_single_state<const RESPECT_HOLES: bool>(
    maps: &[Map; 2],
    width: usize,
    tiles_count: usize,
    state: [Coordinate; 4],
    output: &mut Vec<[Coordinate; 4]>,
    delta_list: &mut impl DeltaList,
) {
    let mut handle_non_adjusted = |delta_i: u8, non_adjusted: [i16; 4]| {
        if non_adjusted == state {
            return;
        }

        let mut adjusted = non_adjusted;
        if RESPECT_HOLES {
            let h0 = (!maps[0].holes.contains_unchecked(Map::tile_index_with(
                adjusted[0],
                adjusted[1],
                width,
            ))) as Coordinate;
            let h1 = (!maps[1].holes.contains_unchecked(Map::tile_index_with(
                adjusted[2],
                adjusted[3],
                width,
            ))) as Coordinate;

            adjusted[0] *= h0;
            adjusted[1] *= h0;
            adjusted[2] *= h1;
            adjusted[3] *= h1;
        }

        let adjusted_i = calculate_visited_index(adjusted, width, tiles_count);

        if delta_list.set(adjusted_i, delta_i) {
            if RESPECT_HOLES {
                delta_list.set(
                    calculate_visited_index(non_adjusted, width, tiles_count),
                    delta_i,
                );
            }
            output.as_mut_ptr().add(output.len()).write(adjusted);
            output.set_len(output.len() + 1);
        }
    };

    let i0h = maps[0].horizontal_wall_index(state[0], state[1]);
    let i0v = maps[0].vertical_wall_index(state[0], state[1]);
    let i1h = maps[1].horizontal_wall_index(state[2], state[3]);
    let i1v = maps[1].vertical_wall_index(state[2], state[3]);

    let left_wall_0 = (!maps[0].vertical_walls.contains_unchecked(i0v)) as Coordinate;
    let left_wall_1 = (!maps[1].vertical_walls.contains_unchecked(i1v)) as Coordinate;

    let right_wall_0 = (!maps[0].vertical_walls.contains_unchecked(i0v + 1)) as Coordinate;
    let right_wall_1 = (!maps[1].vertical_walls.contains_unchecked(i1v + 1)) as Coordinate;

    let top_wall_0 = (!maps[0].horizontal_walls.contains_unchecked(i0h)) as Coordinate;
    let top_wall_1 = (!maps[1].horizontal_walls.contains_unchecked(i1h)) as Coordinate;

    let bottom_wall_0 = (!maps[0].horizontal_walls.contains_unchecked(i0h + 1)) as Coordinate;
    let bottom_wall_1 = (!maps[1].horizontal_walls.contains_unchecked(i1h + 1)) as Coordinate;

    // d = 1 & r = -1
    let delta_0 = [1 & left_wall_0, 1 & left_wall_1];

    let delta_0_i = (delta_0[0] << 3) | (delta_0[1] << 2) | (1 << 1) | (0 << 0);

    let mut non_adjusted_0 = state;
    non_adjusted_0[0] -= delta_0[0];
    non_adjusted_0[2] -= delta_0[1];

    // d = 2 & r = -1
    let delta_1 = [1 & top_wall_0, 1 & top_wall_1];

    let delta_1_i = (delta_1[0] << 3) | (delta_1[1] << 2) | (0 << 1) | (0 << 0);

    let mut non_adjusted_1 = state;
    non_adjusted_1[1] -= delta_1[0];
    non_adjusted_1[3] -= delta_1[1];

    // d = 1 & r = +1
    let delta_2 = [1 & right_wall_0, 1 & right_wall_1];

    let delta_2_i =
        (delta_2[0] << 3) | (delta_2[1] << 2) | (1 << 1) | ((delta_2[0] | delta_2[1]) << 0);

    let mut non_adjusted_2 = state;
    non_adjusted_2[0] += delta_2[0];
    non_adjusted_2[2] += delta_2[1];

    // d = 2 & r = +1
    let delta_3 = [1 & bottom_wall_0, 1 & bottom_wall_1];

    let delta_3_i =
        (delta_3[0] << 3) | (delta_3[1] << 2) | (0 << 1) | ((delta_3[0] | delta_3[1]) << 0);

    let mut non_adjusted_3 = state;
    non_adjusted_3[1] += delta_3[0];
    non_adjusted_3[3] += delta_3[1];

    handle_non_adjusted(delta_0_i as u8, non_adjusted_0);
    handle_non_adjusted(delta_1_i as u8, non_adjusted_1);
    handle_non_adjusted(delta_2_i as u8, non_adjusted_2);
    handle_non_adjusted(delta_3_i as u8, non_adjusted_3);
}

pub fn bfs_2d_distances<const RESPECT_HOLES: bool, const DEFAULT_VALUE: usize>(
    start_state: [Coordinate; 2],
    width: Coordinate,
    map: &Map,
    distances: &mut [usize],
) {
    let mut tasks = vec![];
    let mut output = vec![];
    tasks.push(start_state);

    let index =
        |state: [i16; 2]| -> usize { state[1] as usize * width as usize + state[0] as usize };

    distances[index(start_state)] = 0;

    for dist in 1.. {
        output.reserve(tasks.len() * 3);
        for task in tasks.drain(..) {
            for instruction in ALL_INSTRUCTIONS {
                let mut state = task;
                apply_instruction::<RESPECT_HOLES>(instruction, map, &mut state);
                let i = index(state);
                let i_dist = &mut distances[i];
                if *i_dist == DEFAULT_VALUE {
                    *i_dist = dist;
                    output.push(state);
                }
            }
        }

        std::mem::swap(&mut tasks, &mut output);

        if tasks.is_empty() {
            break;
        }
    }
}
