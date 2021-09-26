use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt::{Debug, Display},
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
    roles::{self, WereWolfRole},
    Reactions, RoleCount,
};

mod channels;
mod start;
mod stop;

#[derive(Debug, Clone)]
pub struct RoundState<S> {
    owner: BTreeSet<UserId>,
    message: MessageId,
    channel: ChannelId,
    guild: GuildId,
    state: S,
}

const DEAD_ROLE_NAME: &str = "W-Dead";

impl<S> RoundState<S> {
    async fn get_msg(&self, ctx: &Context) -> Result<Message, serenity::Error> {
        self.channel.message(&ctx.http, self.message).await
    }
    pub async fn update_msg(
        &self,
        ctx: &Context,
        msg: &str,
        reactions: &[Reactions],
    ) -> Result<Message, serenity::Error> {
        let mut cfg_message = self.get_msg(ctx).await?;

        cfg_message.delete_reactions(&ctx.http).await?;
        cfg_message.edit(&ctx.http, |m| m.content(msg)).await?;

        for reaction in reactions {
            cfg_message.react(&ctx.http, reaction).await?;
        }

        Ok(cfg_message)
    }

    /// Checks if the given User is registered as an Owner
    pub fn is_owner(&self, id: &UserId) -> bool {
        self.owner.contains(id)
    }

    /// Loads the ID of the Role for Dead players or creates it if it does not
    /// currently exist
    #[tracing::instrument(skip(self, ctx))]
    async fn dead_role(&self, ctx: &Context) -> Result<RoleId, serenity::Error> {
        let g_roles = self.guild.roles(&ctx.http).await?;

        let role_index_result = g_roles
            .iter()
            .find(|(_, role)| role.name.to_lowercase() == DEAD_ROLE_NAME.to_lowercase());

        let id = match role_index_result {
            Some((id, _)) => *id,
            None => {
                tracing::debug!("Creating Role for Dead-Players: {:?}", DEAD_ROLE_NAME);

                let nrole = self
                    .guild
                    .create_role(&ctx.http, |r| r.name(DEAD_ROLE_NAME).position(0))
                    .await?;

                nrole.id
            }
        };
        Ok(id)
    }
    /// Loads the ID of the Role for Dead players or creates it if it does not
    /// currently exist
    #[tracing::instrument(skip(self, ctx))]
    async fn everyone_role(&self, ctx: &Context) -> Result<RoleId, serenity::Error> {
        let g_roles = self.guild.roles(&ctx.http).await?;

        Ok(*g_roles
            .iter()
            .min_by(|(_, x), (_, y)| x.position.cmp(&y.position))
            .expect("There is always at least the @everyone Role-Available")
            .0)
    }
}

#[derive(Debug, Clone)]
pub struct RegisterUsers {
    /// All the Participants for the Round
    participants: Vec<UserId>,
}
#[derive(Debug, Clone)]
pub struct RegisterRoles {
    /// All the Participants for the Round
    participants: Vec<UserId>,
    /// The selected Roles for the current Round
    roles: Vec<WereWolfRole>,
    /// The current Page that is displayed for the Role-Selection
    role_page: usize,
}
#[derive(Debug, Clone)]
pub struct RoleCounts {
    /// All the Participants for the current Round
    participants: Vec<UserId>,
    /// The Roles and the Number of Players for each Role
    roles: BTreeMap<WereWolfRole, usize>,
    /// The Messages used to get the Number of Players for a Role, which can
    /// be given to Multiple Players
    role_messages: HashMap<MessageId, WereWolfRole>,
}
#[derive(Debug, Clone)]
pub struct Ongoing {
    /// All the Participants for the Round as well as all their Roles
    participants: Vec<(UserId, WereWolfRole)>,
    /// The ChannelID of the Moderator Channel
    moderator_channel: ChannelId,
    /// The Channels for all the Roles in the current Game
    channels: BTreeMap<String, ChannelId>,
}
#[derive(Debug, Clone)]
pub struct Done {}

use async_trait::async_trait;

#[derive(Clone, Copy)]
pub struct TransitionContext<'a> {
    pub bot_id: UserId,
    pub ctx: &'a Context,
}

