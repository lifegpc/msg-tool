pub trait VecExt<T> {
    /// Copy potentially overlapping sequence of elements from `src` to `dst`.
    fn copy_overlapped(&mut self, src: usize, dst: usize, len: usize);
}

impl<T: Copy> VecExt<T> for Vec<T> {
    fn copy_overlapped(&mut self, src: usize, dst: usize, len: usize) {
        let src = src.min(self.len());
        let dst = dst.min(self.len());
        if src < dst {
            let max_count = len.min(dst - src);
            for i in 0..max_count {
                self[dst + i] = self[src + i];
            }
        } else {
            let max_count = len.min(src - dst);
            for i in (0..max_count).rev() {
                self[dst + i] = self[src + i];
            }
        }
    }
}
