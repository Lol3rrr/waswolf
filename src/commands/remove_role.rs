use serenity::{
    client::Context,
    framework::standard::{Args, CommandResult},
    http::CacheHttp,
    model::channel::Message,
};

use crate::{get_storage, util};

#[tracing::instrument(skip(ctx, msg, args))]
pub async fn remove_role(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    tracing::debug!("Received remove-role Command");

    let channel_id = msg.channel_id;
    let guild_id = msg.guild_id.unwrap();

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

    match storage.backend().remove_role(guild_id, role_name).await {
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
