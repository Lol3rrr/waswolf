use std::{
    error::Error,
    fmt::{Debug, Display},
};

use async_trait::async_trait;
use serenity::{
    client::Context,
    http::Http,
    model::id::UserId,
    prelude::{RwLock, TypeMap},
};

#[async_trait]
pub trait BotContext: Send + Sync {
    fn get_http(&self) -> &Http;

    fn get_data(&self) -> &RwLock<TypeMap>;
}

/// This provides all the Context needed for performing a Transition from one State to the next one
#[derive(Clone, Copy)]
pub struct TransitionContext<'a> {
    /// The UserID of the Bot itself
    pub bot_id: UserId,
    /// The Discord-Serenity Context
    pub ctx: &'a dyn BotContext,
}

/// This Trait defines an interface for transitioning between different States
#[async_trait]
pub trait Transition<S> {
    /// A simple unfailable transition from origin to Self
    async fn transition<'a>(origin: S, context: TransitionContext<'a>) -> Self;
}

/// The Error type returned by a failed attempt to transition from one State to another
pub struct TransitionError(pub Box<dyn Error + Send + Sync>);

impl TransitionError {
    /// Create a new TransitionError from the given Error
    pub fn new<E>(err: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self(Box::new(err))
    }
}

impl Debug for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}
impl Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl Error for TransitionError {}

/// This Trait defines an interface that allow for failable transitions between two States
#[async_trait]
pub trait TryTransition<S>
where
    Self: Sized,
{
    /// Attempts to transition to this State from another one, which is allowed to fail
    async fn try_transition<'a>(
        source: S,
        context: TransitionContext<'a>,
    ) -> Result<Self, TransitionError>;
}

#[async_trait]
impl BotContext for Context {
    fn get_http(&self) -> &Http {
        &self.http
    }

    fn get_data(&self) -> &RwLock<TypeMap> {
        &self.data
    }
}
