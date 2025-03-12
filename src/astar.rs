use std::time::Instant;

use crate::{
    bfs::{bfs_2d_distances, handle_single_state, BFSCallback},
    calculate_visited_index,
    delta_list::{BitSetDeltaList, DeltaList},
    end_state, Coordinate, Map,
};

pub trait AStarPriorityQueue {
    fn new(width: usize, height: usize, maps: &[Map; 2]) -> Self;

    fn push(&mut self, state: [Coordinate; 4]);

    fn pop(&mut self) -> Option<[Coordinate; 4]>;
}

struct GenericPriorityQueue<const MAX: bool> {
    tasks: Vec<Vec<[Coordinate; 4]>>,
    non_emptiness_bitset: Vec<u128>,
    i_at_least: usize,
}

impl<const MAX: bool> GenericPriorityQueue<MAX> {
    pub fn new(indices: usize) -> Self {
        let bitset_len = (indices + 127) / 128;
        Self {
            tasks: vec![vec![]; indices],
            non_emptiness_bitset: vec![0; bitset_len],
            i_at_least: if MAX { bitset_len - 1 } else { 0 },
        }
    }

    #[inline(always)]
    pub fn push(&mut self, i: usize, state: [Coordinate; 4]) {
        self.tasks[i].push(state);
        self.non_emptiness_bitset[i / 128] |= 1 << (i % 128);
        self.set_at_least(i);
    }

    #[inline(always)]
    fn set_at_least(&mut self, i: usize) {
        self.i_at_least = if MAX {
            self.i_at_least.max(i / 128)
        } else {
            self.i_at_least.min(i / 128)
        }
    }

    #[inline(always)]
    fn max(&self) -> Option<(usize, usize)> {
        let (i, v) = self.non_emptiness_bitset[..=self.i_at_least]
            .iter()
            .cloned()
            .enumerate()
            .rev()
            .find(|&(_, v)| v != 0)?;
        let c = 127 - v.leading_zeros() as usize;
        Some((i, c))
    }

    #[inline(always)]
    fn min(&self) -> Option<(usize, usize)> {
        let (i, v) = self.non_emptiness_bitset[self.i_at_least..]
            .iter()
            .cloned()
            .enumerate()
            .find(|&(_, v)| v != 0)?;
        let c = v.trailing_zeros() as usize;
        Some((i, c))
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Option<[Coordinate; 4]> {
        let (i, c) = if MAX { self.max() } else { self.min() }?;
        let tasks = &mut self.tasks[128 * i + c];
        let task = tasks.pop().unwrap();
        self.non_emptiness_bitset[i] ^= (1 & (tasks.is_empty() as u128)) << c;
        self.i_at_least = i/128;
        Some(task)
    }
}

#[test]
#[cfg(test)]
fn priority_queue_test() {
    let mut queue = GenericPriorityQueue::<false>::new(1025);
    queue.push(0, [0; 4]);
    queue.push(1024, [1; 4]);
    assert_eq!(queue.pop(), Some([0; 4]));
    queue.push(1022, [2; 4]);
    queue.push(1023, [3; 4]);
    assert_eq!(queue.pop(), Some([2; 4]));
    assert_eq!(queue.pop(), Some([3; 4]));
    assert_eq!(queue.pop(), Some([1; 4]));
    assert_eq!(queue.pop(), None);
}

pub struct ManhattanDistancePriorityQueue(GenericPriorityQueue<true>);

impl AStarPriorityQueue for ManhattanDistancePriorityQueue {
    fn new(width: usize, height: usize, _map: &[Map; 2]) -> Self {
        Self(GenericPriorityQueue::new((width + height) * 2))
    }

    #[inline(always)]
    fn push(&mut self, state: [Coordinate; 4]) {
        let i = (state[0] + state[1] + state[2] + state[3]) as usize;
        self.0.push(i, state);
    }

