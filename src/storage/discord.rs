use std::{error::Error, fmt::Display, future::ready, sync::Arc};

use async_trait::async_trait;
use serenity::{
    futures::StreamExt,
    http::Http,
    model::{
        channel::ChannelType,
        id::{ChannelId, GuildId},
    },
};

use crate::roles::WereWolfRole;

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
    async fn load_roles(&self, guild: GuildId) -> Result<Vec<WereWolfRole>, Box<dyn Error + Send>> {
        let channel_id = match self.obtain_settings_channel(guild).await {
            Ok(c) => c,
            Err(e) => {
                todo!("Could not obtain Setttings Channel");
            }
        };

        let raw_messages_iter = channel_id.messages_iter(self.http.as_ref()).boxed();

        let current_user = match self.http.get_current_user().await {
            Ok(u) => u,
            Err(e) => return Err(Box::new(DiscordLoadError::SerenityError(e))),
        };

        let message_iter = raw_messages_iter
            .filter_map(|raw_message| {
                ready(match raw_message {
                    Ok(m) => Some(m),
                    Err(_) => None,
                })
            })
            .filter(|m| ready(m.author.id == current_user.id));

        // TODO
        println!("Parse all the Settings messages and turn them intor their correct Roles");

        Ok(Vec::new())
    }

    async fn set_role(
        &self,
        guild: GuildId,
        role: WereWolfRole,
    ) -> Result<(), Box<dyn Error + Send>> {
        let channel_id = match self.obtain_settings_channel(guild).await {
            Ok(id) => id,
            Err(e) => {
                todo!("Could not obtain Settings Channel for Guild");
            }
        };

        // TODO
        println!("Save the Role in the Chat");

        Ok(())
    }
}
