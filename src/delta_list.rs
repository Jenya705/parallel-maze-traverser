use std::{
    collections::{hash_map::Entry, HashMap},
    sync::atomic::{AtomicU64, AtomicU8, Ordering},
};

use fixedbitset::FixedBitSet;

fn bits2val(bits: [bool; 4]) -> u8 {
    0 | ((bits[0] as u8) << 3)
        | ((bits[1] as u8) << 2)
        | ((bits[2] as u8) << 1)
        | ((bits[3] as u8) << 0)
}

fn val2bits(val: u8) -> [bool; 4] {
    [
        val & 0b1000 != 0,
        val & 0b0100 != 0,
        val & 0b0010 != 0,
        val & 0b0001 != 0,
    ]
}

pub trait DeltaList {
    fn new(len: usize) -> Self;

    fn set(&mut self, index: usize, value: u8) -> bool;

    fn set_bits(&mut self, index: usize, bits: [bool; 4]) -> bool {
        self.set(index, bits2val(bits))
    }

    fn get(&self, index: usize) -> u8;

    fn get_bits(&self, index: usize) -> [bool; 4] {
        val2bits(self.get(index))
    }

    #[cfg(feature = "written_count")]
    fn written(&self) -> usize;
}

pub struct BitSetDeltaList {
    bit_sets: [FixedBitSet; 4],
    #[cfg(feature = "written_count")]
    written: usize,
}

impl DeltaList for BitSetDeltaList {
    fn new(len: usize) -> Self {
        Self {
            bit_sets: std::array::from_fn(|_| FixedBitSet::with_capacity(len)),
            #[cfg(feature = "written_count")]
            written: 0,
        }
    }

    fn set(&mut self, index: usize, value: u8) -> bool {
        self.set_bits(index, val2bits(value))
    }

    fn set_bits(&mut self, index: usize, bits: [bool; 4]) -> bool {
        if self.get(index) != 0 {
            false
        } else {
            for i in 0..4 {
                self.bit_sets[i].set(index, bits[i]);
            }
            #[cfg(feature = "written_count")]
            {
                self.written += 1;
            }
            true
        }
    }

    fn get(&self, index: usize) -> u8 {
        bits2val(self.get_bits(index))
    }

    fn get_bits(&self, index: usize) -> [bool; 4] {
        let mut res = [false; 4];
        for i in 0..4 {
            res[i] = self.bit_sets[i].contains(index);
        }
        res
    }

    #[cfg(feature = "written_count")]
    fn written(&self) -> usize {
        self.written
    }
}

pub struct HashMapLazyDeltaList {
    map: HashMap<usize, u8>,
}

impl DeltaList for HashMapLazyDeltaList {
    fn new(_len: usize) -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn set(&mut self, index: usize, value: u8) -> bool {
        match self.map.entry(index) {
            Entry::Occupied(_) => false,
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(value);
                true
            }
        }
    }

    fn get(&self, index: usize) -> u8 {
        self.map.get(&index).cloned().unwrap_or(0)
    }

    #[cfg(feature = "written_count")]
    fn written(&self) -> usize {
        self.map.len()
    }
}

pub struct AsyncDeltaListAccessor<'a, T> {
    pub list: &'a T,
}

impl<'a, T> DeltaList for AsyncDeltaListAccessor<'a, T>
where
    T: AsyncDeltaList,
{
    fn new(_len: usize) -> Self {
        unimplemented!()
    }

    fn set(&mut self, index: usize, value: u8) -> bool {
        self.list.set(index, value)
    }

    fn set_bits(&mut self, index: usize, bits: [bool; 4]) -> bool {
        self.list.set_bits(index, bits)
    }

    fn get(&self, index: usize) -> u8 {
        self.list.get(index)
    }

    fn get_bits(&self, index: usize) -> [bool; 4] {
        self.list.get_bits(index)
    }

    #[cfg(feature = "written_count")]
    fn written(&self) -> usize {
        unimplemented!()
    }
}

