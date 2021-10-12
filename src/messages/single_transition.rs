use std::{future::Future, marker::PhantomData, sync::Arc};

use async_trait::async_trait;

use super::{AsyncTransition, Context, TransitionResult};

enum CurrentState<F, S> {
    First(F),
    Second(S),
}

pub struct SingleState<S, A, F, O> {
    state: CurrentState<S, Arc<TransitionResult<O>>>,
    _marker: PhantomData<(A, F)>,
}

impl<S, A, F, O> SingleState<S, A, F, O>
where
    S: FnMut(Context, A) -> F,
    F: Future<Output = TransitionResult<O>>,
{
    pub fn new(transition: S) -> Self {
        Self {
            state: CurrentState::First(transition),
            _marker: PhantomData {},
        }
    }
}

#[async_trait]
impl<S, A, F, O> AsyncTransition<A, O> for SingleState<S, A, F, O>
where
    Self: Sized,
    S: FnMut(Context, A) -> F + Send,
    F: Future<Output = TransitionResult<O>> + Send,
    A: Send,
    O: Send + Sync,
{
    async fn transition(&mut self, context: Context, arguments: A) -> Arc<TransitionResult<O>> {
        let func = match &mut self.state {
            CurrentState::First(f) => f,
            CurrentState::Second(output) => return output.clone(),
        };

        let result = Arc::new(func(context, arguments).await);
        match result.as_ref() {
            TransitionResult::NextState(_) => {
                self.state = CurrentState::Second(result.clone());
                result
            }
            TransitionResult::Error(_) => {
                self.state = CurrentState::Second(result.clone());
                result
            }
            TransitionResult::NoTransition => result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn simple_transition() {
        let mut transition: SingleState<_, (), _, i32> =
            SingleState::new(|_, _| async move { TransitionResult::NextState(13) });
        let expected = 13;

        let result = transition.transition(Context::default(), ()).await;

        match result.as_ref() {
            TransitionResult::NoTransition => panic!("Received Transition"),
            TransitionResult::NextState(result) => assert_eq!(expected, *result),
            TransitionResult::Error(e) => panic!("Received Error: {:?}", e),
        };
    }

    #[tokio::test]
    async fn simple_transition_multiple_attempts() {
        let mut transition: SingleState<_, (), _, i32> =
            SingleState::new(|_, _| async move { TransitionResult::NextState(13) });
        let expected = 13;

        let result = transition.transition(Context::default(), ()).await;
        match result.as_ref() {
            TransitionResult::NoTransition => panic!("Received Transition"),
            TransitionResult::NextState(result) => assert_eq!(expected, *result),
            TransitionResult::Error(e) => panic!("Received Error: {:?}", e),
        };

        let result = transition.transition(Context::default(), ()).await;
        match result.as_ref() {
            TransitionResult::NoTransition => panic!("Received Transition"),
            TransitionResult::NextState(result) => assert_eq!(expected, *result),
            TransitionResult::Error(e) => panic!("Received Error: {:?}", e),
        };
    }
}
