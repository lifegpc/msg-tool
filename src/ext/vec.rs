//!Extensions for vector operations.
/// Extensions for vector operations.
pub trait VecExt<T> {
    /// Copy potentially overlapping sequence of elements from `src` to `dst`.
    fn copy_overlapped(&mut self, src: usize, dst: usize, len: usize);
}

pub trait VecExt2<T> {
    /// Pop the first element of the vector, returning `None` if empty.
    fn pop_first(&mut self) -> Option<T>;
}

impl<T: Copy> VecExt<T> for Vec<T> {
    fn copy_overlapped(&mut self, src: usize, dst: usize, mut len: usize) {
        let mut src = src.min(self.len());
        let mut dst = dst.min(self.len());
        if dst > src {
            while len > 0 {
                let preceding = (dst - src).min(len);
                for i in 0..preceding {
                    self[dst + i] = self[src + i];
                }
                len -= preceding;
                src += preceding;
                dst += preceding;
            }
        } else {
            for i in 0..len {
                self[dst + i] = self[src + i];
            }
        }
    }
}

impl<T> VecExt2<T> for Vec<T> {
    fn pop_first(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            Some(self.remove(0))
        }
    }
}
