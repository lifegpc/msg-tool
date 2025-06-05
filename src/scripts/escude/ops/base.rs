use super::super::script::{ReadParam, VM};
use crate::ext::io::*;
use anyhow::Result;

pub trait CustomOps<T: std::fmt::Debug + TryInto<u64>>: std::fmt::Debug {
    fn run<'a>(&mut self, vm: &mut VM<'a, T>, op: u8) -> Result<bool>
    where
        MemReaderRef<'a>: ReadParam<T>,
        T: TryInto<u64>
            + Default
            + Eq
            + Ord
            + Copy
            + std::fmt::Debug
            + std::fmt::Display
            + std::hash::Hash
            + From<u8>
            + std::ops::Neg<Output = T>
            + std::ops::Add<Output = T>
            + std::ops::Sub<Output = T>
            + std::ops::Mul<Output = T>
            + std::ops::Div<Output = T>
            + std::ops::Rem<Output = T>
            + std::ops::Not<Output = T>
            + std::ops::BitAnd<Output = T>
            + std::ops::BitOr<Output = T>
            + std::ops::BitXor<Output = T>
            + std::ops::Shr<Output = T>
            + std::ops::Shl<Output = T>,
        anyhow::Error: From<<T as TryInto<u64>>::Error>;
}
