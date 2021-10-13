use std::sync::Arc;

use serenity::{
    http::Http,
    model::id::{GuildId, MessageId},
};
use statemachines::{AsyncTransition, TransitionResult};
use tokio::sync::OnceCell;

use crate::{
    messages::{Context, Event},
    storage::Storage,
};

pub struct NotifyQueue {
    queue: OnceCell<Arc<tokio::sync::mpsc::UnboundedSender<(MessageId, GuildId)>>>,
}

impl NotifyQueue {
    pub fn new() -> Self {
        Self {
            queue: OnceCell::new(),
        }
    }

    pub fn notify(&self, msg_id: MessageId, guild_id: GuildId) {
        self.queue.get().unwrap().send((msg_id, guild_id)).unwrap();
    }
}

pub async fn run_notifier(http: Arc<Http>, storage: Storage) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    crate::NOTIFY_SM_QUEUE.queue.set(Arc::new(tx)).unwrap();

    tokio::spawn(background_notifier(http, storage, rx));
}

async fn background_notifier(
    http: Arc<Http>,
    storage: Storage,
    mut queue: tokio::sync::mpsc::UnboundedReceiver<(MessageId, GuildId)>,
) {
    loop {
        let (msg_id, guild_id) = match queue.recv().await {
            Some(m) => m,
            None => {
                tracing::error!("All Senders have Dropped");
                return;
            }
        };

        let sm_entry = match crate::SMMap.get(&msg_id) {
            Some(s) => s,
            None => {
                continue;
            }
        };

        let context = Context::new(
            Some(http.clone()),
            Some(Event::Notify),
            Some(storage.clone()),
            guild_id,
        );

        let sm_lock = sm_entry.val();
        let mut sm = match sm_lock.try_lock() {
            Ok(s) => s,
            Err(_) => {
                crate::NOTIFY_SM_QUEUE.notify(msg_id, guild_id);
                continue;
            }
        };

        match sm.transition(context, ()).await.as_ref() {
            TransitionResult::NoTransition => {
                tracing::debug!("No Transition");
            }
            TransitionResult::Done(_) => {
                tracing::debug!("Transition Done");
            }
            TransitionResult::Error(e) => {
                tracing::error!("Error Transitioning: {:?}", e);
            }
        };
    }
}
