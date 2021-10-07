use std::collections::BTreeSet;

use serenity::{
    client::Context,
    http::CacheHttp,
    model::id::{GuildId, UserId},
};

use super::roles;

#[derive(Debug)]
pub enum LoadModsError {
    FindModRole(roles::FindRoleError),
}

pub async fn load_mods(
    ctx: &Context,
    guild_id: GuildId,
    role_name: &str,
) -> Result<BTreeSet<UserId>, LoadModsError> {
    let mod_role = match roles::find_role(role_name, guild_id, ctx.http()).await {
        Ok(r) => r,
        Err(e) => return Err(LoadModsError::FindModRole(e)),
    };

    let mods = roles::role_users(mod_role, guild_id, ctx.http()).await;

    Ok(mods)
}
