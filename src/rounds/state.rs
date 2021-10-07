use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt::Debug,
};

use serenity::{
    client::Context,
    model::{
        channel::{Message, PermissionOverwriteType},
        guild::Member,
        id::{ChannelId, GuildId, MessageId, RoleId, UserId},
    },
};

use crate::{
    roles::{self, WereWolfRoleConfig},
    util, Reactions, RoleCount,
};

mod channels;
mod start;
mod stop;

mod states;
pub use states::*;

mod traits;
pub use traits::*;

pub struct StringError(String);

impl std::fmt::Debug for StringError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    write!(f, "{:?}", self.0)
  }
}
impl std::fmt::Display for StringError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    write!(f, "{}", self.0)
  }
}
impl std::error::Error for StringError {}


/// The State for a given Round
#[derive(Debug, Clone)]
pub struct RoundState<S> {
    /// The Set of Users that can actually manage the Round
    mods: BTreeSet<UserId>,
    /// The Root Message that is used to perform all the Configurations
    message: MessageId,
    channel: ChannelId,
    /// The GuildID for the Round
    guild: GuildId,
    /// The possible Roles that could be configured for the Round
    role_configs: Vec<WereWolfRoleConfig>,
    /// The Current State of the Round
    state: S,
}

/// The Name of the Role used for Dead-Players
const DEAD_ROLE_NAME: &str = "W-Dead";

impl<S> RoundState<S> {
    async fn new_raw(
        mods: BTreeSet<UserId>,
        message: MessageId,
        channel: ChannelId,
        guild: GuildId,
        role_configs: Vec<WereWolfRoleConfig>,
        state: S,
    ) -> Self {
        Self {
            mods,
            message,
            channel,
            guild,
            role_configs,
            state,
        }
    }

    pub fn transition<T>(self, n_state: T) -> RoundState<T> {
        RoundState {
            mods: self.mods,
            message: self.message,
            channel: self.channel,
            guild: self.guild,
            role_configs: self.role_configs,
            state: n_state,
        }
    }

    pub fn find_role_config(&self, emoji: &str) -> Option<WereWolfRoleConfig> {
        self.role_configs
            .iter()
            .find(|r| r.emoji() == emoji)
            .cloned()
    }

    async fn get_msg(&self, ctx: &dyn BotContext) -> Result<Message, serenity::Error> {
        self.channel.message(ctx.get_http(), self.message).await
    }

    pub async fn update_msg(
        &self,
        ctx: &dyn BotContext,
        msg: &str,
        reactions: &[Reactions],
    ) -> Result<Message, serenity::Error> {
        let mut cfg_message = self.get_msg(ctx).await?;

        cfg_message.delete_reactions(ctx.get_http()).await?;
        cfg_message.edit(ctx.get_http(), |m| m.content(msg)).await?;

        for reaction in reactions {
            cfg_message.react(ctx.get_http(), reaction).await?;
        }

        Ok(cfg_message)
    }

    /// Checks if the given User is registered as an Owner
    pub fn is_owner(&self, id: &UserId) -> bool {
        self.mods.contains(id)
    }

    /// Loads the ID of the Role for Dead players or creates it if it does not
    /// currently exist
    async fn dead_role(&self, ctx: &dyn BotContext) -> Result<RoleId, serenity::Error> {
        let id = match util::roles::find_role(DEAD_ROLE_NAME, self.guild, ctx.get_http()).await {
            Ok(id) => id,
            Err(_) => {
                let nrole = self
                    .guild
                    .create_role(ctx.get_http(), |r| r.name(DEAD_ROLE_NAME).position(0))
                    .await?;

                nrole.id
            }
        };

        Ok(id)
    }
    /// Loads the ID of the Role for Dead players or creates it if it does not
    /// currently exist
    async fn everyone_role(&self, ctx: &dyn BotContext) -> Result<RoleId, serenity::Error> {
        let g_roles = self.guild.roles(ctx.get_http()).await?;

        Ok(*g_roles
            .iter()
            .min_by(|(_, x), (_, y)| x.position.cmp(&y.position))
            .expect("There is always at least the @everyone Role-Available")
            .0)
    }
}

use async_trait::async_trait;

impl RoundState<RegisterUsers> {
    pub async fn new(
        mods: BTreeSet<UserId>,
        message: MessageId,
        channel: ChannelId,
        guild: GuildId,
        role_configs: Vec<WereWolfRoleConfig>,
    ) -> Self {
        let state = RegisterUsers {
            participants: Vec::new(),
        };

        Self::new_raw(mods, message, channel, guild, role_configs, state).await
    }

