#![cfg_attr(any(docsrs, feature = "unstable"), feature(doc_cfg))]
#[cfg(feature = "kirikiri-arc")]
pub mod kr_arc;
#[cfg(feature = "simple-pack")]
mod simple_pack;
