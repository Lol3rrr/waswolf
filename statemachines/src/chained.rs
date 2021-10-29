use std::{marker::PhantomData, sync::Arc};

use async_trait::async_trait;

use crate::{AsyncTransition, TransitionResult};

enum StateResult<FR, SR> {
    Empty,
    First(FR),
    Second(SR),
}

/// This will chain two States/Transitions together to be run after one another
pub struct Chained<F, S, A, M, N, E, C> {
    first: F,
    second: S,

    result: StateResult<Arc<TransitionResult<M, E>>, Arc<TransitionResult<N, E>>>,

    _marker: PhantomData<(A, C)>,
}

impl<F, S, A, M, N, E, C> Chained<F, S, A, M, N, E, C>
where
    F: AsyncTransition<A, C, M, E>,
    S: AsyncTransition<M, C, N, E>,
{
    /// Creates a new Chain of the Two Transitions
    pub fn new(first: F, second: S) -> Self {
        Self {
            first,
            second,

            result: StateResult::Empty,

            _marker: PhantomData {},
        }
    }
}

#[async_trait]
impl<F, S, A, M, N, E, C> AsyncTransition<A, C, N, E> for Chained<F, S, A, M, N, E, C>
where
    Self: Send,
    F: AsyncTransition<A, C, M, E> + Send,
    S: AsyncTransition<M, C, N, E> + Send,
    A: Send,
    M: Clone + Send + Sync,
    N: Send + Sync,
    E: Clone + Send + Sync,
    C: Send,
{
    async fn transition(
        &mut self,
        context: C,
        arguments: A,
    ) -> std::sync::Arc<TransitionResult<N, E>> {
        match &self.result {
            StateResult::Empty => {
                let result = self.first.transition(context, arguments).await;
                let n_result = match result.as_ref() {
                    TransitionResult::NoTransition => {
                        return Arc::new(TransitionResult::NoTransition)
                    }
                    TransitionResult::Done(_) => TransitionResult::NoTransition,
                    TransitionResult::Error(e) => TransitionResult::Error(e.clone()),
                };

                self.result = StateResult::First(result);

                Arc::new(n_result)
            }
            StateResult::First(first_res) => {
                let intermediate = match first_res.as_ref() {
                    TransitionResult::Done(value) => value.clone(),
                    TransitionResult::Error(e) => {
                        return Arc::new(TransitionResult::Error(e.clone()))
                    }
                    _ => unreachable!(""),
                };

                let result = self.second.transition(context, intermediate).await;

                match result.as_ref() {
                    TransitionResult::NoTransition => return result,
                    _ => {}
                }

                self.result = StateResult::Second(result.clone());

                result
            }
            StateResult::Second(second_res) => second_res.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Next;

    use super::*;

    #[tokio::test]
    async fn simple() {
        let mut chained = Chained::new(
            Next::new(|_: (), number: usize| async move {
                TransitionResult::<usize, ()>::Done(number * 2)
            }),
            Next::new(|_: (), number: usize| async move { TransitionResult::Done(number + 4) }),
        );

        let first_result = chained.transition((), 13).await;
        match first_result.as_ref() {
            TransitionResult::NoTransition => assert!(true),
            res => panic!("Expected no transition but got {:?}", res),
        };

        let second_result = chained.transition((), 13).await;
        match second_result.as_ref() {
            TransitionResult::Done(value) => assert_eq!(30, *value),
            res => panic!("Expected Done-Transition but got {:?}", res),
        };
    }
}
