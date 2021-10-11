use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serenity::model::channel::Message;

mod cfg_reactions;

mod roles_msg;
pub use roles_msg::get_roles_msg;

mod distribute;
pub use distribute::{distribute_roles, DistributeError};

use crate::rounds::BotContext;

pub async fn cfg_role_msg_reactions(
    message: &Message,
    ctx: &dyn BotContext,
    roles: &[WereWolfRoleConfig],
    page: usize,
) {
    let reactions = cfg_reactions::reactions(roles, page);
    for reaction in reactions {
        if let Err(e) = message.react(ctx.get_http(), reaction).await {
            tracing::error!("Adding Reaction: {:?}", e);
        }
    }
}

/// The Config for a Custom Werewolf Role
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct WereWolfRoleConfig {
    /// The Name of the Role used for Displaying it as well as for the Channel names
    name: String,
    /// The Emoji used to select the Role itself when creating the Round and the like
    emoji: String,
    /// Whether or not this Role can be assigned to mutliple Players in a single Round, notable
    /// examples of this would be the "Werewolf" Role itself
    mutli_player: bool,
    /// Whether or not this Role "masks" another Role, meaning that it also needs one more Role
    /// which will also be assigned to the Player and will be used by the Player at some Point in
    /// the Game
    masks_role: bool,
    /// A Lsit of other Role-Channels that a Player should be added to, like when a Player with
    /// this Role needs access to one general Chat that belongs to another Role as well as their
    /// own Chat
    #[serde(default)]
    other_role_channels: Vec<String>,
}

impl Display for WereWolfRoleConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({}) - Multiple Players: {} - Contains another Role: {} - Accesses other Channels: {:?}",
            self.name, self.emoji, self.mutli_player, self.masks_role, self.other_role_channels
        )
    }
}

impl PartialOrd for WereWolfRoleConfig {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.name.partial_cmp(&other.name)
    }
}
impl Ord for WereWolfRoleConfig {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl WereWolfRoleConfig {
    /// Creates a new Role-Config based on the given Data
    pub fn new<N, E>(
        name: N,
        emoji: E,
        mutli_player: bool,
        masks_role: bool,
        other_role_channels: Vec<String>,
    ) -> Self
    where
        N: Into<String>,
        E: Into<String>,
    {
        Self {
            name: name.into(),
            emoji: emoji.into(),
            mutli_player,
            masks_role,
            other_role_channels,
        }
    }

    /// The Name of the Role
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The Emoji that is intended to be used for this
    pub fn emoji(&self) -> &str {
        &self.emoji
    }

    /// Whether or not the Role can be assigned to multiple-Players
    pub fn multi_player(&self) -> bool {
        self.mutli_player
    }

    /// Whether or not this Role can "mask"/"contain" another Role in an actual Round
    pub fn masks_role(&self) -> bool {
        self.masks_role
    }

    /// Creates an actual Role-Instance from this Config, will use the provided function to get
    /// another Role if this Config needs/masks another Role
    pub fn to_instance<F>(&self, get_masked: &mut F) -> WereWolfRoleInstance
    where
        F: FnMut() -> WereWolfRoleConfig,
    {
        if !self.masks_role {
            return WereWolfRoleInstance::new(
                self.name.clone(),
                None,
                self.other_role_channels.clone(),
            );
        }

        let other_role = get_masked();
        let other_instance = other_role.to_instance(get_masked);

        WereWolfRoleInstance::new(
            self.name.clone(),
            Some(Box::new(other_instance)),
            self.other_role_channels.clone(),
        )
    }

    /// Gets the List of all Channel Names that this Role needs access to
    pub fn channels(&self) -> impl Iterator<Item = String> {
        std::iter::once(self.name.clone()).chain(self.other_role_channels.clone())
    }
}

/// An actual Instance of a Role, which is intended to be used for a running Round
#[derive(Debug, Clone, PartialEq)]
pub struct WereWolfRoleInstance {
    /// The Name of the Role
    name: String,
    /// The Role masked by this Role, if any
    masked_role: Option<Box<Self>>,
    /// A List of extra Channels that this Role needs access to
    extra_channels: Vec<String>,
}

impl WereWolfRoleInstance {
    /// Creates a new Role-Instace with the given Data
    fn new(name: String, masked_role: Option<Box<Self>>, extra_channels: Vec<String>) -> Self {
        Self {
            name,
            masked_role,
            extra_channels,
        }
    }

    /// Gets the Channels that this Role Instance actually needs access to
    pub fn channels(&self) -> Vec<String> {
        let mut result = vec![self.name.clone()];

        if let Some(other) = &self.masked_role {
            result.push(other.name.clone());
        }

        for other_role in self.extra_channels.iter() {
            result.push(other_role.to_string());
        }

        result
    }

    /// The Name of the Role this instance is based upon
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The Role that is masked by this Role, if any
    pub fn masked_role(&self) -> Option<&Self> {
        match &self.masked_role {
            Some(r) => Some(&r),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_simple() {
        let instance = WereWolfRoleInstance::new("Test".to_string(), None, Vec::new());
        let expected = vec!["Test".to_string()];

        let result = instance.channels();

        assert_eq!(expected, result);
    }
    #[test]
    fn channels_masked_role() {
        let instance = WereWolfRoleInstance::new(
            "Test".to_string(),
            Some(Box::new(WereWolfRoleInstance::new(
                "Other".to_string(),
                None,
                Vec::new(),
            ))),
            Vec::new(),
        );
        let expected = vec!["Test".to_string(), "Other".to_string()];

        let result = instance.channels();

        assert_eq!(expected, result);
    }
    #[test]
    fn channels_extra_roles() {
        let instance =
            WereWolfRoleInstance::new("Test".to_string(), None, vec!["Extra".to_string()]);
        let expected = vec!["Test".to_string(), "Extra".to_string()];

        let result = instance.channels();

        assert_eq!(expected, result);
    }

    #[test]
    fn to_instance_not_masking() {
        let config = WereWolfRoleConfig::new("root", "", false, false, Vec::new());
        let expected = WereWolfRoleInstance::new(config.name().to_string(), None, Vec::new());

        let result = config.to_instance(&mut || panic!("We dont want to mask another Role"));

        assert_eq!(expected, result);
    }
    #[test]
    fn to_instance_masking() {
        let config = WereWolfRoleConfig::new("root", "", false, true, Vec::new());
        let expected = WereWolfRoleInstance::new(
            config.name().to_string(),
            Some(Box::new(WereWolfRoleInstance::new(
                "inner".to_string(),
                None,
                Vec::new(),
            ))),
            Vec::new(),
        );

        let result = config
            .to_instance(&mut || WereWolfRoleConfig::new("inner", "", false, false, Vec::new()));

        assert_eq!(expected, result);
    }
}
