use serenity::{
    client::Context,
    framework::standard::{Args, CommandResult},
    http::CacheHttp,
    model::channel::Message,
};

use crate::{util, MOD_ROLE_NAME};

mod sm;

fn missing_part(missing_part: &str) -> String {
    format!(
        "```
Missing '{}'
Format: 'add-role {{name}}'
```",
        missing_part
    )
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

    let sm = sm::create(name.clone(), msg.author.id, channel_id, ctx)
        .await
        .unwrap();

    let sm_msg_id = sm.message_id();
    crate::SMMAP.add(sm_msg_id, sm);

    Ok(())
}
