use crate::{AllocError, AllocResult, Layout};
use buddy_allocator::Heap;
use core::ptr::NonNull;

pub struct BuddyByteAllocator {
    inner: Heap<32>,
}
impl BuddyByteAllocator {
    pub const fn new() -> Self {
        Self {
            inner: Heap::<32>::new(),
        }
    }
    pub fn init(&mut self, start: usize, size: usize) {
        unsafe { self.inner.init(start, size) };
    }

    pub fn alloc_bytes(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        self.inner.alloc(layout).map_err(|_| AllocError::NoMemory)
    }

    pub fn dealloc_bytes(&mut self, pos: NonNull<u8>, layout: Layout) {
        self.inner.dealloc(pos, layout)
    }
    pub fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        unsafe { self.inner.add_to_heap(start, start + size) };
        Ok(())
    }
    pub fn total_bytes(&self) -> usize {
        self.inner.stats_total_bytes()
    }
}
