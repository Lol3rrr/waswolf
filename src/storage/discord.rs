use std::{error::Error, fmt::Display, future::ready, sync::Arc};

use async_trait::async_trait;
use serenity::{
    futures::StreamExt,
    http::Http,
    model::{
        channel::{ChannelType, Message},
        id::{ChannelId, GuildId, MessageId, UserId},
    },
    FutureExt,
};

use crate::roles::WereWolfRoleConfig;

use super::StorageBackend;

const SETTINGS_CHANNEL_NAME: &str = "W-Settings";

/// The Discord Storage Backend
pub struct DiscordStorage {
    http: Arc<Http>,
}

impl DiscordStorage {
    /// Creates a new Discord Storage Instance with the given Http instance for making all the
    /// needed API Calls to Discord itself
    pub fn new(http: Arc<Http>) -> Self {
        Self { http }
    }

    /// Attempts to load the Settings Channel for the given Guild, returns an error if there
    /// was an error while interacting with the Discord API or when the Channel could not be
    /// found
    #[tracing::instrument(skip(self))]
    async fn get_settings_channel(&self, guild: GuildId) -> Result<ChannelId, ()> {
        let channels = match guild.channels(self.http.as_ref()).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Loading Guild Channels: {:?}", e);
                return Err(());
            }
        };

        let result = channels
            .into_iter()
            .find(|(_, channel)| channel.name().eq_ignore_ascii_case(SETTINGS_CHANNEL_NAME))
            .map(|(id, _)| id);

        match result {
            Some(id) => Ok(id),
            None => {
                tracing::error!("Could not find Settings channel for Guild {:?}", guild);
                Err(())
            }
        }
    }

    /// Attempts to create the Settings Channel for the Guild
    #[tracing::instrument(skip(self))]
    async fn create_settings_channel(&self, guild: GuildId) -> Result<ChannelId, ()> {
        let create_channel_result = guild
            .create_channel(self.http.as_ref(), |c| {
                c.name(SETTINGS_CHANNEL_NAME)
                    .kind(ChannelType::Text)
                    .topic("A simple Storage Channel for the Settings of the Bot")
            })
            .await;

        match create_channel_result {
            Ok(c) => Ok(c.id),
            Err(e) => {
                tracing::error!("Creating Settings Channel: {:?}", e);
                Err(())
            }
        }
    }

    /// This is used to obtain the ID of the Settings Channel, by either loading it when the
    /// Channel already exists on the Guild or by creating the Channel in case the Settings Channel
    /// could not be found
    async fn obtain_settings_channel(&self, guild: GuildId) -> Result<ChannelId, ()> {
        match self.get_settings_channel(guild).await {
            Ok(id) => return Ok(id),
            Err(_) => {}
        };

        match self.create_settings_channel(guild).await {
            Ok(id) => return Ok(id),
            Err(_) => {}
        };

        Err(())
    }

    async fn settings_message_iter(
        &'_ self,
        channel_id: ChannelId,
        bot_id: UserId,
    ) -> impl serenity::futures::Stream<Item = Message> + '_ {
        let raw_msg_iter = channel_id.messages_iter(self.http.as_ref()).boxed();

        raw_msg_iter
            .filter_map(|raw_message| {
                ready(match raw_message {
                    Ok(m) => Some(m),
                    Err(_) => None,
                })
            })
            .filter(move |m| ready(m.author.id == bot_id))
    }

    async fn find_role_message(
        &self,
        channel_id: ChannelId,
        bot_id: UserId,
        role_name: &str,
    ) -> Result<MessageId, ()> {
        // TODO

        let message_iter = self.settings_message_iter(channel_id, bot_id).await;

        let mut result_iter = message_iter
            .map(|msg| {
                let parsed = serde_json::from_str::<WereWolfRoleConfig>(&msg.content);
                (msg, parsed)
            })
            .filter(|(_, p_res)| ready(p_res.is_ok()))
            .map(|(msg, tmp)| (msg, tmp.unwrap()))
            .filter(|(_, config)| ready(config.name() == role_name));

        match result_iter.next().await {
            Some((c, _)) => Ok(c.id),
            None => {
                todo!("Handle non existing Role Search")
            }
        }
    }
}

#[derive(Debug)]
pub enum DiscordLoadError {
    SerenityError(serenity::Error),
}

impl Display for DiscordLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerenityError(e) => write!(f, "Serentiy ({})", e),
        }
    }
}
impl Error for DiscordLoadError {}

#[derive(Debug)]
pub enum DiscordSetError {
    SerenityError(serenity::Error),
}

impl Display for DiscordSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerenityError(e) => write!(f, "Serenity ({})", e),
        }
    }
}
impl Error for DiscordSetError {}

#[async_trait]
impl StorageBackend for DiscordStorage {
    async fn load_roles(
        &self,
        guild: GuildId,
    ) -> Result<Vec<WereWolfRoleConfig>, Box<dyn Error + Send>> {
        let channel_id = match self.obtain_settings_channel(guild).await {
            Ok(c) => c,
            Err(e) => {
                todo!("Could not obtain Setttings Channel");
            }
        };

        let current_user = match self.http.get_current_user().await {
            Ok(u) => u,
            Err(e) => return Err(Box::new(DiscordLoadError::SerenityError(e))),
        };

        let message_iter = self
            .settings_message_iter(channel_id, current_user.id)
            .await;

        let role_config_iter = message_iter
            .map(|msg| serde_json::from_str::<WereWolfRoleConfig>(&msg.content))
            .filter_map(|p_res| async move {
                match p_res {
                    Ok(v) => Some(v),
                    Err(_) => None,
                }
            });

        Ok(role_config_iter.collect().await)
    }

    async fn set_role(
        &self,
        guild: GuildId,
        role: WereWolfRoleConfig,
    ) -> Result<(), Box<dyn Error + Send>> {
        let channel_id = match self.obtain_settings_channel(guild).await {
            Ok(id) => id,
            Err(e) => {
                todo!("Could not obtain Settings Channel for Guild");
            }
        };

        // TODO
        // Check if the Role already exists and then only update it otherwise continue with
        // the current Function and create a new Entry for it

        let serialized = match serde_json::to_string(&role) {
            Ok(s) => s,
            Err(e) => {
                todo!("Handle Serialize failure");
            }
        };

        if let Err(e) = channel_id
            .send_message(self.http.as_ref(), |m| m.content(serialized))
            .await
        {
            todo!("Handle Error sending Config to channel");
        }

        Ok(())
    }

    async fn remove_role(
        &self,
        guild: GuildId,
        role_name: &str,
    ) -> Result<(), Box<dyn Error + Send>> {
        tracing::debug!("Remove Role: {:?}", role_name);

        // TODO
        let channel_id = match self.obtain_settings_channel(guild).await {
            Ok(id) => id,
            Err(e) => {
                todo!("Could not obtain Settings Channel for Guild");
            }
        };

        let current_user = match self.http.get_current_user().await {
            Ok(u) => u,
            Err(e) => return Err(Box::new(DiscordLoadError::SerenityError(e))),
        };

        let role_msg_id = match self
            .find_role_message(channel_id, current_user.id, role_name)
            .await
        {
            Ok(id) => id,
            Err(e) => {
                todo!("Could not find Role Message");
            }
        };

        match channel_id
            .delete_message(self.http.as_ref(), role_msg_id)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                todo!("Handle delete Error")
            }
        }
    }
}
