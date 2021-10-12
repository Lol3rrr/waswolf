use std::{marker::PhantomData, sync::Arc};

use async_trait::async_trait;

use super::{AsyncTransition, Context, TransitionResult};

pub struct Chained<F, S, I, M, O> {
    first: F,
    first_result: Option<Arc<TransitionResult<M>>>,
    second: S,
    _marker: PhantomData<(I, M, O)>,
}

impl<F, S, I, M, O> Chained<F, S, I, M, O>
where
    F: AsyncTransition<I, M>,
    S: AsyncTransition<M, O>,
{
    pub fn new(first: F, second: S) -> Self {
        Self {
            first,
            first_result: None,
            second,
            _marker: PhantomData {},
        }
    }
}

#[async_trait]
impl<F, S, I, M, O> AsyncTransition<I, O> for Chained<F, S, I, M, O>
where
    Self: Send + Sized,
    F: AsyncTransition<I, M> + Send,
    S: AsyncTransition<M, O> + Send,
    I: Send,
    M: Clone + Sync + Send,
    O: Send + Sync,
{
    async fn transition(
        &mut self,
        context: Context,
        arguments: I,
    ) -> std::sync::Arc<TransitionResult<O>> {
        let first_result = match self.first_result.as_ref() {
            Some(result) => result,
            None => {
                let result = self.first.transition(context, arguments).await;
                if let TransitionResult::NoTransition = result.as_ref() {
                    return Arc::new(TransitionResult::NoTransition);
                }

                self.first_result = Some(result.clone());

                let tmp_res = match result.as_ref() {
                    TransitionResult::NoTransition => unreachable!(""),
                    TransitionResult::NextState(_) => TransitionResult::NoTransition,
                    TransitionResult::Error(e) => TransitionResult::Error(e.clone()),
                };

                return Arc::new(tmp_res);
            }
        };

        let intermediate = match first_result.as_ref() {
            TransitionResult::Error(e) => return Arc::new(TransitionResult::Error(e.clone())),
            TransitionResult::NextState(value) => value.clone(),
            _ => unreachable!(""),
        };

        self.second.transition(context, intermediate).await
    }
}

#[cfg(test)]
mod tests {
    use crate::messages::single_transition::SingleState;

    use std::sync::atomic;

    use super::*;

    #[tokio::test]
    async fn transitions() {
        let mut chained = Chained::new(
            SingleState::new(|_, _: ()| async move {
                println!("Executing First");
                TransitionResult::NextState(13)
            }),
            SingleState::new(|_, number| async move {
                println!("Executing Second");
                TransitionResult::NextState(number * 2)
            }),
        );
        let expected = 13 * 2;

        match chained.transition(Context::default(), ()).await.as_ref() {
            TransitionResult::NoTransition => {}
            res => panic!("Unexpected Error or transition: {:?}", res),
        };

        match chained.transition(Context::default(), ()).await.as_ref() {
            TransitionResult::NextState(result) => assert_eq!(expected, *result),
            res => panic!("Unexected Error or no Transition: {:?}", res),
        };
    }

    #[tokio::test]
    async fn delayed_transitions() {
        let shared = Arc::new(atomic::AtomicU8::new(5));

        let mut chained = Chained::new(
            SingleState::new(|_, _: ()| {
                let inner = shared.clone();
                async move {
                    inner.fetch_sub(1, atomic::Ordering::SeqCst);

                    let current = inner.load(atomic::Ordering::SeqCst);
                    if current > 0 {
                        println!("Current: {:?}", current);

                        return TransitionResult::NoTransition;
                    }

                    TransitionResult::NextState(13)
                }
            }),
            SingleState::new(|_, number| async move {
                println!("Executing Second");
                TransitionResult::NextState(number * 2)
            }),
        );
        let expected = 13 * 2;

        for _ in 0..5 {
            match chained.transition(Context::default(), ()).await.as_ref() {
                TransitionResult::NoTransition => {}
                res => panic!("Unexpected Error or transition: {:?}", res),
            };
        }

        match chained.transition(Context::default(), ()).await.as_ref() {
            TransitionResult::NextState(result) => assert_eq!(expected, *result),
            res => panic!("Unexected Error or no Transition: {:?}", res),
        };
    }
}