#[async_trait]
pub trait Transition<S> {
    async fn transition<'a>(source: S, context: TransitionContext<'a>) -> Self;
}

pub struct TransitionError(pub Box<dyn Error + Send + Sync>);
impl TransitionError {
    pub fn new<E>(err: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self(Box::new(err))
    }
}
impl Debug for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}
impl Display for TransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl Error for TransitionError {}

#[async_trait]
pub trait TryTransition<S>
where
    Self: Sized,
{
    async fn try_transition<'a>(
        source: S,
        context: TransitionContext<'a>,
    ) -> Result<Self, TransitionError>;
}

impl RoundState<RegisterUsers> {
    pub fn new(owner: UserId, message: MessageId, channel: ChannelId, guild: GuildId) -> Self {
        let mods = {
            let mut tmp = BTreeSet::new();
            tmp.insert(owner);
            tmp
        };
        RoundState {
            owner: mods,
            message,
            channel,
            guild,
            state: RegisterUsers {
                participants: Vec::new(),
            },
        }
    }

    pub fn add_participant(&mut self, user: UserId) {
        self.state.participants.push(user);
    }
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

    pub fn add_moderator(&mut self, moderator: UserId) {
        self.owner.insert(moderator);
    }
    pub fn remove_moderator(&mut self, moderator: UserId) {
        self.owner.remove(&moderator);
    }
}

impl RoundState<RegisterRoles> {
    pub fn needs_role_count_config(&self) -> bool {
        self.state.roles.iter().any(|r| r.needs_multiple())
    }

    async fn update_page(&mut self, ctx: &Context) -> Result<(), serenity::Error> {
        let cfg_message = self.get_msg(ctx).await?;
        cfg_message.delete_reactions(&ctx.http).await?;

        let w_roles = WereWolfRole::all_roles();
        roles::cfg_role_msg_reactions(&cfg_message, ctx, &w_roles, self.state.role_page).await;
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

    pub fn add_role(&mut self, role: WereWolfRole) {
        self.state.roles.push(role);
    }
    pub fn remove_role(&mut self, role: WereWolfRole) {
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
            source.owner.len()
        );

        let w_roles = WereWolfRole::all_roles();
        let roles_msg = roles::get_roles_msg(&w_roles);

        let cfg_message = source
            .update_msg(context.ctx, &roles_msg, &[])
            .await
            .map_err(TransitionError::new)?;

        let page = 0;
        roles::cfg_role_msg_reactions(&cfg_message, context.ctx, &w_roles, page).await;

        Ok(RoundState {
            owner: source.owner,
            message: source.message,
            channel: source.channel,
            guild: source.guild,
            state: RegisterRoles {
                participants: source.state.participants,
                roles: Vec::new(),
                role_page: page,
            },
        })
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

        let data = context.ctx.data.read().await;
        let role_counts = data.get::<RoleCount>().expect("The general Datastructure to store the Messages for Role-Counts should always be registered");
        let mut role_counts = role_counts.lock().await;

        for role in source.state.roles.iter().filter(|r| r.needs_multiple()) {
            let role_msg = format!(
                "Reply with the Number of Players that should get the {}-Role",
                role
            );
            let role_q_msg = source
                .channel
                .say(&context.ctx.http, role_msg)
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

        Ok(RoundState {
            owner: source.owner,
            message: source.message,
            channel: source.channel,
            guild: source.guild,
            state: RoleCounts {
                participants: source.state.participants,
                roles: role_map,
                role_messages,
            },
        })
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

        Ok(RoundState {
            owner: source.owner,
            message: source.message,
            channel: source.channel,
            guild: source.guild,
            state: Ongoing {
                participants,
                moderator_channel: mod_channel,
                channels: role_channel,
            },
        })
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
            &source.state.participants,
            &source.state.channels,
        )
        .await;

        source
            .update_msg(context.ctx, "The Round has completed", &[])
            .await
            .map_err(TransitionError::new)?;

        Ok(RoundState {
            owner: source.owner,
            message: source.message,
            channel: source.channel,
            guild: source.guild,
            state: Done {},
        })
    }
}
