use serenity::{
    client::Context,
    framework::standard::{Args, CommandResult},
    http::CacheHttp,
    model::channel::Message,
};

use crate::{get_storage, parse_bool, roles::WereWolfRoleConfig};

#[tracing::instrument(skip(ctx, msg, args))]
pub async fn add_role(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    tracing::debug!("Received add-role Command");

    let channel_id = msg.channel_id;
    let guild_id = msg.guild_id.unwrap();

    let mut args_iter = args.iter::<String>();

    let name = match args_iter.next() {
        Some(n) => n.unwrap(),
        None => {
            let resp = "Missing Role-Name\nFormat: `add-role {name} {emoji} {can be assigned to multiple players} {can 'mask' another Role at the start}`";
            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| m.content(resp))
                .await
            {
                tracing::error!("Sending Response: {:?}", e);
            }

            return Ok(());
        }
    };

    let emoji = match args_iter.next() {
        Some(e) => e.unwrap(),
        None => {
            todo!("Missing Emoji for Role")
        }
    };

    let multi_player = match args_iter.next() {
        Some(raw_m) => match parse_bool(&raw_m.unwrap().to_lowercase()) {
            Some(v) => v,
            None => {
                todo!("Invalid Multi-Player for Role")
            }
        },
        None => {
            todo!("Missing Multi-Player");
        }
    };

    let masks_role = match args_iter.next() {
        Some(raw_m) => match parse_bool(&raw_m.unwrap().to_lowercase()) {
            Some(v) => v,
            None => {
                todo!("Invalid Masks-Role for Role")
            }
        },
        None => {
            todo!("Missing Masks-Role");
        }
    };

    let data = ctx.data.read().await;
    let storage = get_storage(&data);

    let backend = storage.backend();

    match backend.load_roles(guild_id).await {
        Ok(r) if r.iter().find(|c| c.name() == name.as_str()).is_some() => {
            todo!("Invalid")
        }
        _ => {}
    };

    let new_config = WereWolfRoleConfig::new(name, emoji, multi_player, masks_role);

    match backend.set_role(guild_id, new_config).await {
        Ok(_) => {
            tracing::debug!("Created new Role");

            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| m.content("Successfully added the Role"))
                .await
            {
                tracing::error!("Sending Confirmation message: {:?}", e);
            }
        }
        Err(e) => {
            tracing::error!("Setting Role: {:?}", e);

            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| m.content("Could not add the Role"))
                .await
            {
                tracing::error!("Sending Error Message: {:?}", e);
            }
        }
    };

    Ok(())
}
