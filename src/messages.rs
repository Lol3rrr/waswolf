use std::sync::Arc;

use serenity::model::id::{GuildId, MessageId};
pub use statemachines::{AsyncTransition, TransitionResult};

use async_trait::async_trait;

pub type SingleState<S, A, F, O> = statemachines::Next<A, O, Context, Arc<TransitionError>, S, F>;
pub type Chained<F, S, I, M, O> =
    statemachines::Chained<F, S, I, M, O, Arc<TransitionError>, Context>;
pub type WithState<S, F, I, O, STATE> =
    statemachines::WithState<I, O, STATE, Context, Arc<TransitionError>, S, F>;
pub type WithLazyState<S, F, I, O, STATE, INIT> =
    statemachines::WithLazyState<I, O, STATE, Context, Arc<TransitionError>, S, F, INIT>;

mod traits;
pub use traits::{Context, Event, TransitionError};

pub struct MessageStateMachine<I, O> {
    guild_id: GuildId,
    message_id: MessageId,
    sm: Box<dyn AsyncTransition<I, Context, O, Arc<TransitionError>> + Send>,
}

impl<I, O> MessageStateMachine<I, O> {
    pub fn new<S>(guild_id: GuildId, message_id: MessageId, sm: S) -> Self
    where
        S: 'static + AsyncTransition<I, Context, O, Arc<TransitionError>> + Send,
    {
        Self {
            guild_id,
            message_id,
            sm: Box::new(sm),
        }
    }

    pub fn guild_id(&self) -> GuildId {
        self.guild_id
    }
    pub fn message_id(&self) -> MessageId {
        self.message_id
    }
}

#[async_trait]
impl<I, O> AsyncTransition<I, Context, O, Arc<TransitionError>> for MessageStateMachine<I, O>
where
    I: Send,
{
    async fn transition(
        &mut self,
        context: Context,
        arguments: I,
    ) -> std::sync::Arc<TransitionResult<O, Arc<TransitionError>>> {
        self.sm.transition(context, arguments).await
    }
}