    #[inline(always)]
    fn pop(&mut self) -> Option<[Coordinate; 4]> {
        self.0.pop()
    }
}
pub struct DisparityPunishableManhattanDistancePriorityQueue(GenericPriorityQueue<true>);

impl AStarPriorityQueue for DisparityPunishableManhattanDistancePriorityQueue {
    fn new(width: usize, height: usize, _map: &[Map; 2]) -> Self {
        Self(GenericPriorityQueue::new((width + height) * 2))
    }

    #[inline(always)]
    fn push(&mut self, state: [Coordinate; 4]) {
        let i1 = state[0] + state[1];
        let i2 = state[2] + state[3];
        let i = i1 + i2 + i2.min(i1) - i2.max(i1);
        self.0.push(i as usize, state);
    }

    #[inline(always)]
    fn pop(&mut self) -> Option<[Coordinate; 4]> {
        self.0.pop()
    }
}

pub struct SingleBFSDistancePriorityQueue<const RESPECT_HOLES: bool> {
    queue: GenericPriorityQueue<false>,
    width: usize,
    distances: [Vec<usize>; 2],
}

impl<const RESPECT_HOLES: bool> AStarPriorityQueue
    for SingleBFSDistancePriorityQueue<RESPECT_HOLES>
{
    fn new(width: usize, height: usize, maps: &[Map; 2]) -> Self {
        let mut distances = std::array::from_fn(|_| vec![usize::MAX; width * height]);

        let mut max_dist_sum = 0;

        for i in 0..2 {
            let map = &maps[i];
            let distances = &mut distances[i];

            bfs_2d_distances::<RESPECT_HOLES, { usize::MAX }>(
                [width as Coordinate - 1, height as Coordinate - 1],
                width as Coordinate,
                map,
                distances,
            );

            if let Some(&v) = distances.iter().filter(|&&v| v != usize::MAX).max() {
                max_dist_sum += v as usize;
            }
        }

        Self {
            queue: GenericPriorityQueue::new(max_dist_sum + 1),
            width,
            distances,
        }
    }

    #[inline(always)]
    fn push(&mut self, state: [Coordinate; 4]) {
        let i1 = self.distances[0][state[1] as usize * self.width + state[0] as usize];
        let i2 = self.distances[1][state[3] as usize * self.width + state[2] as usize];
        self.queue.push(i1 + i2, state);
    }

    #[inline(always)]
    fn pop(&mut self) -> Option<[Coordinate; 4]> {
        self.queue.pop()
    }
}

pub fn launch_astar<Q: AStarPriorityQueue, const RESPECT_HOLES: bool>(
    width: Coordinate,
    height: Coordinate,
    maps: &[Map; 2],
    callback: &mut impl BFSCallback,
) {
    let elapsed = Instant::now();

    let width_u = width as usize;
    let height_u = height as usize;
    let tiles_count = width_u * height_u;
    let states_count = tiles_count.pow(2);

    let end = calculate_visited_index(end_state(width, height), width_u, tiles_count);
    let mut list = BitSetDeltaList::new(states_count);
    #[cfg(feature = "written_count")]
    {
        crate::delta_list::written_start(states_count);
    }

    let mut output = Vec::<[Coordinate; 4]>::with_capacity(4);

    let mut queue = Q::new(width_u, height_u, maps);
    queue.push([0; 4]);

    loop {
        let Some(state) = queue.pop() else {
            break;
        };

        unsafe {
            // len is always 0 and capacity is always 4
            handle_single_state::<RESPECT_HOLES>(
                maps,
                width_u,
                tiles_count,
                state,
                &mut output,
                &mut list,
            );
        }

        if list.get(end) != 0 {
            break;
        }

        for new_state in output.drain(..) {
            queue.push(new_state);
        }
    }
    #[cfg(feature = "written_count")]
    {
        crate::delta_list::written_end(&list);
    }
    println!("A* time elapsed: {:?}", elapsed.elapsed());
    callback.callback(width_u, height_u, tiles_count, maps, &mut list);
}
