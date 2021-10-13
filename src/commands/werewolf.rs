use serenity::{
    client::Context,
    framework::standard::CommandResult,
    http::CacheHttp,
    model::{channel::Message, id::GuildId},
};

use crate::{get_storage, roles::WereWolfRoleConfig, storage::StorageBackend, util, MOD_ROLE_NAME};

mod sm;

async fn get_role_configs(
    ctx: &Context,
    guild_id: GuildId,
) -> Result<Vec<WereWolfRoleConfig>, Box<dyn std::error::Error + Send>> {
    let data = ctx.data.read().await;
    let storage = get_storage(&data);

    storage.load_roles(guild_id).await
}

#[tracing::instrument(skip(ctx, msg))]
pub async fn werewolf(ctx: &Context, msg: &Message) -> CommandResult {
    tracing::debug!("Received werewolf command");

    let guild_id = match msg.guild_id {
        Some(gid) => gid,
        None => return Ok(()),
    };
    let channel_id = msg.channel_id;

    let role_configs = match get_role_configs(ctx, guild_id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Loading Roles: {:?}", e);
            util::msgs::send_content(channel_id, ctx.http(), "Could not load Roles").await;

            return Ok(());
        }
    };

    // TODO
    // Check if a round is already running

    tracing::debug!("Starting new Round");

    let mod_role = match util::roles::find_role(MOD_ROLE_NAME, guild_id, ctx.http()).await {
        Ok(r) => r,
        Err(util::roles::FindRoleError::NotFound) => {
            tracing::error!("'Game Master'-Role does not exist on the Guild");

            util::msgs::send_content(
                channel_id,
                ctx.http(),
                &format!(
                    "Could not start a new Round as it could not find a Role with the Name '{}'",
                    MOD_ROLE_NAME
                ),
            )
            .await;

            return Ok(());
        }
        Err(e) => {
            tracing::error!("Error getting 'Game Master'-Role for Guild: {:?}", e);
            return Ok(());
        }
    };
    let mods = util::roles::role_users(mod_role, guild_id, ctx.http()).await;

    tracing::debug!("Started new Round");

    let bot_id = ctx.http.get_current_user().await.unwrap().id;

    match sm::create(ctx, guild_id, channel_id, mods, bot_id).await {
        Ok((sm_msg_id, round_sm)) => {
            crate::SMMap.insert(sm_msg_id, serenity::prelude::Mutex::new(round_sm));
        }
        Err(e) => {
            tracing::error!("Creating Round Config State-Machine: {:?}", e);
        }
    };

    Ok(())
}
