use serenity::{
    client::Context,
    framework::standard::{Args, CommandResult},
    http::CacheHttp,
    model::channel::Message,
};

use crate::get_storage;

#[tracing::instrument(skip(ctx, msg, args))]
pub async fn remove_role(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    tracing::debug!("Received remove-role Command");

    let channel_id = msg.channel_id;
    let guild_id = msg.guild_id.unwrap();

    let role_name = match args.current() {
        Some(r) => r,
        None => {
            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| {
                    m.content("Must supply the Name of the Role to remove")
                })
                .await
            {
                tracing::error!("Sending invalid Args Message: {:?}", e);
            }

            return Ok(());
        }
    };

    let data = ctx.data.read().await;
    let storage = get_storage(&data);

    match storage.backend().remove_role(guild_id, role_name).await {
        Ok(_) => {
            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| {
                    m.content(format!("Removed Role \"{}\"", role_name))
                })
                .await
            {
                tracing::error!("Sending Confirmation message: {:?}", e);
            }
        }
        Err(e) => {
            tracing::error!("Removing Role: {:?}", e);

            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| {
                    m.content(format!("Could not remove Role \"{}\"", role_name))
                })
                .await
            {
                tracing::error!("Sending Error message: {:?}", e);
            }
        }
    };

    Ok(())
}
