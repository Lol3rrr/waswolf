use serenity::{
    client::Context,
    framework::standard::{Args, CommandResult},
    http::CacheHttp,
    model::channel::Message,
};

use crate::{get_storage, storage::StorageBackend, util, MOD_ROLE_NAME};

#[tracing::instrument(skip(ctx, msg, args))]
pub async fn remove_role(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    tracing::debug!("Received remove-role Command");

    let channel_id = msg.channel_id;
    let guild_id = msg.guild_id.unwrap();

    let server_mods = match util::mods::load_mods(ctx, guild_id, MOD_ROLE_NAME).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Loading Mods: {:?}", e);

            util::msgs::send_content(channel_id, ctx.http(), "Could not load Mods for the Server")
                .await;

            return Ok(());
        }
    };
    if !server_mods.contains(&msg.author.id) {
        tracing::error!("Non Mod User executed the Command");

        util::msgs::send_content(
            channel_id,
            ctx.http(),
            &format!(
                "Only Users with the '{}'-Role can use this Command",
                MOD_ROLE_NAME
            ),
        )
        .await;

        return Ok(());
    }

    let role_name = match args.current() {
        Some(r) => r,
        None => {
            util::msgs::send_content(
                channel_id,
                ctx.http(),
                "Must supply the Name of the Role to remove",
            )
            .await;

            return Ok(());
        }
    };

    let data = ctx.data.read().await;
    let storage = get_storage(&data);

    match storage.remove_role(guild_id, role_name).await {
        Ok(_) => {
            util::msgs::send_content(
                channel_id,
                ctx.http(),
                &format!("Removed Role \"{}\"", role_name),
            )
            .await;
        }
        Err(e) => {
            tracing::error!("Removing Role: {:?}", e);

            util::msgs::send_content(
                channel_id,
                ctx.http(),
                &format!("Could not remove Role \"{}\"", role_name),
            )
            .await;
        }
    };

    Ok(())
}
