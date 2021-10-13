use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{Debug, Display},
    sync::Arc,
};

use serenity::{
    http::{CacheHttp, Http},
    model::{
        channel::{Message, ReactionType},
        id::{ChannelId, GuildId, MessageId, RoleId, UserId},
    },
};
use statemachines::{AsyncTransition, TransitionResult};

use crate::{
    messages::{Context, Event, MessageStateMachine, TransitionError, WithLazyState, WithState},
    roles::{self, WereWolfRoleConfig, WereWolfRoleInstance},
    rounds::{self, start::StartSource},
    storage::StorageBackend,
    util, Reactions, DEAD_ROLE_NAME,
};

#[derive(Debug, Clone)]
struct GeneralWerewolfState<C> {
    mods: BTreeSet<UserId>,
    message: StateMessage,
    bot_user: UserId,

    inner: C,
}

#[derive(Debug, Clone)]
struct RegisterPlayers {
    players: Vec<UserId>,
}

#[derive(Debug, Clone)]
struct SelectRoles {
    players: Vec<UserId>,

    all_roles: Vec<WereWolfRoleConfig>,
    role_page: usize,
    selected_roles: BTreeSet<WereWolfRoleConfig>,
}

#[derive(Debug, Clone)]
struct RoleCounts {
    players: Vec<UserId>,

    roles: BTreeMap<WereWolfRoleConfig, usize>,
    role_messages: BTreeSet<WereWolfRoleConfig>,

    count_queue: Arc<crossbeam::queue::SegQueue<(WereWolfRoleConfig, usize)>>,
}

#[derive(Debug, Clone)]
struct Running {
    players: BTreeMap<UserId, WereWolfRoleInstance>,
    moderator_channel: ChannelId,
    channels: BTreeMap<String, ChannelId>,
}

type RegisterPlayersState = GeneralWerewolfState<RegisterPlayers>;
type SelectRolesState = GeneralWerewolfState<SelectRoles>;
type RoleCountsState = GeneralWerewolfState<RoleCounts>;
type RunningState = GeneralWerewolfState<Running>;

impl<C> GeneralWerewolfState<C> {
    pub async fn get_everyone_role(&self, http: &Http) -> Result<RoleId, serenity::Error> {
        util::roles::get_everyone_role(self.message.guild_id, http)
            .await
            .map_err(|e| match e {
                util::roles::FindRoleError::NotFound => unreachable!(""),
                util::roles::FindRoleError::SerenityError(e) => e,
            })
    }

    pub async fn get_dead_player_role(&self, http: &Http) -> Result<RoleId, serenity::Error> {
        let guild_id = self.message.guild_id;

        match util::roles::find_role(DEAD_ROLE_NAME, guild_id, http).await {
            Ok(id) => Ok(id),
            Err(util::roles::FindRoleError::NotFound) => {
                let nrole = guild_id
                    .create_role(http, |r| r.name(DEAD_ROLE_NAME).position(0))
                    .await?;

                Ok(nrole.id)
            }
            Err(util::roles::FindRoleError::SerenityError(e)) => Err(e),
        }
    }

    pub async fn handle_error<E>(&self, http: &Http, error: &E)
    where
        E: Display,
    {
        let message_content = format!("[Error] {}", error);
        if let Err(e) = self.message.update(http, message_content, &[]).await {
            tracing::error!("Updating Message with Error: {:?}", e);
        }
    }
}

impl GeneralWerewolfState<SelectRoles> {
    pub async fn from_first(
        http: &Http,
        first: GeneralWerewolfState<RegisterPlayers>,
        all_roles: Vec<WereWolfRoleConfig>,
    ) -> Result<Self, serenity::Error> {
        let instant = Self {
            mods: first.mods,
            message: first.message,
            bot_user: first.bot_user,

            inner: SelectRoles {
                players: first.inner.players,
                all_roles,
                role_page: 0,
                selected_roles: BTreeSet::new(),
            },
        };

        instant.update_msg(http).await?;

        Ok(instant)
    }

    async fn update_msg(&self, http: &Http) -> Result<(), serenity::Error> {
        let roles_content = Self::roles_content(&self.inner.all_roles);
        let roles_reactions = roles::reactions(&self.inner.all_roles, 0);

        self.message
            .update(http, roles_content, &roles_reactions)
            .await?;

        Ok(())
    }

    fn roles_content(roles: &[WereWolfRoleConfig]) -> String {
        let mut result = "Select all the Roles for the Round\n".to_string();
        for role in roles {
            result.push_str(role.emoji());
            result.push_str(": ");
            result.push_str(role.name());
            result.push_str("\n");
        }

        result.push_str(&format!(
            "\nUse {} and {} to navigate between the Pages",
            Reactions::PreviousPage,
            Reactions::NextPage
        ));

        result
    }

