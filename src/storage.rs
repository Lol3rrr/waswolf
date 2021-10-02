use async_trait::async_trait;
use serenity::model::id::GuildId;
use std::{error::Error, sync::Arc};

use crate::roles::WereWolfRole;

pub mod discord;

/// The Storage Backend that should be used to load, store and update Custom Werewolf Roles for a
/// Guild
#[async_trait]
pub trait StorageBackend {
    /// Attempt to load all the Rules stored for the given Guild
    async fn load_roles(&self, guild: GuildId) -> Result<Vec<WereWolfRole>, Box<dyn Error + Send>>;

    /// Attempt to set a Role for the Guild, this can be used both for updating an existing Role on
    /// the Guild and adding a new Role to the Guild
    async fn set_role(
        &self,
        guild: GuildId,
        role: WereWolfRole,
    ) -> Result<(), Box<dyn Error + Send>>;
}

pub struct Storage {
    backend: Arc<dyn StorageBackend + Send + Sync>,
}

impl Storage {
    /// Creates a new Storage Instance with the given Backend
    pub fn new<S>(backend: S) -> Self
    where
        S: StorageBackend + Send + Sync + 'static,
    {
        Self {
            backend: Arc::new(backend),
        }
    }

    /// Returns a reference to the underlying Storage Backend
    pub fn backend(&self) -> &(dyn StorageBackend + Send + Sync) {
        self.backend.as_ref()
    }
}
