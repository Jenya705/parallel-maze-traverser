use std::{
    collections::hash_map::Entry,
    sync::atomic::{AtomicU64, AtomicU8, Ordering},
};

use fixedbitset::FixedBitSet;
use rustc_hash::FxHashMap;

/// Gibt eine Zahl mit den gegebenen Bits zurück
#[inline(always)]
fn bits2val(bits: [bool; 4]) -> u8 {
    0 | ((bits[0] as u8) << 3)
        | ((bits[1] as u8) << 2)
        | ((bits[2] as u8) << 1)
        | ((bits[3] as u8) << 0)
}

/// Gibt 4 Bits einer Zahl zurück
#[inline(always)]
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

    /// FORCED means that the given index is not allowed to be filled!
    ///
    /// When FORCED is true, the function always returns true (i.e. always changes it's state)
    fn set<const FORCED: bool>(&mut self, index: usize, value: u8) -> bool;

    /// FORCED means that the given index is not allowed to be filled!
    fn set_bits<const FORCED: bool>(&mut self, index: usize, bits: [bool; 4]) -> bool {
        self.set::<FORCED>(index, bits2val(bits))
    }

    fn get(&self, index: usize) -> u8;

    fn get_bits(&self, index: usize) -> [bool; 4] {
        val2bits(self.get(index))
    }

    /// Es ist nur dann aktiviert, wenn das Programm mit dem folgenden Feature kompiliert ist.
    /// 
    /// Ansonst wird die Anzahl der geschriebenen Elementen nicht gezählt.
    #[cfg(feature = "written_count")]
    fn written(&self) -> usize;
}

pub struct BitSetDeltaList<const LEN: usize> {
    bit_sets: [FixedBitSet; LEN],
    #[cfg(feature = "written_count")]
    written: usize,
}

impl<const LEN: usize> BitSetDeltaList<LEN> {
    pub fn inner_new(len: usize) -> Self {
        Self {
            bit_sets: std::array::from_fn(|_| FixedBitSet::with_capacity(len)),
            #[cfg(feature = "written_count")]
            written: 0,
        }
    }

    pub fn inner_get_bit(&self, index: usize, bit: usize) -> bool {
        self.bit_sets[bit].contains(index)
    }

    pub fn inner_set_bits<const FORCED: bool>(&mut self, index: usize, bits: [bool; LEN]) -> bool {
        if FORCED || self.inner_get_bits(index) == [false; LEN] {
            for i in 0..LEN {
                self.bit_sets[i].set(index, bits[i]);
            }
            #[cfg(feature = "written_count")]
            {
                self.written += 1;
            }
            true
        } else {
            false
        }
    }

    pub fn inner_get_bits(&self, index: usize) -> [bool; LEN] {
        let mut res = [false; LEN];
        for i in 0..LEN {
            res[i] = self.bit_sets[i].contains(index);
        }
        res
    }

    pub fn inner_clear(&mut self) {
        for i in 0..LEN {
            self.bit_sets[i].clear();
        }
    }

    #[cfg(feature = "written_count")]
    pub fn inner_written(&self) -> usize {
        self.written
    }
}

impl DeltaList for BitSetDeltaList<4> {
    fn new(len: usize) -> Self {
        Self::inner_new(len)
    }

    fn set<const FORCED: bool>(&mut self, index: usize, value: u8) -> bool {
        self.set_bits::<FORCED>(index, val2bits(value))
    }

    fn set_bits<const FORCED: bool>(&mut self, index: usize, bits: [bool; 4]) -> bool {
        self.inner_set_bits::<FORCED>(index, bits)
    }

    fn get(&self, index: usize) -> u8 {
        bits2val(self.get_bits(index))
    }

    fn get_bits(&self, index: usize) -> [bool; 4] {
        self.inner_get_bits(index)
    }

    #[cfg(feature = "written_count")]
    fn written(&self) -> usize {
        self.inner_written()
    }
}

pub struct HashMapLazyDeltaList {
    map: FxHashMap<usize, u8>,
}

impl HashMapLazyDeltaList {
    pub fn into_bitset(self, len: usize) -> BitSetDeltaList<4> {
        let mut list = BitSetDeltaList::new(len);
        for (key, value) in self.map {
            list.set::<true>(key, value);
        }
        list
    }

    pub fn is_bitset_conversion_worth(&self, len: usize) -> bool {
        // I didn't forget about u8
        self.map.len() * (std::mem::size_of::<usize>() / 2) >= len
    }
}

impl DeltaList for HashMapLazyDeltaList {
    fn new(_len: usize) -> Self {
        Self {
            map: FxHashMap::default(),
        }
    }

    fn set<const FORCED: bool>(&mut self, index: usize, value: u8) -> bool {
        match self.map.entry(index) {
            Entry::Occupied(_) if !FORCED => false,
            Entry::Vacant(vacant_entry) => {
                vacant_entry.insert(value);
                true
            }
            _ => unreachable!(),
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

/// Eine Brücke zwischen einem AsyncDeltaList und einem DeltaList (Nimmt eine AsyncDeltaList und stellt die als eine Sync-DeltaList vor)
/// 
/// Die Struktur wird dafür benutzt, um die Implementation der parallen und nicht-parallelen Algorithmen zu verallgemeinern
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

    fn set<const FORCED: bool>(&mut self, index: usize, value: u8) -> bool {
        self.list.set::<FORCED>(index, value)
    }

    fn set_bits<const FORCED: bool>(&mut self, index: usize, bits: [bool; 4]) -> bool {
        self.list.set_bits::<FORCED>(index, bits)
    }

    fn get(&self, index: usize) -> u8 {
        self.list.get(index)
    }

    fn get_bits(&self, index: usize) -> [bool; 4] {
        self.list.get_bits(index)
    }

    #[cfg(feature = "written_count")]
    fn written(&self) -> usize {
        self.list.written()
    }
}

pub trait AsyncDeltaList {
    fn new(len: usize) -> Self;

    fn set<const FORCED: bool>(&self, index: usize, value: u8) -> bool;

    fn set_bits<const FORCED: bool>(&self, index: usize, bits: [bool; 4]) -> bool {
        self.set::<FORCED>(index, bits2val(bits))
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

    fn set<const FORCED: bool>(&self, index: usize, value: u8) -> bool {
        let (vindex, vbit) = Self::visited_index(index);

        let visited_val = self.visited[vindex].fetch_or(1 << vbit, Ordering::Relaxed);

        if FORCED || (visited_val & (1 << vbit) == 0) {
            let (index, bit) = Self::bit_index(index);
            self.bits[index].fetch_or((value as u64) << bit, Ordering::Relaxed);
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

    fn set<const FORCED: bool>(&self, index: usize, value: u8) -> bool {
        let res = if FORCED {
            self.values[index].store(value, Ordering::Relaxed);
            true
        } else {
            self.values[index]
                .compare_exchange(0, value, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        };

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

pub enum FourBitDeltaListKind {
    BitSet,
    LazyHashMap,
    AtomicBitSet,
    CompareAndSwapAtomicBitSet,
}

#[cfg(feature = "written_count")]
pub fn written_start(len: usize) {
    println!("len: {len}");
}

#[cfg(feature = "written_count")]
pub fn written_end_async(list: &impl AsyncDeltaList) {
    println!("written: {}", list.written());
}

#[cfg(feature = "written_count")]
pub fn written_end(list: &impl DeltaList) {
    println!("written: {}", list.written());
}
