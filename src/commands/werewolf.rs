use serenity::{
    client::Context,
    framework::standard::CommandResult,
    http::CacheHttp,
    model::{channel::Message, id::GuildId},
    prelude::Mutex,
};

use crate::{
    get_storage, roles::WereWolfRoleConfig, rounds::Round, storage::StorageBackend, util,
    Reactions, Rounds, MOD_ROLE_NAME,
};

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

    let mut data = ctx.data.write().await;
    let rounds = data
        .get_mut::<Rounds>()
        .expect("The shared Rounds-Datastructure should always exist in a running Instance");
    if rounds.get(&guild_id).is_some() {
        tracing::error!("There is already a Round running on the Guild");

        util::msgs::send_content(
            channel_id,
            ctx.http(),
            "There is already a running Round in this Guild",
        )
        .await;

        return Ok(());
    }

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

    let entry_msg = format!(
        "Creating new Round.\nReact with:\n{}: Enter as a Player\n{}: Start the Round itself",
        Reactions::Entry,
        Reactions::Confirm
    );

    let result = match channel_id
        .send_message(&ctx.http, |m| {
            m.content(entry_msg)
                .reactions(&[Reactions::Entry, Reactions::Confirm])
        })
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Sending New-Round Message: {:?}", e);
            return Ok(());
        }
    };
    let msg_id = result.id;

    rounds.insert(
        guild_id,
        Mutex::new(Round::new(mods, msg_id, result.channel_id, guild_id, role_configs).await),
    );

    tracing::debug!("Started new Round");

    Ok(())
}
