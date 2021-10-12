#![warn(missing_docs)]
//! Provides a couple of tools to easily create and compose StateMachines

mod traits;
pub use traits::*;

mod next;
pub use next::Next;

mod chained;
pub use chained::Chained;
