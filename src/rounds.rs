use std::collections::{BTreeSet, HashMap};

use serenity::{
    client::Context,
    model::{
        channel::{Message, Reaction},
        guild::Member,
        id::{ChannelId, GuildId, MessageId, UserId},
    },
    prelude::Mutex,
};

use crate::roles::WereWolfRoleConfig;

use self::state::TransitionError;

mod sm;
mod state;

pub use state::BotContext;

/// A Single Round of Werewolf
pub struct Round {
    sm: sm::RoundSM,
}

impl Round {
    /// Creates a new Empty Round with the given Owner
    pub async fn new(
        mods: BTreeSet<UserId>,
        message_id: MessageId,
        channel: ChannelId,
        guild_id: GuildId,
        role_configs: Vec<WereWolfRoleConfig>,
    ) -> Self {
        Self {
            sm: sm::RoundSM::new(mods, message_id, channel, guild_id, role_configs).await,
        }
    }

    #[tracing::instrument(skip(self, ctx, message_id, reply))]
    pub async fn role_reply(
        &mut self,
        bot_id: UserId,
        ctx: &Context,
        message_id: MessageId,
        reply: Message,
    ) -> Result<(), TransitionError> {
        match self
            .sm
            .clone()
            .step_role_reply(bot_id, ctx, message_id, reply)
            .await
        {
            Ok(n) => {
                self.sm = n;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[tracing::instrument(skip(self, ctx, reaction))]
    pub async fn handle_add_react(
        &mut self,
        bot_id: UserId,
        ctx: &Context,
        reaction: Reaction,
    ) -> Result<(), TransitionError> {
        match self.sm.clone().step_add_react(bot_id, ctx, reaction).await {
            Ok(nsm) => {
                self.sm = nsm;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[tracing::instrument(skip(self, _ctx, reaction))]
    pub async fn handle_remove_react(&mut self, _ctx: &Context, reaction: Reaction) {
        self.sm = self.sm.clone().step_remove_react(reaction);
    }

    #[tracing::instrument(skip(self, ctx, new))]
    pub async fn handle_member_update(&mut self, ctx: &Context, new: Member) {
        if !self.sm.is_dead(ctx, &new).await {
            return;
        }

        self.sm.clear_channel_permissions(ctx, new.user.id).await;
    }

    pub fn is_done(&self) -> bool {
        self.sm.is_done()
    }

    #[tracing::instrument(skip(self, ctx, msg))]
    pub async fn update_msg(&self, ctx: &Context, msg: &str) {
        if let Err(e) = self.sm.update_msg(ctx, msg).await {
            tracing::error!("{:?}", e);
        }
    }
}

pub struct RoundsMap {
    rounds: HashMap<GuildId, Mutex<Round>>,

    running_rounds_metric: prometheus::IntGauge,
}

impl RoundsMap {
    pub fn new(registry: &prometheus::Registry) -> Self {
        let running_metric = prometheus::IntGauge::with_opts(prometheus::Opts::new(
            "rounds_running",
            "The Number of currently running Rounds",
        ))
        .unwrap();
        running_metric.set(0);

        registry.register(Box::new(running_metric.clone())).unwrap();

        Self {
            rounds: HashMap::new(),
            running_rounds_metric: running_metric,
        }
    }

    pub fn get(&self, guild: &GuildId) -> Option<&Mutex<Round>> {
        self.rounds.get(guild)
    }
    pub fn get_from_reaction(&self, msg: &Reaction) -> Option<&Mutex<Round>> {
        let guild_id = msg.guild_id?;
        self.get(&guild_id)
    }

    pub fn insert(&mut self, id: GuildId, data: Mutex<Round>) {
        self.rounds.insert(id, data);

        self.running_rounds_metric.set(self.rounds.len() as i64);
    }

    pub fn remove(&mut self, id: &GuildId) {
        self.rounds.remove(id);

        self.running_rounds_metric.set(self.rounds.len() as i64);
    }
    pub fn remove_from_reaction(&mut self, msg: &Reaction) {
        let guild_id = match msg.guild_id {
            Some(g) => g,
            None => return,
        };

        self.remove(&guild_id);
    }
}
