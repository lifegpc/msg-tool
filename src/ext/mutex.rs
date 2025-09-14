//! Extension for [std::sync::Mutex].
pub trait MutexExt<T> {
    /// Lock the mutex, blocking the current thread until it can be acquired.
    fn lock_blocking(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for std::sync::Mutex<T> {
    fn lock_blocking(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|err| err.into_inner())
    }
}
