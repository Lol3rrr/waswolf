use std::sync::Arc;

use async_trait::async_trait;
use serenity::{
    http::Http,
    model::{
        channel::{Message, Reaction},
        id::GuildId,
    },
};

use crate::storage::Storage;

use super::Chained;

#[derive(Debug, Clone)]
pub enum TransitionError {
    Serenity,
}

#[derive(Debug, Clone)]
pub enum TransitionResult<N> {
    NoTransition,
    NextState(N),
    Error(Arc<TransitionError>),
}

#[derive(Debug)]
pub enum Event {
    AddReaction { reaction: Reaction },
    Reply { message: Message },
}

pub struct Context {
    http: Option<Arc<Http>>,
    event: Option<Event>,
    storage: Option<Storage>,
    guild_id: GuildId,
}

impl Context {
    pub fn new(
        http: Option<Arc<Http>>,
        event: Option<Event>,
        storage: Option<Storage>,
        guild_id: GuildId,
    ) -> Self {
        Self {
            http,
            event,
            storage,
            guild_id,
        }
    }

    pub fn event(&self) -> Option<&Event> {
        self.event.as_ref()
    }
    pub fn http(&self) -> Option<&Http> {
        self.http.as_ref().map(|h| h.as_ref())
    }
    pub fn storage(&self) -> Option<&Storage> {
        self.storage.as_ref()
    }
    pub fn guild_id(&self) -> GuildId {
        self.guild_id
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new(None, None, None, GuildId(0))
    }
}

#[async_trait]
pub trait AsyncTransition<A, N> {
    async fn transition(&mut self, context: Context, arguments: A) -> Arc<TransitionResult<N>>;

    fn chain<Other, Output>(self, other: Other) -> Chained<Self, Other, A, N, Output>
    where
        Self: Sized,
        Other: AsyncTransition<N, Output>,
    {
        Chained::new(self, other)
    }
}
