use std::collections::BTreeMap;

use lockfree::map::Map;
use serenity::{
    model::id::{GuildId, MessageId},
    prelude::Mutex,
};
use statemachines::{AsyncTransition, TransitionResult};

use crate::messages::{Context, MessageStateMachine};

pub struct StateMachineMap {
    map: Map<MessageId, Mutex<MessageStateMachine<(), ()>>>,
    running_rounds: Mutex<BTreeMap<GuildId, Option<MessageId>>>,
}

impl StateMachineMap {
    pub fn new() -> Self {
        Self {
            map: Map::new(),
            running_rounds: Mutex::new(BTreeMap::new()),
        }
    }

    pub async fn reserve_running_game(&self, guild: GuildId) -> Result<(), ()> {
        let mut current_rounds = self.running_rounds.lock().await;

        if current_rounds.contains_key(&guild) {
            Err(())
        } else {
            current_rounds.insert(guild, None);
            Ok(())
        }
    }
    /// Checks if the given Guild is registered as having an ongoing Round,
    /// if there is None we register it with the MessageID to allow for easier
    /// removal later on
    ///
    /// # Returns
    /// `Ok` if the Guild had no previously running Round and is now marked
    /// `Err` the Guild already has a running Round
    pub async fn mark_running_game(&self, guild: GuildId, message_id: MessageId) -> Result<(), ()> {
        let mut current_rounds = self.running_rounds.lock().await;

        match current_rounds.get_mut(&guild) {
            Some(internal) => {
                *internal = Some(message_id);
                Ok(())
            }
            None => Err(()),
        }
    }
    /// Unmarks the given Guild and therefore allows for new Rounds to be started
    pub async fn unmark_running_game(&self, guild: GuildId, message_id: MessageId) {
        let mut current_rounds = self.running_rounds.lock().await;

        let raw_val = match current_rounds.get(&guild) {
            Some(v) => v,
            None => return,
        };

        match raw_val {
            Some(val) if *val == message_id => {}
            _ => return,
        };

        current_rounds.remove(&guild);
    }

    pub fn get_map(&self) -> &Map<MessageId, Mutex<MessageStateMachine<(), ()>>> {
        &self.map
    }

    pub async fn update(&self, message_id: MessageId, context: Context) {
        let sm_mutex = match self.map.get(&message_id) {
            Some(s) => s,
            None => return,
        };

        let value = sm_mutex.val();
        let mut sm = value.lock().await;

        self.update_inner(&mut sm, message_id, context).await;
    }
    pub async fn try_lock_update(&self, message_id: MessageId, context: Context) -> Result<(), ()> {
        let sm_mutex = match self.map.get(&message_id) {
            Some(s) => s,
            None => return Ok(()),
        };

        let value = sm_mutex.val();
        let mut sm = match value.try_lock() {
            Ok(s) => s,
            Err(_) => return Err(()),
        };

        self.update_inner(&mut sm, message_id, context).await;
        Ok(())
    }

    async fn update_inner(
        &self,
        sm: &mut MessageStateMachine<(), ()>,
        message_id: MessageId,
        context: Context,
    ) {
        match sm.transition(context, ()).await.as_ref() {
            TransitionResult::NoTransition => {}
            TransitionResult::Done(_) => {
                self.map.remove(&message_id);

                let guild_id = sm.guild_id();
                let msg_id = sm.message_id();
                let mut current_rounds = self.running_rounds.lock().await;
                match current_rounds.get(&guild_id) {
                    Some(running_msg_id) if running_msg_id == &Some(msg_id) => {
                        current_rounds.remove(&guild_id);
                    }
                    _ => {}
                };
            }
            TransitionResult::Error(e) => {
                tracing::error!("Transitioning: {:?}", e);

                self.map.remove(&message_id);

                let guild_id = sm.guild_id();
                let msg_id = sm.message_id();
                let mut current_rounds = self.running_rounds.lock().await;
                match current_rounds.get(&guild_id) {
                    Some(running_msg_id) if running_msg_id == &Some(msg_id) => {
                        current_rounds.remove(&guild_id);
                    }
                    _ => {}
                };
            }
        };
    }

    pub fn add(&self, message_id: MessageId, sm: MessageStateMachine<(), ()>) {
        self.map.insert(message_id, Mutex::new(sm));
    }
}
