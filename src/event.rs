use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ptr::null_mut;
use std::sync::atomic::AtomicPtr;

use esp_idf_sys::*;

pub const EVENT_SUICIDE: i32 = i32::MAX;

pub struct Registration<T> {
    base: esp_event_base_t,
    event: i32,
    inst: esp_event_handler_instance_t,
    drop: *mut (dyn FnMut(esp_event_base_t, i32, *mut T) + 'static),
    _p: PhantomData<T>,
}

unsafe extern "C" fn trampoline<T, F>(
    arg: *mut c_void,
    base: esp_event_base_t,
    event: i32,
    data: *mut c_void,
) where
    F: FnMut(esp_event_base_t, i32, *mut T) + 'static,
{
    let mut fun = ManuallyDrop::new(Box::from_raw(arg as *mut F));
    (fun)(base, event, data as *mut T)
}

impl<T> Registration<T> {
    pub unsafe fn unregister(self) {
        // Magic of dropping happens here
    }
}

pub fn post<T>(base: esp_event_base_t, event: i32, obj: T) -> Result<(), EspError> {
    let mut t = ManuallyDrop::new(obj);
    unsafe {
        esp!(esp_event_post(
            base,
            event,
            &mut t as *mut ManuallyDrop<T> as *mut _,
            std::mem::size_of::<T>() as _,
            u32::MAX
        ))
    }
}

pub fn register<T, F>(base: esp_event_base_t, event: i32, mut f: F) -> Registration<T>
where
    F: FnMut(esp_event_base_t, i32, *mut T) + 'static,
{
    let mut fun = Box::new(f);
    let mut fun = Box::into_raw(fun);
    let mut inst = null_mut();
    unsafe {
        esp!(esp_event_handler_instance_register(
            base,
            event,
            Some(trampoline::<T, F>),
            fun as *mut _,
            &mut inst,
        ))
        .unwrap();
        Registration {
            base,
            event,
            inst,
            drop: fun,
            _p: PhantomData,
        }
    }
}

unsafe impl<T> Send for Registration<T> {}

impl<T> Drop for Registration<T> {
    fn drop(&mut self) {
        unsafe {
            // Event handler itself will drop the callback
            esp!(esp_event_handler_instance_unregister(
                self.base, self.event, self.inst
            ))
            .unwrap();
            if !self.drop.is_null() {
                // Drop here
                let _ = Box::from_raw(self.drop);
            }
        }
    }
}