    fn find_role(&self, emoji: &ReactionType) -> Option<&WereWolfRoleConfig> {
        self.inner
            .all_roles
            .iter()
            .find(|r| emoji.unicode_eq(r.emoji()))
    }
}

impl GeneralWerewolfState<RoleCounts> {
    pub async fn new(http: &Http, previous: SelectRolesState) -> Result<Self, serenity::Error> {
        let queue = Arc::new(crossbeam::queue::SegQueue::new());

        let mut roles = BTreeMap::new();
        let mut role_messages = BTreeSet::new();

        let channel_id = previous.message.channel_id;

        for role in previous.inner.selected_roles {
            if role.multi_player() {
                let tmp_sm = create_role_sm(
                    http,
                    previous.message.guild_id,
                    channel_id,
                    previous.message.message_id,
                    previous.message.guild_id,
                    previous.mods.clone(),
                    role.clone(),
                    queue.clone(),
                )
                .await?;

                let msg_id = tmp_sm.message_id();
                crate::SMMAP.add(msg_id, tmp_sm);
                role_messages.insert(role);
            } else {
                roles.insert(role, 1);
            }
        }

        if let Err(e) = previous
            .message
            .update(http, "Configuring Roles...", &[])
            .await
        {
            tracing::error!("Updating Message with current Status: {:?}", e);
        }

        if role_messages.is_empty() {
            crate::NOTIFY_SM_QUEUE.notify(previous.message.message_id, previous.message.guild_id);
        }

        let instance = Self {
            mods: previous.mods,
            message: previous.message,
            bot_user: previous.bot_user,

            inner: RoleCounts {
                players: previous.inner.players,

                roles,
                role_messages,

                count_queue: queue,
            },
        };

        Ok(instance)
    }
}

impl RunningState {
    pub async fn new(
        http: &Http,
        previous: RoleCountsState,
    ) -> Result<Self, Arc<dyn std::fmt::Display + Send + Sync>> {
        if let Err(e) = previous
            .message
            .update(http, "Setting Round up...", &[])
            .await
        {
            tracing::error!("Updating Message with current Status: {:?}", e);
        }

        let everyone_role_id = previous.get_everyone_role(http).await.unwrap();
        let dead_role_id = previous.get_dead_player_role(http).await.unwrap();

        let source = StartSource {
            participants: previous.inner.players.clone(),
            roles: previous.inner.roles.clone(),
            guild: previous.message.guild_id,
            mods: previous.mods.clone(),
        };

        let (players, moderator_channel, channels) = match rounds::start::start(
            previous.bot_user,
            source,
            DEAD_ROLE_NAME,
            dead_role_id,
            everyone_role_id,
            http,
        )
        .await
        {
            Ok(d) => d,
            Err(e) => {
                previous.handle_error(http, &e).await;
                return Err(Arc::new(e));
            }
        };

        let running_content = format!(
            "Started Werewolf Round, react with {} to End the Round",
            Reactions::Stop
        );
        if let Err(e) = previous
            .message
            .update(http, &running_content, &[Reactions::Stop])
            .await
        {
            tracing::error!("Updating Message with current Status: {:?}", e);
        }

        Ok(Self {
            mods: previous.mods,
            message: previous.message,
            bot_user: previous.bot_user,

            inner: Running {
                players,
                moderator_channel,
                channels,
            },
        })
    }
}

#[derive(Debug, Clone)]
struct StateMessage {
    guild_id: GuildId,
    channel_id: ChannelId,
    message_id: MessageId,
}

impl StateMessage {
    pub async fn update<C>(
        &self,
        http: &Http,
        content: C,
        reactions: &[Reactions],
    ) -> Result<(), serenity::Error>
    where
        C: AsRef<str>,
    {
        let mut msg = self.channel_id.message(http, self.message_id).await?;

        msg.edit(http, |e| e.content(content.as_ref())).await?;

        msg.delete_reactions(http).await?;

        for reaction in reactions {
            msg.react(http, reaction).await?;
        }

        Ok(())
    }
}

