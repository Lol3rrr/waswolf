use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt::Display,
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
    async fn get_msg(&self, ctx: &Context) -> Message {
        self.channel.message(&ctx.http, self.message).await.unwrap()
    }
    async fn update_msg(&self, ctx: &Context, msg: &str, reactions: &[Reactions]) -> Message {
        let mut cfg_message = self.get_msg(ctx).await;

        cfg_message.delete_reactions(&ctx.http).await.unwrap();
        cfg_message
            .edit(&ctx.http, |m| m.content(msg))
            .await
            .unwrap();

        for reaction in reactions {
            cfg_message.react(&ctx.http, reaction).await.unwrap();
        }

        cfg_message
    }

    /// Checks if the given User is registered as an Owner
    pub fn is_owner(&self, id: &UserId) -> bool {
        self.owner.contains(id)
    }

    /// Loads the ID of the Role for Dead players or creates it if it does not
    /// currently exist
    #[tracing::instrument(skip(self, ctx))]
    async fn dead_role(&self, ctx: &Context) -> RoleId {
        let g_roles = self.guild.roles(&ctx.http).await.unwrap();

        let role_index_result = g_roles
            .iter()
            .find(|(_, role)| role.name.to_lowercase() == DEAD_ROLE_NAME.to_lowercase());

        match role_index_result {
            Some((id, _)) => id.clone(),
            None => {
                tracing::debug!("Creating Role for Dead-Players: {:?}", DEAD_ROLE_NAME);

                let nrole = self
                    .guild
                    .create_role(&ctx.http, |r| r.name(DEAD_ROLE_NAME).position(0))
                    .await
                    .unwrap();

                nrole.id
            }
        }
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

pub struct TransitionContext<'a> {
    pub ctx: &'a Context,
}

#[async_trait]
pub trait Transition<S> {
    async fn transition<'a>(source: S, context: TransitionContext<'a>) -> Self;
}

#[async_trait]
pub trait TryTransition<S>
where
    Self: Sized,
{
    async fn try_transition<'a>(
        source: S,
        context: TransitionContext<'a>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>>;
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

        match index_result {
            Some((index, _)) => {
                self.state.participants.remove(index);

                tracing::debug!("Removed User({:?}) from Round", user);
            }
            None => {}
        };
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
        self.state
            .roles
            .iter()
            .find(|r| r.needs_multiple())
            .is_some()
    }

    async fn update_page(&mut self, ctx: &Context) {
        let cfg_message = self.get_msg(ctx).await;
        cfg_message.delete_reactions(&ctx.http).await.unwrap();

        let w_roles = WereWolfRole::all_roles();
        roles::cfg_role_msg_reactions(&cfg_message, ctx, &w_roles, self.state.role_page).await;
    }
    pub async fn next_page(&mut self, ctx: &Context) {
        self.state.role_page += 1;
        self.update_page(ctx).await;
    }
    pub async fn previous_page(&mut self, ctx: &Context) {
        self.state.role_page -= 1;
        self.update_page(ctx).await;
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

        match index_result {
            Some((index, _)) => {
                self.state.roles.remove(index);

                tracing::debug!("Removed Role({:?}) from Round", role);
            }
            None => {}
        };
    }
}

impl RoundState<RoleCounts> {
    #[tracing::instrument(skip(self, ctx, message_id, reply))]
    pub async fn role_reply(&mut self, ctx: &Context, message_id: MessageId, reply: Message) {
        let role = match self.state.role_messages.remove(&message_id) {
            Some(role) => role,
            None => {
                tracing::error!("No known Role for the given Message that was responded to");
                return;
            }
        };

        let count: usize = match reply.content.parse() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Parsing Role-Count: {:?}", e);
                return;
            }
        };

        if count < 1 {
            tracing::error!("Invalid Count for Role: {}", count);
            return;
        }

        self.state.roles.insert(role.clone(), count);

        // Delete the original Role-Message as well as the Reply to the Message
        let channel_id = reply.channel_id;
        channel_id
            .delete_message(&ctx.http, message_id)
            .await
            .unwrap();
        channel_id
            .delete_message(&ctx.http, reply.id)
            .await
            .unwrap();
    }

    pub fn is_configured(&self) -> bool {
        self.state.role_messages.len() == 0
    }
}

