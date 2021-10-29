use serenity::{
    client::Context, framework::standard::CommandResult, http::CacheHttp, model::channel::Message,
};

use crate::{util, MOD_ROLE_NAME};

mod sm;

#[tracing::instrument(skip(ctx, msg))]
pub async fn werewolf(ctx: &Context, msg: &Message) -> CommandResult {
    tracing::debug!("Received werewolf command");

    let guild_id = match msg.guild_id {
        Some(gid) => gid,
        None => return Ok(()),
    };
    let channel_id = msg.channel_id;

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

    if !mods.contains(&msg.author.id) {
        tracing::error!("Non Mod attempted to start Round");
        util::msgs::send_content(
            channel_id,
            ctx.http(),
            "Only Moderators/Game Masters can start a new Round",
        )
        .await;

        return Ok(());
    }

    if crate::SMMAP.reserve_running_game(guild_id).await.is_err() {
        tracing::error!("Attempted to start new Round in Guild with running Round");
        util::msgs::send_content(
            channel_id,
            ctx.http(),
            "There already exists an ongoing Round",
        )
        .await;

        return Ok(());
    }

    tracing::debug!("Starting new Round");

    let bot_id = ctx.http.get_current_user().await.unwrap().id;

    match sm::create(ctx, guild_id, channel_id, mods, bot_id).await {
        Ok(round_sm) => {
            let sm_msg_id = round_sm.message_id();

            crate::SMMAP.add(sm_msg_id, round_sm);
            crate::SMMAP
                .mark_running_game(guild_id, sm_msg_id)
                .await
                .unwrap();
        }
        Err(e) => {
            tracing::error!("Creating Round Config State-Machine: {:?}", e);
        }
    };

    Ok(())
}
