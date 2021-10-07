use async_trait::async_trait;
use serenity::model::id::GuildId;
use std::{error::Error, sync::Arc};

use crate::roles::WereWolfRoleConfig;

pub mod discord;

mod cache;

/// The Storage Backend that should be used to load, store and update Custom Werewolf Roles for a
/// Guild
#[async_trait]
pub trait StorageBackend {
    /// Attempt to load all the Rules stored for the given Guild
    async fn load_roles(
        &self,
        guild: GuildId,
    ) -> Result<Vec<WereWolfRoleConfig>, Box<dyn Error + Send>>;

    /// Attempt to set a Role for the Guild, this can be used both for updating an existing Role on
    /// the Guild and adding a new Role to the Guild
    async fn set_role(
        &self,
        guild: GuildId,
        role: WereWolfRoleConfig,
    ) -> Result<(), Box<dyn Error + Send>>;

    /// Attempts to remove the Role with the given Name again
    async fn remove_role(
        &self,
        guild: GuildId,
        role_name: &str,
    ) -> Result<(), Box<dyn Error + Send>>;
}

pub struct Storage {
    backend: Arc<dyn StorageBackend + Send + Sync>,
    cache: Arc<cache::Cache>,
}

impl Storage {
    /// Creates a new Storage Instance with the given Backend
    pub fn new<S>(backend: S) -> Self
    where
        S: StorageBackend + Send + Sync + 'static,
    {
        Self {
            backend: Arc::new(backend),
            cache: Arc::new(cache::Cache::new()),
        }
    }
}

#[async_trait]
impl StorageBackend for Storage {
    async fn load_roles(
        &self,
        guild: GuildId,
    ) -> Result<Vec<WereWolfRoleConfig>, Box<dyn Error + Send>> {
        if let Some(r) = self.cache.get_roles(guild) {
            return Ok(r);
        }

        match self.backend.load_roles(guild).await {
            Ok(r) => {
                self.cache.populate(guild, r.clone());
                Ok(r)
            }
            Err(e) => Err(e),
        }
    }

    async fn set_role(
        &self,
        guild: GuildId,
        role: WereWolfRoleConfig,
    ) -> Result<(), Box<dyn Error + Send>> {
        self.cache.set_role(guild, role.clone());
        self.backend.set_role(guild, role).await
    }

    async fn remove_role(
        &self,
        guild: GuildId,
        role_name: &str,
    ) -> Result<(), Box<dyn Error + Send>> {
        self.cache.remove_role(guild, role_name);
        self.backend.remove_role(guild, role_name).await
    }
}