    /// Adds a new Player to the Round
    pub fn add_participant(&mut self, user: UserId) {
        self.state.participants.push(user);

        tracing::debug!("Added User({:?}) to Round", user);
    }
    /// Removes a Player from the round again
    pub fn remove_participant(&mut self, user: UserId) {
        let index_result = self
            .state
            .participants
            .iter()
            .enumerate()
            .find(|(_, u)| **u == user);

        let index = match index_result {
            Some((i, _)) => i,
            None => return,
        };

        self.state.participants.remove(index);

        tracing::debug!("Removed User({:?}) from Round", user);
    }
}

impl RoundState<RegisterRoles> {
    pub fn needs_role_count_config(&self) -> bool {
        self.state.roles.iter().any(|r| r.multi_player())
    }

    async fn update_page(&mut self, ctx: &Context) -> Result<(), serenity::Error> {
        let cfg_message = self.get_msg(ctx).await?;
        cfg_message.delete_reactions(&ctx.http).await?;

        roles::cfg_role_msg_reactions(&cfg_message, ctx, &self.role_configs, self.state.role_page)
            .await;
        Ok(())
    }
    pub async fn next_page(&mut self, ctx: &Context) -> Result<(), serenity::Error> {
        self.state.role_page += 1;
        self.update_page(ctx).await
    }
    pub async fn previous_page(&mut self, ctx: &Context) -> Result<(), serenity::Error> {
        self.state.role_page -= 1;
        self.update_page(ctx).await
    }

    pub fn add_role(&mut self, role: WereWolfRoleConfig) {
        self.state.roles.push(role);
    }
    pub fn remove_role(&mut self, role: WereWolfRoleConfig) {
        let index_result = self
            .state
            .roles
            .iter()
            .enumerate()
            .find(|(_, r)| **r == role);

        let index = match index_result {
            Some((index, _)) => index,
            None => return,
        };

        self.state.roles.remove(index);

        tracing::debug!("Removed Role({:?}) from Round", role);
    }
}

impl RoundState<RoleCounts> {
    #[tracing::instrument(skip(self, ctx, message_id, reply))]
    pub async fn role_reply(
        &mut self,
        ctx: &Context,
        message_id: MessageId,
        reply: Message,
    ) -> Result<(), serenity::Error> {
        // TODO
        // Take a closer look at these Failure cases

        let role = match self.state.role_messages.remove(&message_id) {
            Some(role) => role,
            None => {
                tracing::error!("No known Role for the given Message that was responded to");
                return Ok(());
            }
        };

        let count: usize = match reply.content.parse() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Parsing Role-Count: {:?}", e);
                return Ok(());
            }
        };

        if count < 1 {
            tracing::error!("Invalid Count for Role: {}", count);
            return Ok(());
        }

        self.state.roles.insert(role.clone(), count);

        // Delete the original Role-Message as well as the Reply to the Message
        let channel_id = reply.channel_id;
        channel_id.delete_message(&ctx.http, message_id).await?;
        channel_id.delete_message(&ctx.http, reply.id).await?;

        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        self.state.role_messages.is_empty()
    }
}

impl RoundState<Ongoing> {
    pub async fn is_dead(&self, ctx: &Context, user: &Member) -> bool {
        let dead_role = match self.dead_role(ctx).await {
            Ok(d) => d,
            Err(_) => {
                return false;
            }
        };

        user.roles.iter().any(|r_id| *r_id == dead_role)
    }

    #[tracing::instrument(skip(self, ctx, user))]
    pub async fn clear_permissions(&self, ctx: &Context, user: UserId) {
        for channel_id in self.state.channels.values() {
            if let Err(e) = channel_id
                .delete_permission(&ctx.http, PermissionOverwriteType::Member(user))
                .await
            {
                tracing::error!("{:?}", e);
            }
        }
    }
}

