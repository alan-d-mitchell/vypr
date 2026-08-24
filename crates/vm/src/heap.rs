use std::alloc::{alloc, dealloc, Layout};
use std::collections::HashMap;
use std::ptr::NonNull;
use crate::value::Object;

const HEAP_SIZE: usize = 64 * 1024 * 1024;

pub struct Heap {
    pub start: *mut u8,
    pub current: *mut u8,
    pub end: *mut u8,
    layout: Layout,

    // head of intrusive linked list tracking all allocated objects
    pub head: Option<NonNull<Object>>,

    // intern pool for strings
    pub strings: HashMap<String, *mut Object>,

    // metrics
    pub capacity: usize,
    pub bytes_allocated: usize,
    pub next_gc_threshold: usize,
}

impl Heap {

    pub fn new() -> Self {
        Self::with_capacity(HEAP_SIZE)
    }

    pub fn with_capacity(size: usize) -> Self {
        let layout: Layout = Layout::from_size_align(size, 8).expect("invalid heap layout");
        let start = unsafe {
            alloc(layout)
        };

        if start.is_null() {
            panic!("failed to allocate VM heap memory");
        }

        let end = unsafe {
            start.add(size)
        };

        Self {
            start,
            current: start,
            end,
            layout,
            head: None,
            strings: HashMap::new(),
            capacity: size,
            bytes_allocated: 0,
            next_gc_threshold: size / 4,
        }
    }

    /// Fast bump allocation for any object type T that begins with an `Object` header.
    pub fn allocate<T>(&mut self, value: T) -> *mut T {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        
        // align current ptr
        let current = self.current as usize;
        let aligned = (current + align - 1) & !(align - 1);
        let next = aligned + size;

        if next > self.end as usize {
            panic!("OUT OF MEMORY");
        }

        let dest = aligned as *mut T;
        unsafe {
            std::ptr::write(dest, value);
            let obj_ptr = dest as *mut Object;

            (*obj_ptr).next = self.head;
            self.head = NonNull::new(obj_ptr);

            self.current = next as *mut u8;
            self.bytes_allocated += size;

            if std::env::var("VYPR_TRACE_GC").is_ok() {
                println!("ALLOC: {:?} ({} bytes) | Total: {}/{}", 
                    (*obj_ptr).ty, size, self.bytes_allocated, self.capacity);
            }
        }

        dest
    }

    /// Reset the bump pointer (used after surviving objects are copied or swept)
    pub unsafe fn reset_nursery(&mut self) {
        self.current = self.start;
        self.bytes_allocated = 0;
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.start, self.layout);
        }
    }
}
