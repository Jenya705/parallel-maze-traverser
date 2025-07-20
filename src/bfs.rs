use std::{
    cell::SyncUnsafeCell,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Instant,
};

use crate::{
    calculate_visited_index,
    delta_list::{
        AsyncDeltaList, AsyncDeltaListAccessor, AtomicBitSetDeltaList, BitSetDeltaList,
        CompareAndSwapAtomicBitSetDeltaList, DeltaList, FourBitDeltaListKind, HashMapLazyDeltaList,
    },
    end_state,
    instructions::{apply_instruction, apply_instructions, maximum_instructions, ALL_INSTRUCTIONS},
    Coordinate, Map,
};

pub trait Callback {
    fn callback(
        &mut self,
        width: usize,
        height: usize,
        tiles_count: usize,
        maps: &[Map; 2],
        list: &impl DeltaList,
    );
}

pub fn launch_bfs<const RESPECT_HOLES: bool>(
    width: Coordinate,
    height: Coordinate,
    maps: Arc<[Map; 2]>,
    threads: usize,
    kind: FourBitDeltaListKind,
    callback: &mut impl Callback,
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
            list.set::<true>(0, 1);
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
            list.set::<true>(0, 1);
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
        FourBitDeltaListKind::BitSet => {
            bit_set_launch!(BitSetDeltaList::<4>);
        }
        FourBitDeltaListKind::LazyHashMap => {
            bit_set_launch!(HashMapLazyDeltaList);
        }
        FourBitDeltaListKind::AtomicBitSet => {
            async_bit_set_launch!(AtomicBitSetDeltaList);
        }
        FourBitDeltaListKind::CompareAndSwapAtomicBitSet => {
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
    tasks.push([0; 4]);
    let mut output = vec![];

    // Anhand des Lemmas über die maximale Länge einer optimalen Lösung
    // kann die Tiefe der Suche begrenzt werden
    let mut instructions_left = maximum_instructions(maps);

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

        if tasks.is_empty() || instructions_left == 0 {
            return;
        }
        instructions_left -= 1;
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
    // SyncUnsafeCell wird dafür benutzt, um die Borrow-Regeln von Rust zu ignorieren.
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

    let done = Arc::new(AtomicBool::new(false));

    for i in 0..threads {
        let thread_tasks = Arc::clone(&thread_tasks);
        let thread_outputs = Arc::clone(&thread_outputs);

        let notifier = Arc::new((Mutex::new(false), Condvar::new()));

        notifiers.push(Arc::clone(&notifier));

        let maps = Arc::clone(&maps);
        let list = Arc::clone(&list);

        let done = Arc::clone(&done);

        std::thread::spawn(move || loop {
            {
                // Dieses Worker-Thread wartet auf das Main-Thread. 
                drop(
                    notifier
                        .1
                        .wait_while(notifier.0.lock().unwrap(), |run| !*run)
                        .unwrap(),
                );
            }

            // Damit Worker-Threads nicht ständig läuften.
            if done.load(Ordering::Relaxed) {
                break;
            }

            // SAFETY: 
            // - each thread access only their vector i
            // - notifiers control whether the main thread and the worker threads access the data,
            //   thus preventing any parallel access between these threads.
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
                // Main-Thread registriert dieses Worker-Thread hat seine Aufgabe erfüllt.
                *notifier.0.lock().unwrap() = false;
                notifier.1.notify_all();
            }
        });
    }

    macro_rules! notify_threads {
        () => {
            for notifier in &notifiers {
                let mut guard = notifier.0.lock().unwrap();
                *guard = true;
                notifier.1.notify_all();
            }
        };
    }

    // Anhand des Lemmas über die maximale Länge einer optimalen Lösung
    // kann die Tiefe der Suche begrenzt werden
    let mut instructions_left = maximum_instructions(&maps);

    while list.get(end) == 0 {
        notify_threads!();

        // Auf die Worker-Threads warten
        for notifier in &notifiers {
            let guard = notifier.0.lock().unwrap();
            drop(notifier.1.wait_while(guard, |run| *run));
        }

        let mut len = 0;
        for i in 0..threads {
            // SAFETY: see the worker thread explanation
            let input = unsafe { thread_tasks[i].get().as_mut().unwrap() };
            let output = unsafe { thread_outputs[i].get().as_mut().unwrap() };
            std::mem::swap(input, output);
            len += input.len();
        }

        if len == 0 || instructions_left == 0 {
            break;
        }
        instructions_left -= 1;

        let avg_len = len / threads;

        let mut j = 0;

        // Der Bilanzierungsalgorithmus
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

    done.store(true, Ordering::Relaxed);
    notify_threads!();
}

#[inline(always)]
pub fn ensure_capacity(tasks: &Vec<[Coordinate; 4]>, output: &mut Vec<[Coordinate; 4]>) {
    output.reserve(tasks.len() * 4);
}

#[inline(always)]
pub fn single_layer_bfs<const RESPECT_HOLES: bool>(
    tasks: &mut Vec<[Coordinate; 4]>,
    output: &mut Vec<[Coordinate; 4]>,
    maps: &[Map; 2],
    width: usize,
    height: usize,
    tiles_count: usize,
    delta_list: &mut impl DeltaList,
    _end: usize,
) {
    ensure_capacity(tasks, output);

    for state in tasks.drain(..) {
        // SAFETY: ensure_capacity was called
        unsafe {
            handle_single_4d_state::<RESPECT_HOLES>(
                maps,
                width,
                height,
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
#[inline(never)]
pub unsafe fn handle_single_4d_state<const RESPECT_HOLES: bool>(
    maps: &[Map; 2],
    width: usize,
    height: usize,
    tiles_count: usize,
    state: [Coordinate; 4],
    output: &mut Vec<[Coordinate; 4]>,
    delta_list: &mut impl DeltaList,
) {
    // Nimmt den neuen ohne Grubewirkung Zustand und
    // - guckt an, ob der Zustand in einer Grube ist
    // - speichert den Zustand bzw. die Zustände
    let mut handle_non_adjusted = |delta_i: u8, non_adjusted: [Coordinate; 4]| {
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

        if delta_list.set::<false>(adjusted_i, delta_i) {
            let non_adjusted_i = calculate_visited_index(non_adjusted, width, tiles_count);

            if RESPECT_HOLES && (non_adjusted_i != adjusted_i) {
                delta_list.set::<true>(non_adjusted_i, delta_i);
            }

            output.as_mut_ptr().add(output.len()).write(adjusted);
            output.set_len(output.len() + 1);
        }
    };

    // Sind die gegebene Positionen am Ende?
    let state0end = state[1] == height as Coordinate - 1 && state[0] == width as Coordinate - 1;
    let state1end = state[3] == height as Coordinate - 1 && state[2] == width as Coordinate - 1;

    let i0h = maps[0].horizontal_wall_index(state[0], state[1]);
    let i0v = maps[0].vertical_wall_index(state[0], state[1]);
    let i1h = maps[1].horizontal_wall_index(state[2], state[3]);
    let i1v = maps[1].vertical_wall_index(state[2], state[3]);

    // Gibt es beim Gänger i eine Wand in diese Richtung?
    // Falls er schon am Ende ist, dann ist er von theoretischen Wänden blockiert. 
    let left_wall_0 = (!state0end && !maps[0].vertical_walls.contains_unchecked(i0v)) as Coordinate;
    let left_wall_1 = (!state1end && !maps[1].vertical_walls.contains_unchecked(i1v)) as Coordinate;

    let right_wall_0 =
        (!state0end && !maps[0].vertical_walls.contains_unchecked(i0v + 1)) as Coordinate;
    let right_wall_1 =
        (!state1end && !maps[1].vertical_walls.contains_unchecked(i1v + 1)) as Coordinate;

    let top_wall_0 =
        (!state0end && !maps[0].horizontal_walls.contains_unchecked(i0h)) as Coordinate;
    let top_wall_1 =
        (!state1end && !maps[1].horizontal_walls.contains_unchecked(i1h)) as Coordinate;

    let bottom_wall_0 =
        (!state0end && !maps[0].horizontal_walls.contains_unchecked(i0h + 1)) as Coordinate;
    let bottom_wall_1 =
        (!state1end && !maps[1].horizontal_walls.contains_unchecked(i1h + 1)) as Coordinate;

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

pub fn launch_bfs_2d<const RESPECT_HOLES: bool>(
    width: Coordinate,
    height: Coordinate,
    maps: &[Map; 2],
) -> Vec<[bool; 2]> {
    let timer = Instant::now();

    let mut instructions = vec![];

    let mut tasks = vec![];
    let mut output = vec![];

    let width_u = width as usize;
    let height_u = height as usize;

    let mut list = BitSetDeltaList::<3>::inner_new(width_u * height_u);

    if bfs_2d::<RESPECT_HOLES>(&mut tasks, &mut output, [0; 2], &maps[0], &mut list) {
        // wenn ein Weg gefunden wurde
        bfs_2d_reconstruction::<RESPECT_HOLES>(&list, &maps[0], [0; 2], &mut instructions);
        let mut start_state = [0; 2];
        // simulieren die Instruktionen für den zweiten Gänger
        for &instruction in instructions.iter() {
            apply_instruction::<RESPECT_HOLES>(instruction, &maps[1], &mut start_state, true);
        }

        // falls er schon am Ende ist, dann muss nichts berechnet werden
        if start_state != [width - 1, height - 1] {
            // das Bitset soll leer sein
            list.inner_clear();
            if bfs_2d::<RESPECT_HOLES>(&mut tasks, &mut output, start_state, &maps[1], &mut list) {
                bfs_2d_reconstruction::<RESPECT_HOLES>(
                    &list,
                    &maps[1],
                    start_state,
                    &mut instructions,
                );
            } else {
                // kein Weg wurde gefunden => markieren, dass keine Lösung existiert
                instructions.clear();
            }
        }
    }

    println!("2d-BFS time elapsed: {:?}", timer.elapsed());

    instructions
}

pub fn bfs_2d<const RESPECT_HOLES: bool>(
    tasks: &mut Vec<[Coordinate; 2]>,
    output: &mut Vec<[Coordinate; 2]>,
    start_state: [Coordinate; 2],
    map: &Map,
    list: &mut BitSetDeltaList<3>,
) -> bool {
    tasks.clear();
    output.clear();

    // [x_dimension, direction, written] ist die Bitrepräsentation der Struktur, die im Bitset list gespeichert wird

    let width = map.width as usize;

    list.inner_set_bits::<true>(Map::tile_index_with_vec(start_state, width), [true; 3]);
    tasks.push(start_state);

    let end = Map::tile_index_with_vec([map.width - 1, map.height - 1], width);

    loop {
        if tasks.is_empty() {
            break false;
        }

        // Aus jedem Zustand können maximal 3 neue Zustände erzeugt
        output.reserve(tasks.len() * 3);
        for task in tasks.drain(..) {
            for instruction in ALL_INSTRUCTIONS {
                let mut state = task;
                apply_instruction::<RESPECT_HOLES>(instruction, map, &mut state, false);

                if list.inner_set_bits::<false>(
                    Map::tile_index_with_vec(state, width),
                    [instruction[0], instruction[1], true],
                ) {
                    output.push(state);
                }
            }
        }

        // Das 3. Bit besagt, ob das Element leer ist. 
        if list.inner_get_bit(end, 2) {
            break true;
        }

        std::mem::swap(output, tasks);
    }
}

pub fn bfs_2d_reconstruction<const RESPECT_HOLES: bool>(
    list: &BitSetDeltaList<3>,
    map: &Map,
    start_state: [Coordinate; 2],
    instructions: &mut Vec<[bool; 2]>,
) {
    let mut dirs = vec![];

    let width = map.width as usize;

    let mut state = [map.width - 1, map.height - 1];

    while state != start_state {
        let delta_i = list.inner_get_bits(Map::tile_index_with_vec(state, width));

        if RESPECT_HOLES && state == [0; 2] {
            for &hole_position in map.holes_placement.iter() {
                if list.inner_get_bit(Map::tile_index_with_vec(hole_position, width), 2) {
                    state = hole_position;
                    break;
                }
            }
        }

        apply_instruction::<false>([delta_i[0], !delta_i[1]], map, &mut state, false);

        dirs.push([delta_i[0], delta_i[1]]);
    }

    let i = instructions.len();
    instructions.reserve(dirs.len());
    for dir in dirs.into_iter().rev() {
        instructions.push(dir);
    }

    let mut state = start_state;
    apply_instructions::<RESPECT_HOLES>(instructions[i..].iter().cloned(), map, &mut state);
    println!("valid: {}", state == [map.width - 1, map.height - 1]);
}

pub fn bfs_2d_distances<const RESPECT_HOLES: bool, const DEFAULT_VALUE: usize>(
    tasks: &mut Vec<[Coordinate; 2]>,
    output: &mut Vec<[Coordinate; 2]>,
    start_state: [Coordinate; 2],
    width: Coordinate,
    map: &Map,
    distances: &mut [usize],
    max_dist: &mut usize,
) {
    tasks.clear();
    output.clear();
    tasks.push(start_state);

    distances[Map::tile_index_with_vec(start_state, width as usize)] = 0;

    for dist in 1.. {
        output.reserve(tasks.len() * 3);
        for task in tasks.drain(..) {
            for instruction in ALL_INSTRUCTIONS {
                let mut state = task;
                let visited_hole =
                    apply_instruction::<RESPECT_HOLES>(instruction, map, &mut state, false);
                // if RESPECT_HOLES is false then visited_hole is always false (i.e. no need to check it in the runtime)
                // wenn es keine Gruben gibt, dann konnte keine Grube besucht werden
                if RESPECT_HOLES && visited_hole {
                    continue;
                }
                let i = Map::tile_index_with_vec(state, width as usize);
                let i_dist = &mut distances[i];
                if *i_dist == DEFAULT_VALUE {
                    *i_dist = dist;
                    output.push(state);
                }
            }
        }

        std::mem::swap(tasks, output);

        if tasks.is_empty() {
            *max_dist = dist - 1;
            break;
        }
    }
}
