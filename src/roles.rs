use std::fmt::Display;

use rand::Rng;
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn emoji(&self) -> &str {
        &self.emoji
    }

    pub fn multi_player(&self) -> bool {
        self.mutli_player
    }

    pub fn masks_role(&self) -> bool {
        self.masks_role
    }

    pub fn to_instance<R>(
        &self,
        non_nested_roles: &mut Vec<WereWolfRoleConfig>,
        rng: &mut R,
    ) -> Option<WereWolfRoleInstance>
    where
        R: Rng,
    {
        if !self.masks_role {
            return Some(WereWolfRoleInstance::new(
                self.name.clone(),
                None,
                self.other_role_channels.clone(),
            ));
        }

        if non_nested_roles.len() == 0 {
            return None;
        }
        let index: usize = rng.gen_range(0..non_nested_roles.len());

        let other_role = non_nested_roles.remove(index);
        let other_instance = other_role.to_instance(non_nested_roles, rng).unwrap();

        Some(WereWolfRoleInstance::new(
            self.name.clone(),
            Some(Box::new(other_instance)),
            self.other_role_channels.clone(),
        ))
    }

    pub fn channels(&self) -> impl Iterator<Item = String> {
        std::iter::once(self.name.clone()).chain(self.other_role_channels.clone())
    }
}

#[derive(Debug, Clone)]
pub struct WereWolfRoleInstance {
    name: String,
    masked_role: Option<Box<Self>>,
    extra_channels: Vec<String>,
}

impl WereWolfRoleInstance {
    fn new(name: String, masked_role: Option<Box<Self>>, extra_channels: Vec<String>) -> Self {
        Self {
            name,
            masked_role,
            extra_channels,
        }
    }

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
}
