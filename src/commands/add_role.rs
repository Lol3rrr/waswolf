use serenity::{
    client::Context,
    framework::standard::{Args, CommandResult},
    http::CacheHttp,
    model::channel::Message,
};

use crate::{
    get_storage, parse_bool, roles::WereWolfRoleConfig, storage::StorageBackend, util,
    MOD_ROLE_NAME,
};

fn missing_part(missing_part: &str) -> String {
    format!("```
Missing '{}'
Format: 'add-role {{name}} {{emoji}} {{mutli-player}} {{masks role}} {{extra Role Channels}}'
Parts:
    * 'name': The Name of the new Role
    * 'emoji': The Emoji that will be used to select the Role
    * 'multi-player': Whether or not the Role can be assigned to multiple Players in the same round
    * 'masks role': Whether or not the Role 'hides' another Role at the beginning of the Round, like when a Player with this Role only gets their real Role later on
    * 'extra Role Channels': A Comma seperated List of other Role-Chats that this Role should have access to
```", missing_part)
}

#[tracing::instrument(skip(ctx, msg, args))]
pub async fn add_role(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    tracing::debug!("Received add-role Command");

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

    let mut args_iter = args.iter::<String>().map(|m| m.unwrap());

    let name = match args_iter.next() {
        Some(n) => n,
        None => {
            let resp = missing_part("Name");
            util::msgs::send_content(channel_id, ctx.http(), &resp).await;

            return Ok(());
        }
    };

    let emoji = match args_iter.next() {
        Some(e) => e,
        None => {
            let resp = missing_part("Emoji");
            util::msgs::send_content(channel_id, ctx.http(), &resp).await;

            return Ok(());
        }
    };

    let multi_player = match args_iter.next() {
        Some(raw_m) => match parse_bool(&raw_m.to_lowercase()) {
            Some(v) => v,
            None => {
                let resp = format!(
                    "```
Invalid Value for 'Multi-Player':
Expected one of 'true', 'yes', 'y', 'false', 'no', 'n'
Got: '{}'
```",
                    raw_m
                );
                util::msgs::send_content(channel_id, ctx.http(), &resp).await;

                return Ok(());
            }
        },
        None => {
            let resp = missing_part("Multi-Player");
            util::msgs::send_content(channel_id, ctx.http(), &resp).await;

            return Ok(());
        }
    };

    let masks_role = match args_iter.next() {
        Some(raw_m) => match parse_bool(&raw_m.to_lowercase()) {
            Some(v) => v,
            None => {
                let resp = format!(
                    "```
Invalid Value for 'Masks Role':
Expected one of 'true', 'yes', 'y', 'false', 'no', 'n'
Got: '{}'
```",
                    raw_m
                );
                util::msgs::send_content(channel_id, ctx.http(), &resp).await;

                return Ok(());
            }
        },
        None => {
            let resp = missing_part("Masks Role");
            util::msgs::send_content(channel_id, ctx.http(), &resp).await;

            return Ok(());
        }
    };

    let other_channels = match args_iter.next() {
        Some(raw) => raw.split(',').map(|p| p.to_string()).collect(),
        None => Vec::new(),
    };

    let data = ctx.data.read().await;
    let storage = get_storage(&data);

    if let Ok(r) = storage.load_roles(guild_id).await {
        if r.iter().any(|c| c.name() == name.as_str()) {
            let resp = format!("There already exists a Role with the Name: {}", name);
            util::msgs::send_content(channel_id, ctx.http(), &resp).await;

            return Ok(());
        }
        if r.iter().any(|c| c.emoji() == emoji.as_str()) {
            let resp = format!("There already exists a Role with the Emoji: {}", emoji);
            util::msgs::send_content(channel_id, ctx.http(), &resp).await;

            return Ok(());
        }
    }

    let new_config = WereWolfRoleConfig::new(name, emoji, multi_player, masks_role, other_channels);

    match storage.set_role(guild_id, new_config).await {
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
