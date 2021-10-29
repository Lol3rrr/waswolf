use std::{future::Future, marker::PhantomData, sync::Arc};

use async_trait::async_trait;

use crate::{AsyncTransition, TransitionResult};

/// Allows you to have some internal State that can be modified with each Transition attempt
pub struct WithLazyState<ARGUMENT, NEXT, STATE, CONTEXT, ERROR, TRANSITION, FUTURE, INIT> {
    transition_fn: TRANSITION,
    init_fn: INIT,
    state: Option<STATE>,
    done: Option<Arc<TransitionResult<NEXT, ERROR>>>,

    _marker: PhantomData<(ARGUMENT, CONTEXT, FUTURE)>,
}

impl<ARGUMENT, NEXT, STATE, CONTEXT, ERROR, TRANSITION, FUTURE, INIT>
    WithLazyState<ARGUMENT, NEXT, STATE, CONTEXT, ERROR, TRANSITION, FUTURE, INIT>
{
    /// Creates a new Transition
    ///
    /// The State will be created using the `init_fn` at the first Transition Event, this allows
    /// you to initialize the State based on the Data you received for transitioning
    pub fn new(init_fn: INIT, transition_fn: TRANSITION) -> Self {
        Self {
            transition_fn,
            init_fn,
            state: None,
            done: None,

            _marker: PhantomData {},
        }
    }
}

#[async_trait]
impl<ARGUMENT, NEXT, STATE, CONTEXT, ERROR, TRANSITION, FUTURE, INIT>
    AsyncTransition<ARGUMENT, CONTEXT, NEXT, ERROR>
    for WithLazyState<ARGUMENT, NEXT, STATE, CONTEXT, ERROR, TRANSITION, FUTURE, INIT>
where
    Self: Send + Sized,
    ARGUMENT: Send,
    NEXT: Sync + Send,
    STATE: Send,
    CONTEXT: Send,
    ERROR: Sync + Send,
    TRANSITION: FnMut(CONTEXT, STATE, ARGUMENT) -> FUTURE + Send,
    FUTURE: Future<Output = (TransitionResult<NEXT, ERROR>, STATE)> + Send,
    INIT: FnMut(&'_ ARGUMENT) -> STATE,
{
    async fn transition(
        &mut self,
        context: CONTEXT,
        arguments: ARGUMENT,
    ) -> Arc<TransitionResult<NEXT, ERROR>> {
        if self.state.is_none() {
            self.state = Some((self.init_fn)(&arguments));
        }

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
        let mut state_sm = WithLazyState::new(
            |first_val: &usize| *first_val,
            |context: usize, mut inner_state: usize, _: usize| async move {
                inner_state += context;

                if inner_state < 2 {
                    return (TransitionResult::NoTransition, inner_state);
                }

                (
                    TransitionResult::<usize, ()>::Done(inner_state),
                    inner_state,
                )
            },
        );

        let result = state_sm.transition(1, 0).await;
        match result.as_ref() {
            TransitionResult::NoTransition => assert!(true),
            res => panic!("Expected no Transition but got {:?}", res),
        };

        let result = state_sm.transition(1, 0).await;
        match result.as_ref() {
            TransitionResult::Done(value) => assert_eq!(2, *value),
            res => panic!("Expected Done but got {:?}", res),
        };
    }
}
