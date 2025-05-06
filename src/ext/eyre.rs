use core::convert::Infallible;
use std::{fmt::Display, panic::Location};

pub(crate) mod reexports {
    //! re-exports

    pub use super::{AnyhowCompatWrapErr as _, CustomWrapErr as _};
    pub use eyre::{bail, ensure, eyre, Report as Error, Result};
}
use reexports::*;

pub trait CustomWrapErr<T, E> {
    fn wrap_err<C>(self, context: C) -> Result<T>
    where
        C: Display + Send + Sync + 'static;

    fn wrap_err_with<C, F>(self, context: F) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C;

    /// like google map red dot, only record the location info without any context message.
    fn dot(self) -> Result<T>;
}

/// For some reason, `anyhow::Error` doesn't impl `std::error::Error`??!
/// Why! Your an error handling library! Anyhow, this increases ergonomic
/// to work around this limitation
pub trait AnyhowCompatWrapErr<T> {
    fn wrap_anyhow_err<C>(self, context: C) -> Result<T>
    where
        C: Display + Send + Sync + 'static;

    fn wrap_anyhow_err_with<C, F>(self, context: F) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C;

    /// like google map red dot, only record the location info without any context message.
    fn dot_anyhow(self) -> Result<T>;
}

/// https://github.com/dtolnay/anyhow/issues/356#issuecomment-2053956844
struct AnyhowNewType(anyhow::Error);

impl std::fmt::Debug for AnyhowNewType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", &self.0)
    }
}
impl std::fmt::Display for AnyhowNewType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        anyhow::Error::fmt(&self.0, f)
    }
}
impl core::error::Error for AnyhowNewType {}

impl<T> AnyhowCompatWrapErr<T> for anyhow::Result<T> {
    #[inline]
    #[track_caller]
    fn wrap_anyhow_err<C>(self, context: C) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
    {
        let caller = Location::caller();
        eyre::WrapErr::wrap_err(
            self.map_err(AnyhowNewType),
            format!(
                "{} at `{}:{}:{}`",
                context,
                caller.file(),
                caller.line(),
                caller.column()
            ),
        )
    }

    #[inline]
    #[track_caller]
    fn wrap_anyhow_err_with<C, F>(self, context: F) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        let caller = Location::caller();
        eyre::WrapErr::wrap_err_with(self.map_err(AnyhowNewType), || {
            format!(
                "{} at `{}:{}:{}`",
                context(),
                caller.file(),
                caller.line(),
                caller.column(),
            )
        })
    }

    #[inline]
    #[track_caller]
    fn dot_anyhow(self) -> Result<T> {
        let caller = Location::caller();
        eyre::WrapErr::wrap_err(
            self.map_err(AnyhowNewType),
            format!(
                "at `{}:{}:{}`",
                caller.file(),
                caller.line(),
                caller.column()
            ),
        )
    }
}

impl<T, E> CustomWrapErr<T, E> for Result<T, E>
where
    E: Display,
    Result<T, E>: eyre::WrapErr<T, E>,
{
    #[inline]
    #[track_caller]
    fn wrap_err<C>(self, context: C) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
    {
        let caller = Location::caller();
        eyre::WrapErr::wrap_err(
            self,
            format!(
                "{} at `{}:{}:{}`",
                context,
                caller.file(),
                caller.line(),
                caller.column()
            ),
        )
    }

    #[inline]
    #[track_caller]
    fn wrap_err_with<C, F>(self, context: F) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        let caller = Location::caller();
        eyre::WrapErr::wrap_err_with(self, || {
            format!(
                "{} at `{}:{}:{}`",
                context(),
                caller.file(),
                caller.line(),
                caller.column(),
            )
        })
    }

    #[inline]
    #[track_caller]
    fn dot(self) -> Result<T> {
        let caller = Location::caller();
        eyre::WrapErr::wrap_err(
            self,
            format!(
                "at `{}:{}:{}`",
                caller.file(),
                caller.line(),
                caller.column()
            ),
        )
    }
}

impl<T> CustomWrapErr<T, Infallible> for Option<T>
where
    Option<T>: eyre::WrapErr<T, Infallible>,
{
    #[inline]
    #[track_caller]
    fn wrap_err<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
    {
        let caller = Location::caller();
        eyre::WrapErr::wrap_err(
            self,
            format!(
                "{} at `{}:{}:{}`",
                context,
                caller.file(),
                caller.line(),
                caller.column()
            ),
        )
    }

    #[inline]
    #[track_caller]
    fn wrap_err_with<C, F>(self, context: F) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        let caller = Location::caller();
        eyre::WrapErr::wrap_err_with(self, || {
            format!(
                "{} at `{}:{}:{}`",
                context(),
                caller.file(),
                caller.line(),
                caller.column(),
            )
        })
    }

    #[inline]
    #[track_caller]
    fn dot(self) -> Result<T> {
        let caller = Location::caller();
        eyre::WrapErr::wrap_err(
            self,
            format!(
                "at `{}:{}:{}`",
                caller.file(),
                caller.line(),
                caller.column()
            ),
        )
    }
}
