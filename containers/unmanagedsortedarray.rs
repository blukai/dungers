use core::cmp::Ordering;
use core::{fmt, mem};

use alloc::{AllocError, Allocator};

use crate::array::InsertError;
use crate::sortedarray::SortedArrayCompare;
use crate::unmanagedarray::UnmanagedArray;

// ----
// unmanaged sorted array map

pub struct UnmanagedSortedArrayMap<K, V>(pub UnmanagedArray<(K, V)>);

impl<K: SortedArrayCompare, V> UnmanagedSortedArrayMap<K, V> {
    // NOTE: all of the functions below were copypasted from SortedArray's impl.
    //   only functions that allocate memory were modified to accept alloc param.

    pub fn try_insert(
        &mut self,
        alloc: impl Allocator,
        key: K,
        value: V,
    ) -> Result<Option<V>, InsertError<(K, V)>> {
        let index = self
            .0
            .partition_point(|(k, _)| k.compare(&key) == Ordering::Less);
        match self.0.get_mut(index) {
            Some((k, existing)) if (k as &K).compare(&key) == Ordering::Equal => {
                Ok(Some(mem::replace(existing, value)))
            }
            _ => self.0.try_insert(alloc, index, (key, value)).map(|_| None),
        }
    }

    // ----
    // extend from

    pub fn try_extend_from_iter<I: Iterator<Item = (K, V)>>(
        &mut self,
        alloc: impl Allocator,
        iter: I,
    ) -> Result<(), AllocError> {
        self.0.try_extend_from_iter(alloc, iter)?;
        self.0.sort_unstable_by(|(a, _), (b, _)| a.compare(b));
        Ok(())
    }

    // ----
    // array deviations

    pub fn contains<Q: ?Sized>(&self, key: &Q) -> bool
    where
        K: SortedArrayCompare<Q>,
    {
        self.0.binary_search_by(|(k, _)| k.compare(key)).is_ok()
    }

    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: SortedArrayCompare<Q>,
    {
        self.0
            .binary_search_by(|(k, _)| k.compare(key))
            .ok()
            .map(|found| unsafe { &self.0.get_unchecked(found).1 })
    }

    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: SortedArrayCompare<Q>,
    {
        self.0
            .binary_search_by(|(k, _)| k.compare(key))
            .ok()
            .map(|found| unsafe { &mut self.0.get_unchecked_mut(found).1 })
    }

    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: SortedArrayCompare<Q>,
    {
        self.0
            .binary_search_by(|(k, _)| k.compare(key))
            .ok()
            .and_then(|found| self.0.remove_ordered(found))
    }
}

impl<K, V> Default for UnmanagedSortedArrayMap<K, V> {
    #[inline]
    fn default() -> Self {
        Self(UnmanagedArray::default())
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for UnmanagedSortedArrayMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0.as_slice(), f)
    }
}

// ----
// unmanaged sorted array set

pub struct UnmanagedSortedArraySet<T>(pub UnmanagedArray<T>);

impl<T: SortedArrayCompare> UnmanagedSortedArraySet<T> {
    // TODO: return true if the set did not previously contain the value.
    pub fn try_insert(&mut self, alloc: impl Allocator, value: T) -> Result<(), InsertError<T>> {
        let index = self
            .0
            .partition_point(|v| v.compare(&value) == Ordering::Less);
        match self.0.get(index) {
            Some(v) if v.compare(&value) == Ordering::Equal => Ok(()),
            _ => self.0.try_insert(alloc, index, value),
        }
    }

    // ----
    // extend from

    pub fn try_extend_from_iter<I: Iterator<Item = T>>(
        &mut self,
        alloc: impl Allocator,
        iter: I,
    ) -> Result<(), AllocError> {
        self.0.try_extend_from_iter(alloc, iter)?;
        self.0.sort_unstable_by(|a, b| a.compare(b));
        Ok(())
    }

    // ----
    // array deviations

    pub fn contains<Q: ?Sized>(&self, value: &Q) -> bool
    where
        T: SortedArrayCompare<Q>,
    {
        self.0.binary_search_by(|v| v.compare(value)).is_ok()
    }

    pub fn remove<Q: ?Sized>(&mut self, value: &Q) -> Option<T>
    where
        T: SortedArrayCompare<Q>,
    {
        self.0
            .binary_search_by(|v| v.compare(value))
            .ok()
            .and_then(|found| self.0.remove_ordered(found))
    }
}

impl<T> Default for UnmanagedSortedArraySet<T> {
    #[inline]
    fn default() -> Self {
        Self(UnmanagedArray::default())
    }
}

impl<T: fmt::Debug> fmt::Debug for UnmanagedSortedArraySet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0.as_slice(), f)
    }
}

// ----

#[cfg(not(no_global_oom_handling))]
mod oom {
    use alloc::this_is_fine;

    use super::*;

    impl<K: SortedArrayCompare, V> UnmanagedSortedArrayMap<K, V> {
        #[track_caller]
        #[inline]
        pub fn insert(&mut self, alloc: impl Allocator, key: K, value: V) -> Option<V> {
            match self.try_insert(alloc, key, value) {
                Ok(maybe_existing) => maybe_existing,
                Err(err) => err.panic(),
            }
        }

        // ----
        // extend from

        #[track_caller]
        #[inline]
        pub fn extend_from_iter<I: Iterator<Item = (K, V)>>(
            &mut self,
            alloc: impl Allocator,
            iter: I,
        ) {
            this_is_fine(self.try_extend_from_iter(alloc, iter))
        }
    }

    impl<T: SortedArrayCompare> UnmanagedSortedArraySet<T> {
        #[track_caller]
        #[inline]
        pub fn insert(&mut self, alloc: impl Allocator, value: T) {
            match self.try_insert(alloc, value) {
                Ok(..) => {}
                Err(err) => err.panic(),
            }
        }

        // ----
        // extend from

        #[track_caller]
        #[inline]
        pub fn extend_from_iter<I: Iterator<Item = T>>(&mut self, alloc: impl Allocator, iter: I) {
            this_is_fine(self.try_extend_from_iter(alloc, iter))
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::Global;

    use super::*;

    #[test]
    fn test_unmanaged_sorted_array_map() {
        let mut this = UnmanagedSortedArrayMap::default();

        this.try_insert(Global, 42, 0).unwrap();
        this.try_insert(Global, 64, 0).unwrap();
        this.try_insert(Global, 27, 0).unwrap();
        this.try_insert(Global, 27, 1).unwrap();
        assert_eq!(this.0.as_slice(), &[(27, 1), (42, 0), (64, 0)]);

        assert!(this.contains(&42));

        assert_eq!(this.get(&27), Some(&1));

        this.remove(&27);
        assert_eq!(this.0.as_slice(), &[(42, 0), (64, 0)]);
    }

    #[test]
    fn test_unmanaged_sorted_array_set() {
        let mut this = UnmanagedSortedArraySet::default();

        this.try_insert(Global, 42).unwrap();
        this.try_insert(Global, 64).unwrap();
        this.try_insert(Global, 27).unwrap();
        this.try_insert(Global, 27).unwrap();
        assert_eq!(this.0.as_slice(), &[27, 42, 64]);

        assert!(this.contains(&42));

        this.remove(&27);
        assert_eq!(this.0.as_slice(), &[42, 64]);
    }
}
