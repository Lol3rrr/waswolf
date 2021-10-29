use std::{future::Future, marker::PhantomData, sync::Arc};

use async_trait::async_trait;

use crate::{AsyncTransition, TransitionResult};

/// Allows you to have some internal State that can be modified with each Transition attempt
pub struct WithState<ARGUMENT, NEXT, STATE, CONTEXT, ERROR, TRANSITION, FUTURE> {
    transition_fn: TRANSITION,
    state: Option<STATE>,
    done: Option<Arc<TransitionResult<NEXT, ERROR>>>,

    _marker: PhantomData<(ARGUMENT, CONTEXT, FUTURE)>,
}

impl<ARGUMENT, NEXT, STATE, CONTEXT, ERROR, TRANSITION, FUTURE>
    WithState<ARGUMENT, NEXT, STATE, CONTEXT, ERROR, TRANSITION, FUTURE>
{
    /// Creates a new Transition with the given inital_state
    pub fn new(inital_state: STATE, transition_fn: TRANSITION) -> Self {
        Self {
            transition_fn,
            state: Some(inital_state),
            done: None,

            _marker: PhantomData {},
        }
    }
}

#[async_trait]
impl<ARGUMENT, NEXT, STATE, CONTEXT, ERROR, TRANSITION, FUTURE>
    AsyncTransition<ARGUMENT, CONTEXT, NEXT, ERROR>
    for WithState<ARGUMENT, NEXT, STATE, CONTEXT, ERROR, TRANSITION, FUTURE>
where
    Self: Send + Sized,
    ARGUMENT: Send,
    NEXT: Sync + Send,
    STATE: Send,
    CONTEXT: Send,
    ERROR: Sync + Send,
    TRANSITION: FnMut(CONTEXT, STATE, ARGUMENT) -> FUTURE + Send,
    FUTURE: Future<Output = (TransitionResult<NEXT, ERROR>, STATE)> + Send,
{
    async fn transition(
        &mut self,
        context: CONTEXT,
        arguments: ARGUMENT,
    ) -> Arc<TransitionResult<NEXT, ERROR>> {
        if let Some(prev_result) = self.done.as_ref() {
            return prev_result.clone();
        }

        let inner_state = self.state.take().unwrap();
        let (result, new_state) = (self.transition_fn)(context, inner_state, arguments).await;

        self.state = Some(new_state);

        match result {
            TransitionResult::Done(val) => {
                let arced = Arc::new(TransitionResult::Done(val));
                self.done = Some(arced.clone());
                arced
            }
            TransitionResult::Error(e) => {
                let arced = Arc::new(TransitionResult::Error(e));
                self.done = Some(arced.clone());
                arced
            }
            TransitionResult::NoTransition => Arc::new(TransitionResult::NoTransition),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn counter() {
        let mut state_sm = WithState::new(
            0,
            |_: (), mut inner_state: usize, argument: usize| async move {
                inner_state += argument;

                if inner_state < 2 {
                    return (TransitionResult::NoTransition, inner_state);
                }

                (
                    TransitionResult::<usize, ()>::Done(inner_state),
                    inner_state,
                )
            },
        );

        let result = state_sm.transition((), 1).await;
        match result.as_ref() {
            TransitionResult::NoTransition => assert!(true),
            res => panic!("Expected no Transition but got {:?}", res),
        };

        let result = state_sm.transition((), 1).await;
        match result.as_ref() {
            TransitionResult::Done(value) => assert_eq!(2, *value),
            res => panic!("Expected Done but got {:?}", res),
        };
    }
}
