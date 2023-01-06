use core::mem::zeroed;
use core::time::Duration;
use std::mem::ManuallyDrop;
use std::ptr::null_mut;

use esp_idf_sys::c_types::c_void;
use esp_idf_sys::*;

pub struct Periodic {
    timer: esp_timer_handle_t,
    drop: *mut (dyn FnMut() + 'static),
}

unsafe extern "C" fn periodic_trampoline<F: FnMut() + 'static>(arg: *mut c_void) {
    if arg.is_null() {
        return;
    }
    let mut fun = ManuallyDrop::new(Box::<F>::from_raw(arg as *mut _));
    (fun)();
}

pub unsafe fn periodic<F>(duration: Duration, f: F) -> Result<Periodic, EspError>
where
    F: FnMut() + 'static,
{
    let mut timer: esp_timer_handle_t = null_mut();

    let mut closure: Box<F> = Box::new(f);
    let mut ptr: *mut F = Box::into_raw(closure);

    // We use reified pointer here (not fat ptr)
    esp!(esp_timer_create(
        &esp_timer_create_args_t {
            callback: Some(periodic_trampoline::<F>),
            arg: ptr as *mut c_void,
            ..zeroed()
        },
        &mut timer
    ))?;

    esp!(esp_timer_start_periodic(timer, duration.as_micros() as _))?;

    // We have a vtable in the timer, we can use a single pointer here,
    // We can only deallocate the box by dropping the Periodic struct
    Ok(Periodic { timer, drop: ptr })
}

impl Drop for Periodic {
    fn drop(&mut self) {
        unsafe {
            esp!(esp_timer_stop(self.timer)).unwrap();
            esp!(esp_timer_delete(self.timer)).unwrap();
            if !self.drop.is_null() {
                let _ = Box::from_raw(self.drop);
            }
        }
    }
}
unsafe impl Send for Periodic {}

pub struct Once {
    timer: esp_timer_handle_t,
    drop: *mut Option<Box<dyn FnOnce() + 'static>>,
}

unsafe extern "C" fn once_trampoline<F: FnOnce() + 'static>(arg: *mut c_void) {
    if arg.is_null() {
        return;
    }
    let mut fun = ManuallyDrop::new(Box::<Option<Box<F>>>::from_raw(arg as *mut _));

    if let Some(fun) = fun.take() {
        (fun)();
    }
}

pub unsafe fn once<F: FnMut() + 'static>(timeout: Duration, f: F) -> Result<Once, EspError> {
    let mut timer: esp_timer_handle_t = null_mut();

    // Need double ptr, we can consume the inner struct from the callback, but must deallocate the outer one from dropping the Once struct
    let mut closure: Box<F> = Box::new(f);
    let mut double: Box<Option<Box<_>>> = Box::new(Some(closure as Box<dyn FnOnce() + 'static>));

    let mut ptr = Box::into_raw(double);

    esp!(esp_timer_create(
        &esp_timer_create_args_t {
            callback: Some(once_trampoline::<F>),
            arg: ptr as *mut c_void,
            ..zeroed()
        },
        &mut timer
    ))?;

    esp!(esp_timer_start_once(timer, timeout.as_micros() as _))?;

    // We have a vtable in the timer
    Ok(Once { timer, drop: ptr })
}

impl Drop for Once {
    fn drop(&mut self) {
        unsafe {
            // Can fail if the timer is not running
            esp_timer_stop(self.timer);
            esp!(esp_timer_delete(self.timer)).unwrap();
            if !self.drop.is_null() {
                let _ = Box::from_raw(self.drop);
            }
        }
    }
}
unsafe impl Send for Once {}
