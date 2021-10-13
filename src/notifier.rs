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

        let context = Context::new(
            Some(http.clone()),
            Some(Event::Notify),
            Some(storage.clone()),
            guild_id,
        );

        match crate::SMMAP.try_lock_update(msg_id, context).await {
            Ok(_) => {}
            Err(_) => {
                crate::NOTIFY_SM_QUEUE.notify(msg_id, guild_id);
                continue;
            }
        };
    }
}
