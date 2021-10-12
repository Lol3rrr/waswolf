use async_trait::async_trait;

mod chained;
mod single_transition;

pub use chained::Chained;
pub use single_transition::SingleState;

mod traits;
pub use traits::*;

pub struct MessageStateMachine<I, O> {
    sm: Box<dyn AsyncTransition<I, O> + Send>,
}

impl<I, O> MessageStateMachine<I, O> {
    pub fn new<S>(sm: S) -> Self
    where
        S: 'static + AsyncTransition<I, O> + Send,
    {
        Self { sm: Box::new(sm) }
    }
}

#[async_trait]
impl<I, O> AsyncTransition<I, O> for MessageStateMachine<I, O>
where
    I: Send,
    O: Send,
{
    async fn transition(
        &mut self,
        context: Context,
        arguments: I,
    ) -> std::sync::Arc<TransitionResult<O>> {
        self.sm.transition(context, arguments).await
    }
}
