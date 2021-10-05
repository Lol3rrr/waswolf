use serenity::{
    client::Context, framework::standard::CommandResult, http::CacheHttp, model::channel::Message,
};

use crate::get_storage;

#[tracing::instrument(skip(ctx, msg))]
pub async fn list_roles(ctx: &Context, msg: &Message) -> CommandResult {
    tracing::debug!("Received list-roles Command");

    let channel_id = msg.channel_id;

    let data = ctx.data.read().await;
    let storage = get_storage(&data);

    let roles_result = storage.backend().load_roles(msg.guild_id.unwrap()).await;

    let roles = match roles_result {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Loading Roles: {:?}", e);
            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| m.content("Could not load Roles"))
                .await
            {
                tracing::error!("Sending Error Message: {:?}", e);
            }

            return Ok(());
        }
    };

    let content = if roles.len() == 0 {
        "No Roles configured".to_owned()
    } else {
        let mut tmp = "Roles \n\n".to_owned();

        for role in roles {
            tmp.push_str(&format!("* {}\n", role));
        }

        tmp
    };

    match channel_id
        .send_message(ctx.http(), |m| m.content(content))
        .await
    {
        Ok(_) => {
            tracing::debug!("Send Role-List");
        }
        Err(e) => {
            tracing::error!("Sending Role-List: {:?}", e);
        }
    };

    Ok(())
}
