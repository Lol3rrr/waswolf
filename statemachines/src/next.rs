use std::{future::Future, marker::PhantomData, sync::Arc};

use async_trait::async_trait;

use crate::{AsyncTransition, TransitionResult};

/// Represents a single State, that can be done or not
pub struct Next<A, N, C, E, T, F> {
    transition_fn: T,
    done: Option<Arc<TransitionResult<N, E>>>,

    _marker: PhantomData<(A, C, F)>,
}

impl<A, N, C, E, T, F> Next<A, N, C, E, T, F>
where
    T: FnMut(C, A) -> F,
    F: Future<Output = TransitionResult<N, E>>,
{
    /// Creates a new State with the given Configuration
    pub fn new(transition_fn: T) -> Self {
        Self {
            transition_fn,
            done: None,

            _marker: PhantomData {},
        }
    }
}

#[async_trait]
impl<A, N, C, E, T, F> AsyncTransition<A, C, N, E> for Next<A, N, C, E, T, F>
where
    Self: Send + Sized,
    A: Send,
    N: Sync + Send,
    C: Send,
    E: Sync + Send,
    T: FnMut(C, A) -> F + Send,
    F: Future<Output = TransitionResult<N, E>> + Send,
{
    async fn transition(&mut self, context: C, arguments: A) -> Arc<TransitionResult<N, E>> {
        if let Some(prev_result) = self.done.as_ref() {
            return prev_result.clone();
        }

        let result = (self.transition_fn)(context, arguments).await;

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
    async fn simple_transition() {
        let mut next_sm =
            Next::new(|_: (), _: ()| async move { TransitionResult::<usize, ()>::Done(13) });

        let result = next_sm.transition((), ()).await;
        match result.as_ref() {
            TransitionResult::Done(value) => assert_eq!(13, *value),
            res => panic!("Expected Transition to complete but got {:?}", res),
        };
    }
}
