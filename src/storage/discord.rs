use std::{error::Error, fmt::Display, future::ready, sync::Arc};

use async_trait::async_trait;
use serenity::{
    futures::StreamExt,
    http::Http,
    model::{
        channel::{ChannelType, Message},
        id::{ChannelId, GuildId, MessageId, UserId},
    },
};

use crate::roles::WereWolfRoleConfig;

use super::StorageBackend;

const SETTINGS_CHANNEL_NAME: &str = "W-Settings";

#[derive(Debug)]
pub enum DiscordError {
    ObtainSettingsChannel,
    FindingRole,
    Serde(serde_json::Error),
    SerenityError(serenity::Error),
}

impl Display for DiscordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ObtainSettingsChannel => write!(f, "ObtainSettingsChannel"),
            Self::FindingRole => write!(f, "FindingRole"),
            Self::Serde(e) => write!(f, "Serde ({})", e),
            Self::SerenityError(e) => write!(f, "Serenity ({})", e),
        }
    }
}
impl Error for DiscordError {}

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
    async fn obtain_settings_channel(&self, guild: GuildId) -> Option<ChannelId> {
        if let Ok(id) = self.get_settings_channel(guild).await {
            return Some(id);
        }

        if let Ok(id) = self.create_settings_channel(guild).await {
            return Some(id);
        }

        None
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
    ) -> Option<MessageId> {
        let message_iter = self.settings_message_iter(channel_id, bot_id).await;

        let mut result_iter = message_iter
            .map(|msg| {
                let parsed = serde_json::from_str::<WereWolfRoleConfig>(&msg.content);
                (msg, parsed)
            })
            .filter(|(_, p_res)| ready(p_res.is_ok()))
            .map(|(msg, tmp)| (msg, tmp.unwrap()))
            .filter(|(_, config)| ready(config.name() == role_name));

        result_iter.next().await.map(|(c, _)| c.id)
    }

    async fn load_roles(&self, guild: GuildId) -> Result<Vec<WereWolfRoleConfig>, DiscordError> {
        let channel_id = match self.obtain_settings_channel(guild).await {
            Some(c) => c,
            None => {
                return Err(DiscordError::ObtainSettingsChannel);
            }
        };

        let current_user = match self.http.get_current_user().await {
            Ok(u) => u,
            Err(e) => return Err(DiscordError::SerenityError(e)),
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

    async fn set_role(&self, guild: GuildId, role: WereWolfRoleConfig) -> Result<(), DiscordError> {
        let channel_id = match self.obtain_settings_channel(guild).await {
            Some(id) => id,
            None => {
                return Err(DiscordError::ObtainSettingsChannel);
            }
        };

        let serialized = match serde_json::to_string(&role) {
            Ok(s) => s,
            Err(e) => {
                return Err(DiscordError::Serde(e));
            }
        };

        if let Err(e) = channel_id
            .send_message(self.http.as_ref(), |m| m.content(serialized))
            .await
        {
            return Err(DiscordError::SerenityError(e));
        }

        Ok(())
    }

    async fn remove_role(&self, guild: GuildId, role_name: &str) -> Result<(), DiscordError> {
        let channel_id = match self.obtain_settings_channel(guild).await {
            Some(id) => id,
            None => {
                return Err(DiscordError::ObtainSettingsChannel);
            }
        };

        let current_user = match self.http.get_current_user().await {
            Ok(u) => u,
            Err(e) => return Err(DiscordError::SerenityError(e)),
        };

        let role_msg_id = match self
            .find_role_message(channel_id, current_user.id, role_name)
            .await
        {
            Some(id) => id,
            None => {
                return Err(DiscordError::FindingRole);
            }
        };

        match channel_id
            .delete_message(self.http.as_ref(), role_msg_id)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(DiscordError::SerenityError(e)),
        }
    }
}

#[async_trait]
impl StorageBackend for DiscordStorage {
    async fn load_roles(
        &self,
        guild: GuildId,
    ) -> Result<Vec<WereWolfRoleConfig>, Box<dyn Error + Send>> {
        self.load_roles(guild)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    }

    async fn set_role(
        &self,
        guild: GuildId,
        role: WereWolfRoleConfig,
    ) -> Result<(), Box<dyn Error + Send>> {
        self.set_role(guild, role)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    }

    async fn remove_role(
        &self,
        guild: GuildId,
        role_name: &str,
    ) -> Result<(), Box<dyn Error + Send>> {
        self.remove_role(guild, role_name)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
    }
}
