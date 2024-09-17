//! inspired by <https://github.com/gfx-rs/range-alloc>, but provides very different api.
use std::{
    fmt::Debug,
    ops::{Add, AddAssign, Range, Sub, SubAssign},
};

/// the `RangeAllocError` error indicates an allocation failure that may be due to resource
/// exhaustion or to something wrong when combining the given input arguments with this allocator.
///
/// it is modelled after [`std::alloc::AllocError`].
#[derive(Debug, thiserror::Error, Copy, Clone, PartialEq, Eq)]
#[error("range allocation failed")]
pub struct RangeAllocError;

#[derive(Debug)]
pub struct RangeAlloc<T> {
    full_range: Range<T>,
    free_ranges: Vec<Range<T>>,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct BestFit<T> {
    index: usize,
    pub range: Range<T>,
}

impl<T> RangeAlloc<T>
where
    T: Debug
        + Clone
        + Copy
        // NOTE: default is needed to be able to get zero.
        + Default
        + Sub<Output = T>
        + SubAssign
        + Add<Output = T>
        + AddAssign
        + PartialOrd
        + Ord,
{
    pub fn new(full_range: Range<T>) -> Self {
        assert!(full_range.start < full_range.end);

        Self {
            full_range: full_range.clone(),
            free_ranges: vec![full_range],
        }
    }

    // NOTE: find_best_fit exists because of the idea where it might be appropriate to have a list
    // of range allocs each of which is ~associated with a "buffer". the goal of find_best_fit
    // would be to find best fit along with best range alloc out of list of all range allocs.
    //
    // but how stupid this ^ idea is? wouldn't it make sens to either allocate a buffer that is
    // able to accomodate all the resources (know your data)? or grow the buffer?
    pub fn find_best_fit(&self, len: T) -> Option<BestFit<T>> {
        assert!(len > T::default(), "invalid len");

        // this is an attempt to find best fit for out of bounds len. bail.
        if len > self.full_range.end {
            return None;
        }

        let mut best_range_idx: Option<usize> = None;

        // TODO: benchmark this vs cloned iter to see if it's faster to clone or chase pointers.
        for (i, free_range) in self.free_ranges.iter().enumerate() {
            let free_range_len = free_range.end - free_range.start;

            // doesn't fit
            if free_range_len < len {
                continue;
            }

            // perfect fit
            if free_range_len == len {
                best_range_idx = Some(i);
                break;
            }

            match best_range_idx {
                Some(bri) => {
                    // TODO: benchmark this vs cloned iter to see if it's faster to clone or chase
                    // pointer.
                    let best_range = &self.free_ranges[bri];
                    let best_range_len = best_range.end - best_range.start;
                    if free_range_len < best_range_len {
                        best_range_idx = Some(i);
                    }
                }
                None => best_range_idx = Some(i),
            }
        }

        best_range_idx.map(|index| BestFit {
            index,
            range: self.free_ranges[index].clone(),
        })
    }

    pub fn allocate_best_fit(&mut self, len: T, best_fit: BestFit<T>) -> Range<T> {
        let BestFit { index, range } = best_fit;
        let range_len = range.end - range.start;

        // perfect fit
        if len == range_len {
            self.free_ranges.remove(index);
            return range;
        }

        self.free_ranges[index].start += len;
        range.start..range.start + len
    }

    pub fn allocate(&mut self, len: T) -> Result<Range<T>, RangeAllocError> {
        self.find_best_fit(len)
            .map(|best_fit| self.allocate_best_fit(len, best_fit))
            .ok_or(RangeAllocError)
    }

    fn defragment_free_ranges(&mut self) {
        // merge ranges (with range 10..20)
        // free ranges = [5..10, 20..96]
        // after grow right = [5..20, 20..96]

        self.free_ranges.sort_by_key(|free_range| free_range.start);

        let mut i = 0;
        while i < self.free_ranges.len() - 1 {
            if self.free_ranges[i].end == self.free_ranges[i + 1].start {
                let next = self.free_ranges.remove(i + 1);
                self.free_ranges[i].end = next.end;
            } else {
                i += 1;
            }
        }
    }

    pub fn deallocate(&mut self, range: Range<T>) {
        assert!(range.start < range.end);
        assert!(range.start >= self.full_range.start && range.end <= self.full_range.end);

        let mut did_grow_side = false;
        for free_range in self.free_ranges.iter_mut() {
            // grow right (with range 10..20)
            // free ranges = [5..10] -> [5..20]
            let can_grow_right = free_range.end == range.start;
            if can_grow_right {
                free_range.end = range.end;
                did_grow_side = true;
                break;
            }

            // grow left (with range 10..20)
            // free ranges = [20..96] -> [10..96]
            let can_grow_left = free_range.start == range.end;
            if can_grow_left {
                free_range.start = range.start;
                did_grow_side = true;
                break;
            }
        }

        if !did_grow_side {
            // TODO: maybe instead of pusing to the end try finding a position for insertion to
            // ensure that free ranges are ordered by ascending range start?
            self.free_ranges.push(range);
        }

        self.defragment_free_ranges();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate() {
        let mut ra = RangeAlloc::new(0..100 as u32);
        let _ = ra.allocate(10);
        assert_eq!(&ra.free_ranges, &[10..100]);
    }

    #[test]
    fn deallocate_right() {
        let mut ra = RangeAlloc::new(0..100 as u32);

        // right
        ra.free_ranges = vec![5..10];
        ra.deallocate(10..20);
        assert_eq!(&ra.free_ranges, &[5..20]);
    }

    #[test]
    fn deallocate_left() {
        let mut ra = RangeAlloc::new(0..100 as u32);

        // left
        ra.free_ranges = vec![20..96];
        ra.deallocate(10..20);
        assert_eq!(&ra.free_ranges, &[10..96]);
    }

    #[test]
    fn deallocate_defragment() {
        let mut ra = RangeAlloc::new(0..100 as u32);

        let _ = ra.allocate(10).unwrap();
        let r1 = ra.allocate(20).unwrap();
        let r2 = ra.allocate(30).unwrap();

        ra.deallocate(r1);
        ra.deallocate(r2);

        assert_eq!(&ra.free_ranges, &[10..100]);
    }

    #[test]
    fn allocate_full_range() {
        let mut ra = RangeAlloc::new(0..100 as u32);
        assert_eq!(ra.allocate(100), Ok(0..100));
        assert!(ra.free_ranges.is_empty());
    }

    #[test]
    #[should_panic]
    fn allocate_zero() {
        let mut ra = RangeAlloc::new(0..100 as u32);
        let _ = ra.allocate(0);
    }

    #[test]
    fn allocate_out_of_bounds() {
        let mut ra = RangeAlloc::new(0..100 as u32);
        assert!(ra.allocate(101).is_err());
    }

    #[test]
    #[should_panic]
    fn deallocate_out_of_bounds() {
        let mut ra = RangeAlloc::new(0..100 as u32);
        ra.deallocate(101..200);
    }

    #[test]
    fn allocate_exhausted() {
        let mut ra = RangeAlloc::new(0..100 as u32);
        assert_eq!(ra.allocate(100), Ok(0..100));
        assert_eq!(ra.allocate(1), Err(RangeAllocError));
    }
}
