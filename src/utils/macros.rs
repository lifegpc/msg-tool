//! Macros Utilities

#[macro_export]
/// A macro likes `try!` but returns `Option<Result<T, E>>`.
macro_rules! try_option {
    ($expr:expr $(,)?) => {
        match $expr {
            std::result::Result::Ok(val) => val,
            std::result::Result::Err(err) => {
                return std::option::Option::Some(std::result::Result::Err(
                    std::convert::From::from(err),
                ));
            }
        }
    };
}
