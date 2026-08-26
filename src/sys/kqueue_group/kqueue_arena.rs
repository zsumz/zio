//! Safe phase and bounds bookkeeping for reused native kqueue storage.

use core::mem::MaybeUninit;

pub(in crate::sys::kqueue_group) enum StageError<E> {
    Bounds,
    Encode(E),
    Misreported,
}

/// Proof that every native input entry was staged before submission.
pub(in crate::sys::kqueue_group) struct StagedChanges<'a, T> {
    input: &'a [MaybeUninit<T>],
    output: &'a mut [MaybeUninit<T>],
    receipts: &'a mut usize,
}

impl<T> StagedChanges<'_, T> {
    pub(super) fn input(&self) -> *const T {
        self.input.as_ptr().cast()
    }

    pub(super) fn output(&mut self) -> *mut T {
        self.output.as_mut_ptr().cast()
    }

    pub(super) fn len(&self) -> usize {
        self.input.len()
    }

    pub(super) fn record_receipts(&mut self, receipts: usize) -> Option<()> {
        if receipts > self.len() {
            return None;
        }
        *self.receipts = receipts;
        Some(())
    }
}

/// One arena shared by wait output and disjoint disarm input/output phases.
pub(in crate::sys::kqueue_group) struct KqueueArena<T> {
    storage: Box<[MaybeUninit<T>]>,
    event_capacity: usize,
    change_capacity: usize,
}

impl<T> KqueueArena<T> {
    pub(super) fn new(event_capacity: usize, change_capacity: usize) -> Option<Self> {
        if event_capacity == 0 {
            return None;
        }
        #[cfg(not(target_os = "netbsd"))]
        {
            i32::try_from(event_capacity).ok()?;
            i32::try_from(change_capacity).ok()?;
        }
        let storage_capacity = event_capacity.max(change_capacity.checked_mul(2)?);
        let mut storage = Vec::new();
        storage.try_reserve_exact(storage_capacity).ok()?;
        storage.resize_with(storage_capacity, MaybeUninit::uninit);
        Some(Self {
            storage: storage.into_boxed_slice(),
            event_capacity,
            change_capacity,
        })
    }

    pub(super) const fn event_capacity(&self) -> usize {
        self.event_capacity
    }

    pub(super) fn wait_output(&mut self) -> *mut T {
        self.storage.as_mut_ptr().cast()
    }

    pub(super) fn event_slot(&self, index: usize) -> Option<&MaybeUninit<T>> {
        self.storage.get(index)
    }

    pub(super) fn receipt_slot(&self, index: usize) -> Option<&MaybeUninit<T>> {
        self.storage.get(self.change_capacity.checked_add(index)?)
    }

    pub(in crate::sys::kqueue_group) fn stage<'a, I, F, E>(
        &'a mut self,
        mut items: I,
        mut encode: F,
        observed: &mut usize,
        receipts: &'a mut usize,
    ) -> Result<StagedChanges<'a, T>, StageError<E>>
    where
        I: ExactSizeIterator,
        F: FnMut(I::Item) -> Result<T, E>,
    {
        let count = items.len();
        if count == 0 || count > self.change_capacity {
            return Err(StageError::Bounds);
        }
        *observed = 0;
        *receipts = 0;
        let (input, retained) = self.storage.split_at_mut(self.change_capacity);
        let input = &mut input[..count];
        for destination in input.iter_mut() {
            let item = items.next().ok_or(StageError::Misreported)?;
            destination.write(encode(item).map_err(StageError::Encode)?);
        }
        if items.next().is_some() {
            return Err(StageError::Misreported);
        }
        Ok(StagedChanges {
            input,
            output: &mut retained[..count],
            receipts,
        })
    }

    #[cfg(test)]
    pub(in crate::sys::kqueue_group) fn receipt_slot_mut(
        &mut self,
        index: usize,
    ) -> Option<&mut MaybeUninit<T>> {
        let index = self.change_capacity.checked_add(index)?;
        self.storage.get_mut(index)
    }
}

impl<T> std::fmt::Debug for KqueueArena<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KqueueArena")
            .field("event_capacity", &self.event_capacity)
            .field("change_capacity", &self.change_capacity)
            .field("storage_capacity", &self.storage.len())
            .finish_non_exhaustive()
    }
}
