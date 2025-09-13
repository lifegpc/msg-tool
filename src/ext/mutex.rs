//! Extension for [std::sync::Mutex].
pub trait MutexExt<T> {
    /// Lock the mutex, blocking the current thread until it can be acquired.
    fn lock_blocking(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for std::sync::Mutex<T> {
    fn lock_blocking(&self) -> std::sync::MutexGuard<'_, T> {
        loop {
            match self.try_lock() {
                Ok(guard) => return guard,
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::yield_now();
                }
                Err(std::sync::TryLockError::Poisoned(err)) => return err.into_inner(),
            }
        }
    }
}
