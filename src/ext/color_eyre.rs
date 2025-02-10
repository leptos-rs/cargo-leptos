use core::convert::Infallible;
use std::fmt::Display;
use std::panic::Location;

pub(crate) mod reexports {
    //! re-exports

    pub use super::CustomWrapErr as _;
    pub use color_eyre::eyre::{bail, ensure, eyre};
    pub use color_eyre::Report as Error;
    pub use color_eyre::Result;
}
use reexports::*;

pub trait CustomWrapErr<T, E> {
    fn context<C>(self, context: C) -> Result<T>
    where
        C: Display + Send + Sync + 'static;

    fn with_context<C, F>(self, context: F) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C;

    /// like google map red dot, only record the location info without any context message.
    fn dot(self) -> Result<T>;
}

impl<T, E> CustomWrapErr<T, E> for Result<T, E>
where
    E: Display,
    Result<T, E>: color_eyre::eyre::WrapErr<T, E>,
{
    #[inline]
    #[track_caller]
    fn context<C>(self, context: C) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
    {
        let caller = Location::caller();
        color_eyre::Context::context(
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
    fn with_context<C, F>(self, context: F) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        let caller = Location::caller();
        color_eyre::Context::with_context(self, || {
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
        color_eyre::Context::context(
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
    Option<T>: color_eyre::eyre::WrapErr<T, Infallible>,
{
    #[inline]
    #[track_caller]
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
    {
        let caller = Location::caller();
        color_eyre::Context::context(
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
    fn with_context<C, F>(self, context: F) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        let caller = Location::caller();
        color_eyre::Context::with_context(self, || {
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
        color_eyre::Context::context(
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
