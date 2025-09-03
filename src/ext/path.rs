//! Extensions for std::path

pub trait PathBufExt {
    /// Remove all extensions from the path.
    fn remove_all_extensions(&mut self);
}

impl PathBufExt for std::path::PathBuf {
    fn remove_all_extensions(&mut self) {
        while self.extension().is_some() {
            self.set_extension("");
        }
    }
}
