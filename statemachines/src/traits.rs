use std::sync::Arc;

use async_trait::async_trait;

use crate::Chained;

/// The Result of an attempted Transition
#[derive(Debug)]
pub enum TransitionResult<N, E> {
    /// No actual Transition occured
    NoTransition,
    /// The current State is done and produced the given Data as a Result
    Done(N),
    /// The attempted Transition caused an Error and should not continue
    Error(E),
}

/// Defines the base Interface for the Statemachine interactions
#[async_trait]
pub trait AsyncTransition<A, C, N, E> {
    /// Attempts to transition from the current State to the Next One, the given
    /// Context and Arguments can be used to provide some additional Data to the
    /// State while transitioning
    async fn transition(&mut self, context: C, arguments: A) -> Arc<TransitionResult<N, E>>;

    /// This is a simple way to chain two Transitions together by simply appending
    /// the other Transition to the current one
    fn chain<T, O>(self, other: T) -> Chained<Self, T, A, N, O, E, C>
    where
        Self: Sized,
        T: AsyncTransition<N, C, O, E>,
    {
        Chained::new(self, other)
    }
}
