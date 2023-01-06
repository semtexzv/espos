use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::ptr;
use core::time::Duration;
use esp_idf_sys::*;

// NOTE: ESP-IDF-specific
const PTHREAD_MUTEX_INITIALIZER: u32 = 0xFFFFFFFF;

pub struct Mutex<T>(UnsafeCell<pthread_mutex_t>, UnsafeCell<T>);

impl<T> Mutex<T> {
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Self(
            UnsafeCell::new(PTHREAD_MUTEX_INITIALIZER as _),
            UnsafeCell::new(data),
        )
    }

    #[inline(always)]
    pub fn lock(&self) -> MutexGuard<'_, T> {
        MutexGuard::new(self)
    }

    #[inline(always)]
    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(self.lock().deref_mut())
    }
}

impl<T> Drop for Mutex<T> {
    fn drop(&mut self) {
        let r = unsafe { pthread_mutex_destroy(self.0.get_mut() as *mut _) };
        debug_assert_eq!(r, 0);
    }
}

unsafe impl<T> Sync for Mutex<T> where T: Send {}
unsafe impl<T> Send for Mutex<T> where T: Send {}

pub struct MutexGuard<'a, T>(&'a Mutex<T>);

impl<'a, T> MutexGuard<'a, T> {
    #[inline(always)]
    fn new(mutex: &'a Mutex<T>) -> Self {
        let r = unsafe { pthread_mutex_lock(mutex.0.get()) };
        debug_assert_eq!(r, 0);

        Self(mutex)
    }
}

unsafe impl<T> Sync for MutexGuard<'_, T> where T: Sync {}

impl<'a, T> Drop for MutexGuard<'a, T> {
    #[inline(always)]
    fn drop(&mut self) {
        let r = unsafe { pthread_mutex_unlock(self.0 .0.get()) };
        debug_assert_eq!(r, 0);
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { self.0 .1.get().as_mut().unwrap() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.0 .1.get().as_mut().unwrap() }
    }
}

pub struct Condvar(UnsafeCell<pthread_cond_t>);

impl Condvar {
    pub fn new() -> Self {
        let mut cond: pthread_cond_t = Default::default();

        let r = unsafe { pthread_cond_init(&mut cond as *mut _, ptr::null()) };
        debug_assert_eq!(r, 0);

        Self(UnsafeCell::new(cond))
    }

    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
        let r = unsafe { pthread_cond_wait(self.0.get(), guard.0 .0.get()) };
        debug_assert_eq!(r, 0);

        guard
    }

    pub fn wait_timeout<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        duration: Duration,
    ) -> (MutexGuard<'a, T>, bool) {
        let mut now: timeval = unsafe { core::mem::zeroed() };
        unsafe { gettimeofday(&mut now, core::ptr::null_mut()) };

        let abstime = timespec {
            tv_sec: now.tv_sec + duration.as_secs() as i32,
            tv_nsec: (now.tv_usec * 1000) + duration.subsec_nanos() as i32,
        };

        let r =
            unsafe { pthread_cond_timedwait(self.0.get(), guard.0 .0.get(), &abstime as *const _) };
        debug_assert!(r == ETIMEDOUT as i32 || r == 0);

        (guard, r == ETIMEDOUT as i32)
    }

    pub fn notify_one(&self) {
        let r = unsafe { pthread_cond_signal(self.0.get()) };
        debug_assert_eq!(r, 0);
    }

    pub fn notify_all(&self) {
        let r = unsafe { pthread_cond_broadcast(self.0.get()) };
        debug_assert_eq!(r, 0);
    }
}

unsafe impl Sync for Condvar {}
unsafe impl Send for Condvar {}

impl Default for Condvar {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Condvar {
    fn drop(&mut self) {
        let r = unsafe { pthread_cond_destroy(self.0.get()) };
        debug_assert_eq!(r, 0);
    }
}
