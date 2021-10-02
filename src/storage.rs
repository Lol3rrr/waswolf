use async_trait::async_trait;
use serenity::model::id::GuildId;
use std::error::Error;

use crate::roles::WereWolfRole;

/// The Storage Backend that should be used to load, store and update Custom Werewolf Roles for a
/// Guild
#[async_trait]
pub trait StorageBackend {
    /// The Error that could be returned when attempting to load Roles from the Backend
    type LoadError: Error;
    /// The Error that could be returned when attempting to set a Role for a Guild
    type SetError: Error;

    /// Attempt to load all the Rules stored for the given Guild
    async fn load_roles(&self, guild: GuildId) -> Result<Vec<WereWolfRole>, Self::LoadError>;

    /// Attempt to set a Role for the Guild, this can be used both for updating an existing Role on
    /// the Guild and adding a new Role to the Guild
    async fn set_role(&self, guild: GuildId, role: WereWolfRole) -> Result<(), Self::SetError>;
}
