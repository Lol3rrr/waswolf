use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

use serenity::{
    http::Http,
    model::{
        channel::{Message, Reaction},
        id::GuildId,
    },
};

use crate::storage::Storage;

#[derive(Clone)]
pub enum TransitionError {
    Serenity,
    Generic(Arc<dyn Display + Send + Sync + 'static>),
    WithReason { reason: String },
}

impl Debug for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serenity => write!(f, "TransitionError::Serenity"),
            Self::Generic(e) => write!(f, "TransitionError::Generic {{ {} }}", e),
            Self::WithReason { reason } => {
                write!(f, "TransitionError::WithReason {{ reason: {:?} }}", reason)
            }
        }
    }
}

impl Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serenity => write!(f, "Interacting with Discord"),
            Self::Generic(e) => Display::fmt(e, f),
            Self::WithReason { reason } => write!(f, "{}", reason),
        }
    }
}

impl TransitionError {
    pub fn arced(self) -> Arc<Self> {
        Arc::new(self)
    }
}

#[derive(Debug)]
pub enum Event {
    Notify,
    AddReaction { reaction: Reaction },
    RemoveReaction { reaction: Reaction },
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
