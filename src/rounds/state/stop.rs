use std::collections::BTreeMap;

use serenity::{
    client::Context,
    model::{
        channel::PermissionOverwriteType,
        id::{ChannelId, GuildId, RoleId, UserId},
    },
};

use crate::roles::WereWolfRole;

use super::channels;

/// This function handles all the Clean-Up when a Round has been finished
#[tracing::instrument(skip(dead_role_id, ctx, guild, participants, channels))]
pub async fn stop(
    dead_role_id: RoleId,
    ctx: &Context,
    guild: GuildId,
    participants: &[(UserId, WereWolfRole)],
    channels: &BTreeMap<String, ChannelId>,
) {
    let guild_channel = guild.channels(&ctx.http).await.unwrap();
    let inactive_category_id = channels::setup_inactive_category(ctx, &guild, &guild_channel)
        .await
        .unwrap();

    // Cleanup all the Role-Channels
    for (_, channel) in channels.iter() {
        // Reset the special Permission-Settings for Players in the current
        // Channel
        for (user, _) in participants.iter() {
            channel
                .delete_permission(&ctx.http, PermissionOverwriteType::Member(user.clone()))
                .await
                .unwrap();
        }

        // Move the Channel back to the Inactive-Category
        channel
            .edit(&ctx.http, |c| c.category(inactive_category_id))
            .await
            .unwrap();
    }

    // Clean-Up all the Players "settings":
    // * Remove the Dead-Role if applied
    for (t_user, _) in participants.iter() {
        let mut member = guild.member(&ctx.http, t_user).await.unwrap();

        if let Err(e) = member.remove_role(&ctx.http, dead_role_id).await {
            tracing::error!("Removing 'W-Dead' Role: {:?}", e);
        }
    }
}