impl RoundState<Ongoing> {
    pub async fn is_dead(&self, ctx: &Context, user: &Member) -> bool {
        let dead_role = self.dead_role(ctx).await;

        user.roles.iter().find(|r_id| **r_id == dead_role).is_some()
    }

    pub async fn clear_permissions(&self, ctx: &Context, user: UserId) {
        for channel_id in self.state.channels.values() {
            channel_id
                .delete_permission(&ctx.http, PermissionOverwriteType::Member(user))
                .await
                .unwrap();
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ToOngoingTransitionError {
    Distributing,
}
impl Display for ToOngoingTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Distributing => write!(f, "Distributing"),
        }
    }
}
impl Error for ToOngoingTransitionError {}

#[async_trait]
impl Transition<RoundState<RegisterUsers>> for RoundState<RegisterRoles> {
    #[tracing::instrument(skip(source, context))]
    async fn transition<'a>(
        source: RoundState<RegisterUsers>,
        context: TransitionContext<'a>,
    ) -> RoundState<RegisterRoles> {
        tracing::debug!(
            "Configured Users({}) and Mods({})",
            source.state.participants.len(),
            source.owner.len()
        );

        let w_roles = WereWolfRole::all_roles();
        let roles_msg = roles::get_roles_msg(&w_roles);

        let cfg_message = source.update_msg(context.ctx, &roles_msg, &[]).await;

        let page = 0;
        roles::cfg_role_msg_reactions(&cfg_message, context.ctx, &w_roles, page).await;

        RoundState {
            owner: source.owner,
            message: source.message,
            channel: source.channel,
            guild: source.guild,
            state: RegisterRoles {
                participants: source.state.participants,
                roles: Vec::new(),
                role_page: page,
            },
        }
    }
}

#[async_trait]
impl Transition<RoundState<RegisterRoles>> for RoundState<RoleCounts> {
    #[tracing::instrument(skip(source, context))]
    async fn transition<'a>(
        source: RoundState<RegisterRoles>,
        context: TransitionContext<'a>,
    ) -> RoundState<RoleCounts> {
        let mut role_messages = HashMap::new();

        let data = context.ctx.data.read().await;
        let role_counts = data.get::<RoleCount>().unwrap();
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
                .unwrap();

            role_messages.insert(role_q_msg.id, role.clone());
            role_counts.insert(role_q_msg.id, source.guild);
        }

        source
            .update_msg(context.ctx, "Configuring Roles..", &[])
            .await;

        let role_map = {
            let mut tmp = BTreeMap::new();
            for role in source.state.roles.clone() {
                tmp.insert(role, 1);
            }
            tmp
        };

        RoundState {
            owner: source.owner,
            message: source.message,
            channel: source.channel,
            guild: source.guild,
            state: RoleCounts {
                participants: source.state.participants,
                roles: role_map,
                role_messages,
            },
        }
    }
}

#[async_trait]
impl TryTransition<RoundState<RoleCounts>> for RoundState<Ongoing> {
    #[tracing::instrument(skip(source, context))]
    async fn try_transition<'a>(
        source: RoundState<RoleCounts>,
        context: TransitionContext<'a>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let dead_role_id = source.dead_role(context.ctx).await;

        let (participants, mod_channel, role_channel) =
            match start::start(&source, DEAD_ROLE_NAME, dead_role_id, context.ctx).await {
                Ok(d) => d,
                Err(e) => {
                    return Err(e);
                }
            };

        let msg = format!(
            "Starting Round react with {} to end the Round",
            Reactions::Stop
        );
        source
            .update_msg(context.ctx, &msg, &[Reactions::Stop])
            .await;

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
impl Transition<RoundState<Ongoing>> for RoundState<Done> {
    #[tracing::instrument(skip(source, context))]
    async fn transition<'a>(source: RoundState<Ongoing>, context: TransitionContext<'a>) -> Self {
        let role_id = source.dead_role(context.ctx).await;
        stop::stop(
            role_id,
            context.ctx,
            source.guild,
            &source.state.participants,
            &source.state.channels,
        )
        .await;

        source
            .update_msg(context.ctx, "The Round has completed", &[])
            .await;

        RoundState {
            owner: source.owner,
            message: source.message,
            channel: source.channel,
            guild: source.guild,
            state: Done {},
        }
    }
}
