use std::sync::Arc;

pub use statemachines::{AsyncTransition, TransitionResult};

use async_trait::async_trait;

pub type SingleState<S, A, F, O> = statemachines::Next<A, O, Context, Arc<TransitionError>, S, F>;
pub type Chained<F, S, I, M, O> =
    statemachines::Chained<F, S, I, M, O, Arc<TransitionError>, Context>;

mod traits;
pub use traits::{Context, Event, TransitionError};

pub struct MessageStateMachine<I, O> {
    sm: Box<dyn AsyncTransition<I, Context, O, Arc<TransitionError>> + Send>,
}

impl<I, O> MessageStateMachine<I, O> {
    pub fn new<S>(sm: S) -> Self
    where
        S: 'static + AsyncTransition<I, Context, O, Arc<TransitionError>> + Send,
    {
        Self { sm: Box::new(sm) }
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
