use std::collections::BTreeMap;

use serenity::{
    http::Http,
    model::{
        channel::PermissionOverwriteType,
        id::{ChannelId, GuildId, RoleId, UserId},
    },
};

use crate::roles::WereWolfRoleInstance;

use super::channels;

/// This function handles all the Clean-Up when a Round has been finished
#[tracing::instrument(skip(dead_role_id, ctx, guild, participants, channels))]
pub async fn stop<'pi, PI, PIT>(
    everyone_role_id: RoleId,
    dead_role_id: RoleId,
    ctx: &Http,
    guild: GuildId,
    participants: PIT,
    channels: &BTreeMap<String, ChannelId>,
) where
    PI: Iterator<Item = (&'pi UserId, &'pi WereWolfRoleInstance)>,
    PIT: Fn() -> PI,
{
    let guild_channel = match guild.channels(ctx).await {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("Loading Channels for Guild: {:?}", e);
            return;
        }
    };
    let inactive_category_id =
        match channels::setup_inactive_category(ctx, &guild, &guild_channel).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Setting up Inactive-Category: {:?}", e);
                return;
            }
        };

    // Cleanup all the Role-Channels
    for (_, channel) in channels.iter() {
        // Reset the special Permission-Settings for Players in the current
        // Channel
        for (user, _) in participants() {
            if let Err(e) = channel
                .delete_permission(ctx, PermissionOverwriteType::Member(*user))
                .await
            {
                tracing::error!("Removing Restrictive-Permission for Player: {:?}", e);
            }
        }

        if let Err(e) = channel
            .delete_permission(ctx, PermissionOverwriteType::Role(everyone_role_id))
            .await
        {
            tracing::error!(
                "Removing Restrictive-Permission for @everyone-Role: {:?}",
                e
            );
        }

        // Move the Channel back to the Inactive-Category
        if let Err(e) = channel
            .edit(ctx, |c| c.category(inactive_category_id))
            .await
        {
            tracing::error!("Moving Channel back into Inactive-Category: {:?}", e);
        }
    }

    // Clean-Up all the Players "settings":
    // * Remove the Dead-Role if applied
    for (t_user, _) in participants() {
        let mut member = match guild.member(ctx, t_user).await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Loading Guild-Member: {:?}", e);
                continue;
            }
        };

        if let Err(e) = member.remove_role(ctx, dead_role_id).await {
            tracing::error!("Removing 'W-Dead' Role: {:?}", e);
        }
    }
}
