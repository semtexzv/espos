use rand::{Error, RngCore};

mod actor;
mod event;
mod mutex;
mod queue;
mod task;
mod timer;

pub use actor::*;
pub use event::*;
pub use mutex::*;
pub use queue::*;
pub use task::*;
pub use timer::*;

pub struct EspRand;

impl RngCore for EspRand {
    fn next_u32(&mut self) -> u32 {
        unsafe { esp_idf_sys::esp_random() }
    }

    fn next_u64(&mut self) -> u64 {
        unsafe { (esp_idf_sys::esp_random() as u64) << 32 | esp_idf_sys::esp_random() as u64 }
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        unsafe { esp_idf_sys::esp_fill_random(dest.as_mut_ptr() as _, dest.len() as _) }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        Ok(Self::fill_bytes(self, dest))
    }
}