#[async_trait]
impl TryTransition<RoundState<RegisterUsers>> for RoundState<RegisterRoles> {
    #[tracing::instrument(skip(source, context))]
    async fn try_transition<'a>(
        source: RoundState<RegisterUsers>,
        context: TransitionContext<'a>,
    ) -> Result<RoundState<RegisterRoles>, TransitionError> {
        tracing::debug!(
            "Configured Users({}) and Mods({})",
            source.state.participants.len(),
            source.mods.len()
        );

        let playerCount: usize = source.state.participants.len();
        if playerCount < 1{
            return Err(TransitionError::new(StringError("Cannot start the game with no player".to_string())));
            

        }
        let roles_msg = roles::get_roles_msg(&source.role_configs);

        let cfg_message = source
            .update_msg(context.ctx, &roles_msg, &[])
            .await
            .map_err(TransitionError::new)?;

        let page = 0;
        roles::cfg_role_msg_reactions(&cfg_message, context.ctx, &source.role_configs, page).await;

        let nstate = RegisterRoles {
            participants: source.state.participants.clone(),
            roles: Vec::new(),
            role_page: page,
        };
        Ok(source.transition(nstate))
        
    }
}

#[async_trait]
impl TryTransition<RoundState<RegisterRoles>> for RoundState<RoleCounts> {
    #[tracing::instrument(skip(source, context))]
    async fn try_transition<'a>(
        source: RoundState<RegisterRoles>,
        context: TransitionContext<'a>,
    ) -> Result<Self, TransitionError> {
        let mut role_messages = HashMap::new();

        let data_lock = context.ctx.get_data();
        let data = data_lock.read().await;
        let role_counts = data.get::<RoleCount>().expect("The general Datastructure to store the Messages for Role-Counts should always be registered");
        let mut role_counts = role_counts.lock().await;

        for role in source.state.roles.iter().filter(|r| r.multi_player()) {
            let role_msg = format!(
                "Reply with the Number of Players that should get the {}-Role",
                role.name()
            );
            let role_q_msg = source
                .channel
                .say(&context.ctx.get_http(), role_msg)
                .await
                .map_err(TransitionError::new)?;

            role_messages.insert(role_q_msg.id, role.clone());
            role_counts.insert(role_q_msg.id, source.guild);
        }

        source
            .update_msg(context.ctx, "Configuring Roles..", &[])
            .await
            .map_err(TransitionError::new)?;

        let role_map = {
            let mut tmp = BTreeMap::new();
            for role in source.state.roles.clone() {
                tmp.insert(role, 1);
            }
            tmp
        };

        let nstate = RoleCounts {
            participants: source.state.participants.clone(),
            roles: role_map,
            role_messages,
        };
        Ok(source.transition(nstate))
    }
}

#[async_trait]
impl TryTransition<RoundState<RoleCounts>> for RoundState<Ongoing> {
    #[tracing::instrument(skip(source, context))]
    async fn try_transition<'a>(
        source: RoundState<RoleCounts>,
        context: TransitionContext<'a>,
    ) -> Result<Self, TransitionError> {
        let dead_role_id = source
            .dead_role(context.ctx)
            .await
            .map_err(TransitionError::new)?;

        let everyone_role_id = source
            .everyone_role(context.ctx)
            .await
            .map_err(TransitionError::new)?;

        let (participants, mod_channel, role_channel) = match start::start(
            context.bot_id,
            &source,
            DEAD_ROLE_NAME,
            dead_role_id,
            everyone_role_id,
            context.ctx,
        )
        .await
        {
            Ok(d) => d,
            Err(e) => {
                return Err(TransitionError::new(e));
            }
        };

        let msg = format!(
            "Starting Round react with {} to end the Round",
            Reactions::Stop
        );
        source
            .update_msg(context.ctx, &msg, &[Reactions::Stop])
            .await
            .map_err(TransitionError::new)?;

        let nstate = Ongoing {
            participants,
            moderator_channel: mod_channel,
            channels: role_channel,
        };
        Ok(source.transition(nstate))
    }
}

#[async_trait]
impl TryTransition<RoundState<Ongoing>> for RoundState<Done> {
    #[tracing::instrument(skip(source, context))]
    async fn try_transition<'a>(
        source: RoundState<Ongoing>,
        context: TransitionContext<'a>,
    ) -> Result<Self, TransitionError> {
        let dead_role_id = source
            .dead_role(context.ctx)
            .await
            .map_err(TransitionError::new)?;
        let everyone_role_id = source
            .everyone_role(context.ctx)
            .await
            .map_err(TransitionError::new)?;

        stop::stop(
            everyone_role_id,
            dead_role_id,
            context.ctx,
            source.guild,
            || source.state.participants.iter(),
            &source.state.channels,
        )
        .await;

        source
            .update_msg(context.ctx, "The Round has completed", &[])
            .await
            .map_err(TransitionError::new)?;

        let nstate = Done {};
        Ok(source.transition(nstate))
    }
}

