use esp_idf_sys::{
    configTICK_RATE_HZ, vTaskDelay, vTaskDelete, xTaskCreatePinnedToCore,
    xTaskGetCurrentTaskHandle, TaskHandle_t,
};
use std::ffi::c_void;
use std::ptr::null_mut;
use std::time::Duration;

pub fn to_ticks(d: Duration) -> u32 {
    ((d.as_millis() as u64 * (configTICK_RATE_HZ as u64)) / 1000) as _
}

#[derive(Debug, Clone, Copy)]
pub struct Task {
    handle: TaskHandle_t,
}

impl Task {
    pub fn current() -> Self {
        unsafe {
            Self {
                handle: xTaskGetCurrentTaskHandle(),
            }
        }
    }

    pub fn sleep(&self, d: Duration) {
        unsafe {
            vTaskDelay(to_ticks(d));
        }
    }

    #[inline(never)]
    pub fn spawn<F>(name: &'static str, stack: u32, fun: F) -> Task
    where
        F: FnOnce(Task) + Send + 'static,
    {
        unsafe extern "C" fn run(fun: *mut c_void) {
            let handle = {
                let fun = Box::from_raw(fun as *mut Box<dyn FnOnce(Task)>);
                let handle = xTaskGetCurrentTaskHandle();
                let handle = Task { handle };
                fun(handle);
                handle
            };
            println!("Task ending");
            vTaskDelete(handle.handle);
        }

        let fun = Box::new(fun);
        let fun = Box::new(fun as Box<dyn FnOnce(Task)>);

        let mut handle = null_mut();

        unsafe {
            xTaskCreatePinnedToCore(
                Some(run),
                name.as_ptr() as *const i8,
                stack,
                Box::into_raw(fun) as *mut _,
                1,
                &mut handle,
                0,
            );
        }
        Task { handle }
    }
}