pub async fn create(
    ctx: &serenity::client::Context,
    guild_id: GuildId,
    channel_id: ChannelId,
    mods: BTreeSet<UserId>,
    bot_user_id: UserId,
) -> Result<MessageStateMachine<(), ()>, serenity::Error> {
    let entry_content = format!(
        "Starting new Round\n{}: Enter as Player\n{}: Start the Round (mods only)",
        Reactions::Entry,
        Reactions::Confirm
    );
    let entry_msg = channel_id
        .send_message(ctx.http().as_ref(), |m| {
            m.content(entry_content)
                .reactions(&[Reactions::Entry, Reactions::Confirm])
        })
        .await?;

    let msg = StateMessage {
        guild_id,
        channel_id,
        message_id: entry_msg.id,
    };

    let initial_state = RegisterPlayersState {
        mods,
        message: msg,
        bot_user: bot_user_id,

        inner: RegisterPlayers {
            players: Vec::new(),
        },
    };

    let sm = WithState::new(
        initial_state,
        move |context: Context, mut state: RegisterPlayersState, _: ()| async move {
            match context.event() {
                Some(Event::AddReaction { reaction }) => {
                    let user_id = reaction.user_id.unwrap();
                    let emoji = &reaction.emoji;

                    if Reactions::Entry == emoji {
                        state.inner.players.push(user_id);
                    } else if Reactions::Confirm == emoji {
                        if !state.mods.contains(&user_id) {
                            tracing::error!(
                                "User({:?}) tried to start Round as non Moderator",
                                user_id
                            );

                            return (TransitionResult::NoTransition, state);
                        }

                        if state.inner.players.is_empty() {
                            tracing::error!("Tried to start a Round with no registered Players");

                            return (TransitionResult::NoTransition, state);
                        }

                        let storage = context.storage().unwrap();
                        let roles = storage.load_roles(state.message.guild_id).await.unwrap();

                        let next_state = match SelectRolesState::from_first(
                            context.http().unwrap(),
                            state.clone(),
                            roles,
                        )
                        .await
                        {
                            Ok(n) => n,
                            Err(_) => {
                                return (
                                    TransitionResult::Error(Arc::new(TransitionError::Serenity)),
                                    state,
                                );
                            }
                        };

                        return (TransitionResult::Done(next_state), state);
                    }

                    (TransitionResult::NoTransition, state)
                }
                Some(Event::RemoveReaction { reaction }) => {
                    let user_id = reaction.user_id.unwrap();
                    let emoji = &reaction.emoji;
                    if Reactions::Entry == emoji {
                        if let Some(index) = state
                            .inner
                            .players
                            .iter()
                            .enumerate()
                            .find(|(_, id)| *id == &user_id)
                            .map(|(index, _)| index)
                        {
                            state.inner.players.remove(index);
                        }
                    }

                    (TransitionResult::NoTransition, state)
                }
                _ => (TransitionResult::NoTransition, state),
            }
        },
    )
    .chain(WithLazyState::new(
        |arg: &SelectRolesState| arg.clone(),
        |context: Context, mut state: SelectRolesState, _: SelectRolesState| async move {
            match context.event() {
                Some(Event::AddReaction { reaction }) => {
                    let user_id = reaction.user_id.unwrap();
                    if !state.mods.contains(&user_id) {
                        tracing::error!("User({:?}) tried to select a Role", user_id);

                        return (TransitionResult::NoTransition, state);
                    }

                    let emoji = &reaction.emoji;

                    if Reactions::PreviousPage == emoji {
                        state.inner.role_page -= 1;
                        if let Err(e) = state.update_msg(context.http().unwrap()).await {
                            tracing::error!("Updating Role-List Message: {:?}", e);
                        }
                    } else if Reactions::NextPage == emoji {
                        state.inner.role_page += 1;
                        if let Err(e) = state.update_msg(context.http().unwrap()).await {
                            tracing::error!("Updating Role-List Message: {:?}", e);
                        }
                    } else if Reactions::Confirm == emoji {
                        let next_state = match RoleCountsState::new(
                            context.http().unwrap(),
                            state.clone(),
                        )
                        .await
                        {
                            Ok(n) => n,
                            Err(e) => {
                                tracing::error!("Transitioning to next State: {:?}", e);
                                return (
                                    TransitionResult::Error(Arc::new(TransitionError::Serenity)),
                                    state,
                                );
                            }
                        };

                        return (TransitionResult::Done(next_state), state);
                    } else {
                        if let Some(role) = state.find_role(emoji).cloned() {
                            state.inner.selected_roles.insert(role.clone());
                        }
                    }
                }
                Some(Event::RemoveReaction { reaction }) => {
                    let user_id = reaction.user_id.unwrap();
                    if !state.mods.contains(&user_id) {
                        return (TransitionResult::NoTransition, state);
                    }

                    let emoji = &reaction.emoji;

                    if let Some(role) = state.find_role(emoji) {
                        let cloned = role.clone();
                        state.inner.selected_roles.remove(&cloned);
                    }
                }
                _ => return (TransitionResult::NoTransition, state),
            };

            (TransitionResult::NoTransition, state)
        },
    ))
    .chain(WithLazyState::new(
        |state: &RoleCountsState| state.clone(),
        |context: Context, mut state: RoleCountsState, _: RoleCountsState| async move {
            match context.event() {
                Some(Event::Notify) => {
                    if state.inner.role_messages.is_empty() {
                        return match RunningState::new(context.http().unwrap(), state.clone()).await
                        {
                            Ok(n_state) => (TransitionResult::Done(n_state), state),
                            Err(e) => (
                                TransitionResult::Error(TransitionError::Generic(e).arced()),
                                state,
                            ),
                        };
                    }

                    let (role, count) = match state.inner.count_queue.pop() {
                        Some(e) => e,
                        None => return (TransitionResult::NoTransition, state),
                    };

                    state.inner.role_messages.remove(&role);

                    state.inner.roles.insert(role, count);

                    if state.inner.role_messages.is_empty() {
                        match RunningState::new(context.http().unwrap(), state.clone()).await {
                            Ok(n_state) => (TransitionResult::Done(n_state), state),
                            Err(e) => (
                                TransitionResult::Error(TransitionError::Generic(e).arced()),
                                state,
                            ),
                        }
                    } else {
                        (TransitionResult::NoTransition, state)
                    }
                }
                _ => (TransitionResult::NoTransition, state),
            }
        },
    ))
    .chain(WithLazyState::new(
        |state: &RunningState| state.clone(),
        |context: Context, state: RunningState, _: RunningState| async move {
            match context.event() {
                Some(Event::AddReaction { reaction }) => {
                    let user_id = reaction.user_id.unwrap();
                    if !state.mods.contains(&user_id) {
                        return (TransitionResult::NoTransition, state);
                    }

                    let emoji = &reaction.emoji;

                    if Reactions::Stop == emoji {
                        let http = context.http().unwrap();

                        let everyone_role_id = state.get_everyone_role(http).await.unwrap();
                        let dead_role_id = state.get_dead_player_role(http).await.unwrap();

                        rounds::stop::stop(
                            everyone_role_id,
                            dead_role_id,
                            http,
                            state.message.guild_id,
                            || state.inner.players.iter(),
                            &state.inner.channels,
                        )
                        .await;

                        if let Err(e) = state.message.update(http, "Round is over", &[]).await {
                            tracing::error!("Updating Message with final State: {:?}", e);
                        }

                        (TransitionResult::Done(()), state)
                    } else {
                        (TransitionResult::NoTransition, state)
                    }
                }
                _ => (TransitionResult::NoTransition, state),
            }
        },
    ));

    Ok(MessageStateMachine::new(guild_id, entry_msg.id, sm))
}

