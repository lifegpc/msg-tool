pub trait VecExt<T> {
    /// Copy potentially overlapping sequence of elements from `src` to `dst`.
    fn copy_overlapped(&mut self, src: usize, dst: usize, len: usize);
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

pub trait SliceExt<T> {
    fn rfind(&self, pattern: &[T]) -> Option<usize>;
}

impl<T: PartialEq> SliceExt<T> for [T] {
    fn rfind(&self, pattern: &[T]) -> Option<usize> {
        if pattern.is_empty() || self.len() < pattern.len() {
            return None;
        }
        for i in (0..=self.len() - pattern.len()).rev() {
            if &self[i..i + pattern.len()] == pattern {
                return Some(i);
            }
        }
        None
    }
}
