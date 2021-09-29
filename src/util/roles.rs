use std::collections::BTreeSet;

use serenity::{
    futures::StreamExt,
    http::Http,
    model::id::{GuildId, RoleId, UserId},
};

#[derive(Debug)]
pub enum FindRoleError {
    SerenityError(serenity::Error),
    NotFound,
}

impl From<serenity::Error> for FindRoleError {
    fn from(e: serenity::Error) -> Self {
        Self::SerenityError(e)
    }
}

/// This will search for a Role with the given Name in the Guild
pub async fn find_role(name: &str, guild: GuildId, http: &Http) -> Result<RoleId, FindRoleError> {
    let roles = guild.roles(http).await?;

    roles
        .iter()
        .find(|(_, role)| role.name.eq_ignore_ascii_case(name))
        .ok_or(FindRoleError::NotFound)
        .map(|(id, _)| *id)
}

/// Loads all Users that belong to a given Role
pub async fn role_users(role: RoleId, guild: GuildId, http: &Http) -> BTreeSet<UserId> {
    let mut member_iter = guild.members_iter(http).boxed();

    let mut result = BTreeSet::new();

    while let Some(member_res) = member_iter.next().await {
        let member = match member_res {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Loading Guild Member: {:?}", e);
                continue;
            }
        };

        for m_role in member.roles {
            if m_role == role {
                result.insert(member.user.id);
                break;
            }
        }
    }

    result
}