#[derive(Debug)]
struct RoleCountState {
    channel_id: ChannelId,
    current_msg: Message,
    round_msg_id: MessageId,
    round_guild_id: GuildId,
    round_mods: BTreeSet<UserId>,
    role: WereWolfRoleConfig,
    count_queue: Arc<crossbeam::queue::SegQueue<(WereWolfRoleConfig, usize)>>,
}

async fn create_role_sm(
    http: &Http,
    guild_id: GuildId,
    channel_id: ChannelId,
    round_msg_id: MessageId,
    round_guild_id: GuildId,
    round_mods: BTreeSet<UserId>,
    role: WereWolfRoleConfig,
    count_queue: Arc<crossbeam::queue::SegQueue<(WereWolfRoleConfig, usize)>>,
) -> Result<MessageStateMachine<(), ()>, serenity::Error> {
    let msg_content = format!(
        "Reply with the Number of Players that should be assigned to the '{}'-Role",
        role.name()
    );
    let msg = channel_id
        .send_message(http, |m| m.content(&msg_content))
        .await?;

    let message_id = msg.id;

    let tmp_state = RoleCountState {
        channel_id,
        current_msg: msg,
        round_msg_id,
        round_guild_id,
        round_mods,
        role,
        count_queue,
    };

    let sm = WithState::new(
        tmp_state,
        |context: Context, state: RoleCountState, _: ()| async move {
            match context.event() {
                Some(Event::Reply { message }) => {
                    if !state.round_mods.contains(&message.author.id) {
                        return (TransitionResult::NoTransition, state);
                    }

                    let raw_content = &message.content;
                    let parsed = match raw_content.parse::<usize>() {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::error!("Parsing Role Count: {:?}", e);
                            return (TransitionResult::NoTransition, state);
                        }
                    };

                    let http = context.http().unwrap();
                    if let Err(e) = message.delete(http).await {
                        tracing::error!("Deleting Response to Role-Count: {:?}", e);
                    }
                    if let Err(e) = state.current_msg.delete(http).await {
                        tracing::error!("Deleting Role-Count Message: {:?}", e);
                    }

                    state.count_queue.push((state.role.clone(), parsed));

                    crate::NOTIFY_SM_QUEUE.notify(state.round_msg_id, state.round_guild_id);

                    (TransitionResult::Done(()), state)
                }
                _ => (TransitionResult::NoTransition, state),
            }
        },
    );

    Ok(MessageStateMachine::new(guild_id, message_id, sm))
}
