use std::{cmp::Reverse, collections::BinaryHeap, time::Instant};

use crate::{
    bfs::{bfs_2d_distances, handle_single_4d_state, Callback},
    calculate_visited_index,
    delta_list::{BitSetDeltaList, DeltaList, HashMapLazyDeltaList},
    end_state, Coordinate, Map,
};

pub trait AStarPriorityQueue: Sized {
    fn new(width: usize, height: usize, maps: &[Map; 2]) -> Option<Self>;

    fn push(&mut self, state: [Coordinate; 4]);

    fn pop(&mut self) -> Option<[Coordinate; 4]>;
}

struct GenericPriorityQueue<T> {
    tasks: Vec<Vec<[Coordinate; 4]>>,
    heap: BinaryHeap<T>,
}

trait GenericPriorityQueueOrdContainer {
    fn into_usize(self) -> usize;

    fn from_usize(val: usize) -> Self;
}

impl GenericPriorityQueueOrdContainer for usize {
    fn into_usize(self) -> usize {
        self
    }

    fn from_usize(val: usize) -> Self {
        val
    }
}

impl GenericPriorityQueueOrdContainer for Reverse<usize> {
    fn into_usize(self) -> usize {
        self.0
    }

    fn from_usize(val: usize) -> Self {
        Self(val)
    }
}

impl<T> GenericPriorityQueue<T>
where
    T: GenericPriorityQueueOrdContainer,
    T: Clone,
    T: Ord + PartialOrd + Eq + PartialEq,
{
    pub fn new(indices: usize) -> Self {
        Self {
            tasks: vec![vec![]; indices],
            heap: BinaryHeap::with_capacity(indices),
        }
    }

    #[inline(always)]
    pub fn push(&mut self, i: usize, state: [Coordinate; 4]) {
        if self.tasks[i].is_empty() {
            self.heap.push(T::from_usize(i));
        }
        self.tasks[i].push(state);
    }

    #[inline(always)]
    pub fn pop(&mut self) -> Option<[Coordinate; 4]> {
        let i: usize = self.heap.peek()?.clone().into_usize();
        let tasks = &mut self.tasks[i];
        let task = tasks.pop().unwrap();
        if tasks.is_empty() {
            self.heap.pop();
        }
        Some(task)
    }
}

#[test]
#[cfg(test)]
fn priority_queue_test() {
    let mut queue = GenericPriorityQueue::<Reverse<usize>>::new(1025);
    queue.push(0, [0; 4]);
    queue.push(1024, [1; 4]);
    assert_eq!(queue.pop(), Some([0; 4]));
    queue.push(1022, [2; 4]);
    queue.push(1023, [3; 4]);
    assert_eq!(queue.pop(), Some([2; 4]));
    assert_eq!(queue.pop(), Some([3; 4]));
    assert_eq!(queue.pop(), Some([1; 4]));
    assert_eq!(queue.pop(), None);

    let mut queue = GenericPriorityQueue::<usize>::new(1025);
    queue.push(0, [0; 4]);
    queue.push(1024, [1; 4]);
    assert_eq!(queue.pop(), Some([1; 4]));
    queue.push(1022, [2; 4]);
    queue.push(1023, [3; 4]);
    assert_eq!(queue.pop(), Some([3; 4]));
    assert_eq!(queue.pop(), Some([2; 4]));
    assert_eq!(queue.pop(), Some([0; 4]));
    assert_eq!(queue.pop(), None);
}

pub struct ManhattanDistancePriorityQueue(GenericPriorityQueue<usize>);

impl AStarPriorityQueue for ManhattanDistancePriorityQueue {
    fn new(width: usize, height: usize, _map: &[Map; 2]) -> Option<Self> {
        Some(Self(GenericPriorityQueue::new((width + height) * 2)))
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
pub struct DisparityPunishableManhattanDistancePriorityQueue(GenericPriorityQueue<usize>);

impl AStarPriorityQueue for DisparityPunishableManhattanDistancePriorityQueue {
    fn new(width: usize, height: usize, _map: &[Map; 2]) -> Option<Self> {
        Some(Self(GenericPriorityQueue::new((width + height) * 2)))
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
    queue: GenericPriorityQueue<Reverse<usize>>,
    width: usize,
    distances: [Vec<usize>; 2],
}

impl<const RESPECT_HOLES: bool> AStarPriorityQueue
    for SingleBFSDistancePriorityQueue<RESPECT_HOLES>
{
    fn new(width: usize, height: usize, maps: &[Map; 2]) -> Option<Self> {
        let mut distances = std::array::from_fn(|_| vec![usize::MAX; width * height]);

        let mut max_dist_sum = 0;

        let mut tasks = vec![];
        let mut output = vec![];

        for i in 0..2 {
            let map = &maps[i];
            let distances = &mut distances[i];

            let mut max_dist = 0;

            bfs_2d_distances::<RESPECT_HOLES, { usize::MAX }>(
                &mut tasks,
                &mut output,
                [width as Coordinate - 1, height as Coordinate - 1],
                width as Coordinate,
                map,
                distances,
                &mut max_dist,
            );

            max_dist_sum += max_dist;
        }

        if distances[0][0] == usize::MAX || distances[1][0] == usize::MAX {
            None
        } else {
            Some(Self {
                queue: GenericPriorityQueue::new(max_dist_sum + 1),
                width,
                distances,
            })
        }
    }

    #[inline(always)]
    fn push(&mut self, state: [Coordinate; 4]) {
        let i1 = self.distances[0][Map::tile_index_with(state[0], state[1], self.width)];
        let i2 = self.distances[1][Map::tile_index_with(state[2], state[3], self.width)];
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
    callback: &mut impl Callback,
    use_hash_map_first: bool,
) {
    let elapsed = Instant::now();

    let width_u = width as usize;
    let height_u = height as usize;
    let tiles_count = width_u * height_u;
    let states_count = tiles_count.pow(2);

    let end = calculate_visited_index(end_state(width, height), width_u, tiles_count);
    #[cfg(feature = "written_count")]
    {
        crate::delta_list::written_start(states_count);
    }
    let mut output = Vec::<[Coordinate; 4]>::with_capacity(4);

    macro_rules! report {
        ($list: expr) => {
            #[cfg(feature = "written_count")]
            {
                crate::delta_list::written_end(&$list);
            }
            println!("A* time elapsed: {:?}", elapsed.elapsed());
            callback.callback(width_u, height_u, tiles_count, maps, &mut $list);
        };
    }

    if let Some(mut queue) = Q::new(width_u, height_u, maps) {
        macro_rules! search {
            ($list: expr) => {
                let Some(state) = queue.pop() else {
                    break false;
                };

                unsafe {
                    // len is always 0 and capacity is always 4
                    handle_single_4d_state::<RESPECT_HOLES>(
                        maps,
                        width_u,
                        height_u,
                        tiles_count,
                        state,
                        &mut output,
                        &mut $list,
                    );
                }

                if $list.get(end) != 0 {
                    break false;
                }

                for new_state in output.drain(..) {
                    queue.push(new_state);
                }
            };
        }

        queue.push([0; 4]);

        if let Some(mut list) = if use_hash_map_first {
            let mut list = HashMapLazyDeltaList::new(states_count);
            let convert = loop {
                search!(list);

                if list.is_bitset_conversion_worth(states_count) {
                    break true;
                }
            };
            if convert {
                Some(list.into_bitset(states_count))
            } else {
                report!(list);
                None
            }
        } else {
            Some(BitSetDeltaList::new(states_count))
        } {
            let _ = loop {
                search!(list);
            };
            report!(list);
        }
    };
}