pub trait AsyncDeltaList {
    fn new(len: usize) -> Self;

    fn set(&self, index: usize, value: u8) -> bool;

    fn set_bits(&self, index: usize, bits: [bool; 4]) -> bool {
        self.set(index, bits2val(bits))
    }

    fn get(&self, index: usize) -> u8;

    fn get_bits(&self, index: usize) -> [bool; 4] {
        val2bits(self.get(index))
    }

    #[cfg(feature = "written_count")]
    fn written(&self) -> usize;
}

pub struct AtomicBitSetDeltaList {
    visited: Vec<AtomicU64>,
    bits: Vec<AtomicU64>,
    #[cfg(feature = "written_count")]
    written: AtomicU64,
}

impl AtomicBitSetDeltaList {
    fn visited_index(index: usize) -> (usize, usize) {
        (index / 64, index % 64)
    }

    fn bit_index(index: usize) -> (usize, usize) {
        (index / 16, (index % 16) * 4)
    }
}

impl AsyncDeltaList for AtomicBitSetDeltaList {
    fn new(len: usize) -> Self {
        let mut visited = vec![];
        visited.resize_with(len / 64 + 1, || AtomicU64::new(0));
        let mut bits = vec![];
        bits.resize_with(len / 16 + 1, || AtomicU64::new(0));
        Self {
            visited,
            bits,
            #[cfg(feature = "written_count")]
            written: AtomicU64::new(0),
        }
    }

    fn set(&self, index: usize, value: u8) -> bool {
        let (vindex, vbit) = Self::visited_index(index);

        if self.visited[vindex].fetch_or(1 << vbit, Ordering::Relaxed) & (1 << vbit) == 0 {
            let (index, bit) = Self::bit_index(index);
            self.bits[index].fetch_xor((value as u64) << bit, Ordering::Relaxed);
            #[cfg(feature = "written_count")]
            {
                self.written.fetch_add(1, Ordering::Relaxed);
            }
            true
        } else {
            false
        }
    }

    fn get(&self, index: usize) -> u8 {
        let (index, bit) = Self::bit_index(index);
        ((self.bits[index].load(Ordering::Relaxed) >> bit) & 0b1111) as u8
    }

    #[cfg(feature = "written_count")]
    fn written(&self) -> usize {
        self.written.load(Ordering::Relaxed) as usize
    }
}

pub struct CompareAndSwapAtomicBitSetDeltaList {
    values: Vec<AtomicU8>,
    #[cfg(feature = "written_count")]
    written: AtomicU64,
}

impl AsyncDeltaList for CompareAndSwapAtomicBitSetDeltaList {
    fn new(len: usize) -> Self {
        let mut values = vec![];
        values.resize_with(len, || AtomicU8::new(0));
        Self {
            values,
            #[cfg(feature = "written_count")]
            written: AtomicU64::new(0),
        }
    }

    fn set(&self, index: usize, value: u8) -> bool {
        let res = self.values[index]
            .compare_exchange(0, value, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok();

        #[cfg(feature = "written_count")]
        {
            if res {
                self.written.fetch_add(1, Ordering::Relaxed);
            }
        }

        res
    }

    fn get(&self, index: usize) -> u8 {
        self.values[index].load(Ordering::Relaxed)
    }

    #[cfg(feature = "written_count")]
    fn written(&self) -> usize {
        self.written.load(Ordering::Relaxed) as usize
    }
}

pub enum DeltaListKind {
    BitSet,
    LazyHashMap,
    AtomicBitSet,
    CompareAndSwapAtomicBitSet,
}

#[cfg(feature="written_count")]
pub fn written_start(len: usize) {
    println!("len: {len}");
}

#[cfg(feature="written_count")]
pub fn written_end_async(list: &impl AsyncDeltaList) {
    println!("written: {}", list.written());
}

#[cfg(feature="written_count")]
pub fn written_end(list: &impl DeltaList) {
    println!("written: {}", list.written());
}
