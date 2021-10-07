use std::{collections::HashMap, sync::RwLock};

use serenity::model::id::GuildId;

use crate::roles::WereWolfRoleConfig;

pub struct Cache {
    roles: RwLock<HashMap<GuildId, Vec<WereWolfRoleConfig>>>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            roles: RwLock::new(HashMap::new()),
        }
    }

    pub fn populate(&self, guild_id: GuildId, roles: Vec<WereWolfRoleConfig>) {
        self.roles.write().unwrap().insert(guild_id, roles);
    }

    pub fn get_roles(&self, guild_id: GuildId) -> Option<Vec<WereWolfRoleConfig>> {
        self.roles.read().unwrap().get(&guild_id).map(|m| m.clone())
    }

    pub fn set_role(&self, guild_id: GuildId, role: WereWolfRoleConfig) {
        let mut map = self.roles.write().unwrap();

        match map.get_mut(&guild_id) {
            Some(m) => {
                m.push(role);
            }
            None => {
                map.insert(guild_id, vec![role]);
            }
        };
    }

    pub fn remove_role(&self, guild_id: GuildId, role_name: &str) {
        let mut map = self.roles.write().unwrap();

        let guild_roles = match map.get_mut(&guild_id) {
            Some(g) => g,
            None => return,
        };

        let index = match guild_roles
            .iter()
            .enumerate()
            .find(|(_, r)| r.name().eq(role_name))
        {
            Some((i, _)) => i,
            None => return,
        };

        guild_roles.remove(index);
    }
}
