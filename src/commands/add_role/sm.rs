use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use serenity::{
    http::{CacheHttp, Http},
    model::id::{ChannelId, MessageId, UserId},
};

use crate::{
    messages::{
        AsyncTransition, Event, MessageStateMachine, SingleState, TransitionError, TransitionResult,
    },
    roles::WereWolfRoleConfig,
    storage::StorageBackend,
    Reactions,
};

#[derive(Debug, Clone)]
struct FirstTransition {
    name: String,
    emoji: String,
    author: UserId,
    message: StateMessage,
}

#[derive(Debug, Clone)]
struct SecondTransition {
    name: String,
    emoji: String,
    multi_player: bool,
    author: UserId,
    message: StateMessage,
}

#[derive(Debug, Clone)]
struct ThirdTransition {
    name: String,
    emoji: String,
    multi_player: bool,
    masks_role: bool,
    extra_channels: Arc<Mutex<BTreeSet<String>>>,
    author: UserId,
    message: StateMessage,
}

#[derive(Debug, Clone)]
struct StateMessage {
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

fn extra_channel_content<'a, I>(channels: I) -> String
where
    I: Iterator<Item = &'a str>,
{
    let mut channel_str = String::new();

    for (index, channel) in channels.enumerate() {
        if index > 0 {
            channel_str.push_str(", ");
        }
        channel_str.push_str(channel);
    }

    format!(
    "Reply to this Message with all the extra Roles whose Chat this Role should also be able to read ({})", channel_str)
}

pub async fn create(
    name: String,
    author: UserId,
    channel_id: ChannelId,
    ctx: &serenity::client::Context,
) -> Result<MessageStateMachine<(), ()>, serenity::Error> {
    let msg = channel_id
        .send_message(ctx.http(), |m| {
            m.content("React with an emoji to use for the Role")
        })
        .await?;

    let guild_id = msg.guild_id.unwrap();
    let msg_id = msg.id;

    let sm = SingleState::new(move |context, _: ()| {
        let name = name.clone();
        let author = author;

        async move {
            let reaction = match context.event() {
                Some(Event::AddReaction { reaction }) => reaction,
                _ => return TransitionResult::NoTransition,
            };

            if reaction.user_id != Some(author) {
                tracing::error!("Different User tried to select an option");
                return TransitionResult::NoTransition;
            }

            let emoji = reaction.emoji.to_string();

            let http = context.http().unwrap();

            let msg = StateMessage {
                channel_id,
                message_id: msg_id,
            };

            if let Err(e) = msg
                .update(
                    http,
                    "Should the Role be able to be assigned to more than one Player?",
                    &[Reactions::Yes, Reactions::No],
                )
                .await
            {
                tracing::error!("Updating Message: {:?}", e);
                return TransitionResult::Error(Arc::new(TransitionError::Serenity));
            }

            TransitionResult::Done(FirstTransition {
                name: name.to_string(),
                emoji,
                author,
                message: msg,
            })
        }
    })
    .chain(SingleState::new(
        |context, state: FirstTransition| async move {
            let reaction = match context.event() {
                Some(Event::AddReaction { reaction }) => reaction,
                _ => return TransitionResult::NoTransition,
            };

            if reaction.user_id != Some(state.author) {
                tracing::error!("Different User tried to select an option");
                return TransitionResult::NoTransition;
            }

            let reacted_emoji = &reaction.emoji;

            let multi_player = if Reactions::Yes == reacted_emoji {
                true
            } else if Reactions::No == reacted_emoji {
                false
            } else {
                return TransitionResult::NoTransition;
            };

            if let Err(e) = state.message.update(context.http().unwrap(), "Should the Role mask/hide/contain another Role, which could be used later on in the Game?", &[Reactions::Yes, Reactions::No]).await {
                tracing::error!("Updating Message: {:?}", e);
                return TransitionResult::Error(Arc::new(TransitionError::Serenity));
            }

            TransitionResult::Done(SecondTransition {
                name: state.name,
                emoji: state.emoji,
                multi_player,
                message: state.message,
                author: state.author
            })
        },
    ))
    .chain(SingleState::new(
        |context, state: SecondTransition| async move {
            let reaction = match context.event() {
                Some(Event::AddReaction { reaction }) => reaction,
                _ => return TransitionResult::NoTransition,
            };

            if reaction.user_id != Some(state.author) {
                tracing::error!("Different User tried to select an option");
                return TransitionResult::NoTransition;
            }

            let reacted_emoji = &reaction.emoji;

            let masks = if Reactions::Yes == reacted_emoji {
                true
            } else if Reactions::No == reacted_emoji {
                false
            } else {
                return TransitionResult::NoTransition;
            };

            let content = extra_channel_content(std::iter::empty());
            if let Err(e) = state.message.update(context.http().unwrap(), content, &[Reactions::Confirm]).await {
                tracing::error!("Updating Message: {:?}", e);
                return TransitionResult::Error(Arc::new(TransitionError::Serenity));
            }

            TransitionResult::Done(ThirdTransition {
                name: state.name,
                emoji: state.emoji,
                multi_player: state.multi_player,
                masks_role: masks,
                extra_channels: Arc::new(Mutex::new(BTreeSet::new())),
                message: state.message,
                author: state.author,
            })
        },
    )).chain(SingleState::new(|context, state: ThirdTransition| async move {
        match context.event() {
            Some(Event::Reply { message }) => {
                if message.author.id != state.author {
                    tracing::error!("Different User tried to select an option");
                    return TransitionResult::NoTransition;
                }

                let content = {
                    let mut lock = state.extra_channels.lock().unwrap();
                    lock.insert(message.content.clone());

                    extra_channel_content(lock.iter().map(|s| s.as_str()))
                };

                let http = context.http().unwrap();

                if let Err(e) = message.delete(http).await {
                    tracing::error!("Removing User Reply: {:?}", e);
                }

                if let Err(e) = state.message.update(http, content, &[Reactions::Confirm]).await {
                    tracing::error!("Updating Message: {:?}", e);
                    return TransitionResult::Error(Arc::new(TransitionError::Serenity));
                }

                TransitionResult::NoTransition
            },
            Some(Event::AddReaction { reaction }) => {
                if reaction.user_id != Some(state.author) {
                    tracing::error!("Different User tried to select an option");
                    return TransitionResult::NoTransition;
                }

                if Reactions::Confirm != &reaction.emoji {
                    return TransitionResult::NoTransition;
                }

                let http = context.http().unwrap();
                let storage = context.storage().unwrap();

                if let Ok(r) = storage.load_roles(context.guild_id()).await {
                    if r.iter().any(|c| c.name() == state.name.as_str()) {
                        let resp = format!("There already exists a Role with the Name: {}", state.name);
                        if let Err(e) = state.message.update(http, resp, &[]).await {
                            tracing::error!("Updating Message with Error: {:?}", e);
                        }

                        return TransitionResult::Done(());
                    }
                    if r.iter().any(|c| c.emoji() == state.emoji.as_str()) {
                        let resp = format!("There already exists a Role with the Emoji: {}", state.emoji);
                        if let Err(e) = state.message.update(http, resp, &[]).await {
                            tracing::error!("Updating Message with Error: {:?}", e);
                        }

                        return TransitionResult::Done(());
                    }
                }

                let extra_channels = {
                    let tmp = state.extra_channels.lock().unwrap();
                    tmp.iter().map(|s| s.to_owned()).collect()
                };
                let new_config = WereWolfRoleConfig::new(state.name, state.emoji, state.multi_player, state.masks_role, extra_channels);

                match storage.set_role(context.guild_id(), new_config).await {
                    Ok(_) => {
                        tracing::debug!("Created new Role");

                        if let Err(e) = state.message.update(http, "Successfully added Role", &[]).await {
                            tracing::error!("Updating message with confirmation: {:?}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Setting Role: {:?}", e);

                        if let Err(e) = state.message.update(http, "Could not add the Role", &[]).await {
                            tracing::error!("Updating message with confirmation: {:?}", e);
                        }
                    }
                };

                TransitionResult::Done(())
            }
            _ => TransitionResult::NoTransition,
        }
    }));

    Ok(MessageStateMachine::new(guild_id, msg_id, sm))
}
