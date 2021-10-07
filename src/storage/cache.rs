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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cache() {
        let cache = Cache::new();
        drop(cache);
    }

    #[test]
    fn get_empty() {
        let cache = Cache::new();

        let result = cache.get_roles(GuildId(13));

        assert_eq!(None, result);
    }

    #[test]
    fn populate_get() {
        let cache = Cache::new();

        let guild = GuildId(13);
        let raw_input = vec![WereWolfRoleConfig::new("test", ":)", false, false, vec![])];

        cache.populate(guild, raw_input.clone());

        let loaded = cache.get_roles(guild);

        assert_eq!(Some(raw_input), loaded);
    }
    #[test]
    fn set_get() {
        let cache = Cache::new();

        let role = WereWolfRoleConfig::new("test", ":)", false, false, vec![]);
        let expected = Some(vec![role.clone()]);

        cache.set_role(GuildId(13), role.clone());

        let result = cache.get_roles(GuildId(13));

        assert_eq!(expected, result);
    }

    #[test]
    fn remove_empty() {
        let cache = Cache::new();

        cache.remove_role(GuildId(13), "test");
    }
    #[test]
    fn set_remove() {
        let cache = Cache::new();

        cache.set_role(
            GuildId(13),
            WereWolfRoleConfig::new("test", ":)", false, false, vec![]),
        );

        cache.remove_role(GuildId(13), "test");

        assert_eq!(Some(vec![]), cache.get_roles(GuildId(13)));
    }
    #[test]
    fn set_remove_different() {
        let cache = Cache::new();

        let role = WereWolfRoleConfig::new("test", ":)", false, false, vec![]);
        let expected = Some(vec![role.clone()]);

        cache.set_role(GuildId(13), role);

        cache.remove_role(GuildId(13), "other");

        assert_eq!(expected, cache.get_roles(GuildId(13)));
    }
}
