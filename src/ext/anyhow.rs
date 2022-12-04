use core::convert::Infallible;
use std::fmt::Display;
use std::panic::Location;

/// re-exports
pub use anyhow::{anyhow, bail, ensure};
pub use anyhow::{Chain, Error, Ok, Result};

pub trait Context<T, E> {
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

impl<T, E> Context<T, E> for Result<T, E>
where
    E: Display,
    Result<T, E>: anyhow::Context<T, E>,
{
    #[inline]
    #[track_caller]
    fn context<C>(self, context: C) -> Result<T>
    where
        C: Display + Send + Sync + 'static,
    {
        let caller = Location::caller();
        anyhow::Context::context(
            self,
            format!(
                "{} at `{}@{}:{}`",
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
        anyhow::Context::with_context(self, || {
            format!(
                "{} at `{}@{}:{}`",
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
        anyhow::Context::context(
            self,
            format!(
                "at `{}@{}:{}`",
                caller.file(),
                caller.line(),
                caller.column()
            ),
        )
    }
}

impl<T> Context<T, Infallible> for Option<T>
where
    Option<T>: anyhow::Context<T, Infallible>,
{
    #[inline]
    #[track_caller]
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Send + Sync + 'static,
    {
        let caller = Location::caller();
        anyhow::Context::context(
            self,
            format!(
                "{} at `{}@{}:{}`",
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
        anyhow::Context::with_context(self, || {
            format!(
                "{} at `{}@{}:{}`",
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
        anyhow::Context::context(
            self,
            format!(
                "at `{}@{}:{}`",
                caller.file(),
                caller.line(),
                caller.column()
            ),
        )
    }
}
