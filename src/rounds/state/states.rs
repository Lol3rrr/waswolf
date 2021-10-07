use std::collections::{BTreeMap, HashMap};

use serenity::model::id::{ChannelId, MessageId, UserId};

use crate::roles::{WereWolfRoleConfig, WereWolfRoleInstance};

#[derive(Debug, Clone)]
pub struct RegisterUsers {
    /// All the Participants for the Round
    pub participants: Vec<UserId>,
}

#[derive(Debug, Clone)]
pub struct RegisterRoles {
    /// All the Participants for the Round
    pub participants: Vec<UserId>,
    /// The selected Roles for the current Round
    pub roles: Vec<WereWolfRoleConfig>,
    /// The current Page that is displayed for the Role-Selection
    pub role_page: usize,
}

#[derive(Debug, Clone)]
pub struct RoleCounts {
    /// All the Participants for the current Round
    pub participants: Vec<UserId>,
    /// The Roles and the Number of Players for each Role
    pub roles: BTreeMap<WereWolfRoleConfig, usize>,
    /// The Messages used to get the Number of Players for a Role, which can
    /// be given to Multiple Players
    pub role_messages: HashMap<MessageId, WereWolfRoleConfig>,
}

#[derive(Debug, Clone)]
pub struct Ongoing {
    /// All the Participants for the Round as well as all their Roles
    pub participants: BTreeMap<UserId, WereWolfRoleInstance>,
    /// The ChannelID of the Moderator Channel
    pub moderator_channel: ChannelId,
    /// The Channels for all the Roles in the current Game
    pub channels: BTreeMap<String, ChannelId>,
}

#[derive(Debug, Clone)]
pub struct Done {}
