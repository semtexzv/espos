use std::marker::PhantomData;
use std::mem;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::time::Duration;

use esp_idf_sys::{
    OSI_QUEUE_SEND_FRONT, vQueueDelete, xQueueGenericCreate, xQueueGenericSend, xQueueGenericSendFromISR,
    xQueueReceive,
};

use crate::task::to_ticks;

pub struct Queue<T> {
    handle: esp_idf_sys::QueueHandle_t,
    _t: PhantomData<T>,
}

impl<T> Queue<T> {
    pub fn new(cap: usize) -> Self {
        assert!(
            std::mem::size_of::<T>() < 80,
            "The item sent in the queue is too big, consider boxing"
        );
        unsafe {
            Self {
                handle: xQueueGenericCreate(cap as _, mem::size_of::<T>() as _, 0),
                _t: PhantomData,
            }
        }
    }

    pub fn send(&self, item: T) {
        let item = ManuallyDrop::new(item);
        assert_eq!(1, unsafe {
            xQueueGenericSend(
                self.handle,
                &item as *const ManuallyDrop<T> as *const _,
                u32::MAX,
                OSI_QUEUE_SEND_FRONT as _,
            )
        });
    }

    #[link_section = ".iram0.text"]
    pub fn send_isr(&self, item: T) {
        let item = ManuallyDrop::new(item);
        let mut i = 0;
        assert_eq!(1, unsafe {
            xQueueGenericSendFromISR(
                self.handle,
                &item as *const ManuallyDrop<T> as *const _,
                &mut i,
                OSI_QUEUE_SEND_FRONT as _,
            )
        });
    }

    pub fn recv(&self) -> T {
        unsafe {
            let mut obj = MaybeUninit::<T>::uninit();
            assert_eq!(
                1,
                xQueueReceive(self.handle, obj.as_mut_ptr() as *mut _, u32::MAX)
            );
            obj.assume_init()
        }
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<T> {
        unsafe {
            let mut obj = MaybeUninit::<T>::uninit();
            if xQueueReceive(self.handle, obj.as_mut_ptr() as *mut _, to_ticks(timeout)) == 1 {
                return Some(obj.assume_init());
            }
            return None;
        }
    }
}

unsafe impl<T> Send for Queue<T> {}

unsafe impl<T> Sync for Queue<T> {}

impl<T> Drop for Queue<T> {
    fn drop(&mut self) {
        println!("Draining queue");
        while let Some(v) = self.recv_timeout(Duration::default()) {}
        println!("Dropping queue");
        unsafe { vQueueDelete(self.handle) }
    }
}
