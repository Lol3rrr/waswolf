use std::error::Error;

use serenity::{
    client::Context,
    model::{
        channel::{Message, Reaction},
        guild::Member,
        id::{ChannelId, GuildId, MessageId, UserId},
    },
};

use crate::{
    roles::WereWolfRole,
    rounds::state::{Transition, TransitionContext},
    Reactions,
};

use super::state::{
    Done, Ongoing, RegisterRoles, RegisterUsers, RoleCounts, RoundState, TryTransition,
};

#[derive(Debug, Clone)]
pub enum RoundSM {
    RegisterUsers(RoundState<RegisterUsers>),
    RegisterRoles(RoundState<RegisterRoles>),
    RoleCounts(RoundState<RoleCounts>),
    Ongoing(RoundState<Ongoing>),
    Done(RoundState<Done>),
}

impl RoundSM {
    /// Creates a new Empty Round with the given Owner
    pub fn new(
        owner: UserId,
        message_id: MessageId,
        channel: ChannelId,
        guild_id: GuildId,
    ) -> Self {
        Self::RegisterUsers(RoundState::new(owner, message_id, channel, guild_id))
    }

    #[tracing::instrument(skip(self, ctx, reaction))]
    pub async fn step_add_react(self, ctx: &Context, reaction: Reaction) -> Self {
        let user_id = reaction.user_id.unwrap();
        let react_data = reaction.emoji;

        match self {
            Self::RegisterUsers(mut state) => {
                if Reactions::Entry == react_data {
                    state.add_participant(user_id);

                    tracing::debug!("Added User({:?}) to Round", user_id);
                    return Self::RegisterUsers(state);
                }
                if Reactions::ModEntry == react_data {
                    state.add_moderator(user_id);

                    tracing::debug!("Added Moderator({:?}) to Round", user_id);
                    return Self::RegisterUsers(state);
                }

                if Reactions::Confirm == react_data {
                    if !state.is_owner(&user_id) {
                        tracing::error!("Non-Owner attempted to start Round");
                        return Self::RegisterUsers(state);
                    }

                    tracing::debug!("Confirmed Round");
                    let nstate: RoundState<RegisterRoles> =
                        Transition::transition(state, TransitionContext { ctx }).await;
                    return Self::RegisterRoles(nstate);
                }

                Self::RegisterUsers(state)
            }
            Self::RegisterRoles(mut state) => {
                if Reactions::Confirm == react_data {
                    let needs_role_config = state.needs_role_count_config();

                    let nstate: RoundState<RoleCounts> =
                        Transition::transition(state, TransitionContext { ctx }).await;

                    if needs_role_config {
                        return Self::RoleCounts(nstate);
                    } else {
                        let nstate: RoundState<Ongoing> =
                            match TryTransition::try_transition(nstate, TransitionContext { ctx })
                                .await
                            {
                                Ok(n) => n,
                                Err(e) => {
                                    tracing::error!("Transitioning {:?}", e);
                                    panic!("");
                                }
                            };
                        return Self::Ongoing(nstate);
                    }
                }

                if Reactions::NextPage == react_data {
                    if !state.is_owner(&user_id) {
                        tracing::error!("Non-Owner attempted to switch Pages");
                        return Self::RegisterRoles(state);
                    }

                    state.next_page(ctx).await;

                    return Self::RegisterRoles(state);
                }
                if Reactions::PreviousPage == react_data {
                    if !state.is_owner(&user_id) {
                        tracing::error!("Non-Owner attempted to switch Pages");
                        return Self::RegisterRoles(state);
                    }

                    state.previous_page(ctx).await;

                    return Self::RegisterRoles(state);
                }

                match WereWolfRole::from_emoji(react_data.clone()) {
                    Some(role) => {
                        tracing::info!("Added Role({:?}) to Round", role);
                        state.add_role(role);

                        Self::RegisterRoles(state)
                    }
                    None => {
                        tracing::error!("Unknown Role: {:?}", react_data);
                        Self::RegisterRoles(state)
                    }
                }
            }
            Self::RoleCounts(state) => Self::RoleCounts(state),
            Self::Ongoing(state) => {
                if Reactions::Stop == react_data {
                    tracing::info!("Stopping/Ending Round");

                    let nstate: RoundState<Done> =
                        Transition::transition(state, TransitionContext { ctx }).await;
                    return Self::Done(nstate);
                }

                Self::Ongoing(state)
            }
            Self::Done(state) => Self::Done(state),
        }
    }

    #[tracing::instrument(skip(self, reaction))]
    pub fn step_remove_react(self, reaction: Reaction) -> Self {
        let user_id = reaction.user_id.unwrap();
        let react_data = reaction.emoji.clone();

        match self {
            Self::RegisterUsers(mut state) => {
                if Reactions::Entry == react_data {
                    tracing::debug!("Removed User({:?}) from Round", user_id);

                    state.remove_participant(user_id);
                    return Self::RegisterUsers(state);
                }
                if Reactions::ModEntry == react_data {
                    tracing::debug!("Removed Moderator({:?}) from Round", user_id);

                    state.remove_moderator(user_id);
                    return Self::RegisterUsers(state);
                }

                Self::RegisterUsers(state)
            }
            Self::RegisterRoles(mut state) => {
                if !state.is_owner(&user_id) {
                    tracing::error!("Non User-Attempted to remove Role");
                    return Self::RegisterRoles(state);
                }

                let removed_role = match WereWolfRole::from_emoji(reaction.emoji.clone()) {
                    Some(r) => r,
                    None => {
                        tracing::error!("Unknown Reaction was removed");
                        return Self::RegisterRoles(state);
                    }
                };

                tracing::debug!("Removed Role({:?}) from Round", removed_role);

                state.remove_role(removed_role);
                Self::RegisterRoles(state)
            }
            Self::RoleCounts(state) => Self::RoleCounts(state),
            Self::Ongoing(state) => Self::Ongoing(state),
            Self::Done(state) => Self::Done(state),
        }
    }

    #[tracing::instrument(skip(self, ctx, message_id, reply))]
    pub async fn step_role_reply(
        self,
        ctx: &Context,
        message_id: MessageId,
        reply: Message,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        match self {
            Self::RegisterUsers(state) => Ok(Self::RegisterUsers(state)),
            Self::RegisterRoles(state) => Ok(Self::RegisterRoles(state)),
            Self::RoleCounts(mut state) => {
                state.role_reply(ctx, message_id, reply).await;

                if state.is_configured() {
                    match TryTransition::try_transition(state, TransitionContext { ctx }).await {
                        Ok(n) => Ok(Self::Ongoing(n)),
                        Err(e) => Err(e),
                    }
                } else {
                    Ok(Self::RoleCounts(state))
                }
            }
            Self::Ongoing(state) => Ok(Self::Ongoing(state)),
            Self::Done(state) => Ok(Self::Done(state)),
        }
    }

    pub async fn is_dead(&self, ctx: &Context, user: &Member) -> bool {
        match self {
            Self::Ongoing(state) => state.is_dead(ctx, user).await,
            _ => false,
        }
    }

    pub async fn clear_channel_permissions(&self, ctx: &Context, user_id: UserId) {
        match self {
            Self::Ongoing(state) => state.clear_permissions(ctx, user_id).await,
            _ => {}
        }
    }

    pub fn is_done(&self) -> bool {
        match self {
            Self::Done(_) => true,
            _ => false,
        }
    }
}