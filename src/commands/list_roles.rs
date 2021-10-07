use serenity::{
    client::Context, framework::standard::CommandResult, http::CacheHttp, model::channel::Message,
};

use crate::{get_storage, roles::WereWolfRoleConfig, storage::StorageBackend, util};

fn role_list_msg(roles: &[WereWolfRoleConfig]) -> String {
    if roles.is_empty() {
        return "No Roles configured".to_owned();
    }

    let mut result = "Roles\n\n".to_owned();

    for role in roles.iter() {
        result.push_str(&format!("* {}\n", role));
    }

    result
}

#[tracing::instrument(skip(ctx, msg))]
pub async fn list_roles(ctx: &Context, msg: &Message) -> CommandResult {
    tracing::debug!("Received list-roles Command");

    let channel_id = msg.channel_id;

    let data = ctx.data.read().await;
    let storage = get_storage(&data);

    let roles_result = storage.load_roles(msg.guild_id.unwrap()).await;

    let roles = match roles_result {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Loading Roles: {:?}", e);
            util::msgs::send_content(channel_id, ctx.http(), "Could not load Roles").await;

            return Ok(());
        }
    };

    let content = role_list_msg(&roles);

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
