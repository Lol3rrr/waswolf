use std::{collections::BTreeMap, error::Error};

use serenity::{
    client::Context,
    model::{
        channel::{PermissionOverwrite, PermissionOverwriteType},
        id::{ChannelId, RoleId, UserId},
        Permissions,
    },
};

use crate::{
    roles::{self, WereWolfRole},
    rounds::state::ToOngoingTransitionError,
};

use super::{channels, RoleCounts, RoundState};

fn channel_access_permissions(user: UserId) -> PermissionOverwrite {
    PermissionOverwrite {
        allow: Permissions::READ_MESSAGES | Permissions::SEND_MESSAGES,
        deny: Permissions { bits: 0 },
        kind: PermissionOverwriteType::Member(user),
    }
}

/// Handles all the Setup-Stuff for starting the actual Round based on the
/// Configuration
#[tracing::instrument(skip(source, dead_role_id, ctx))]
pub async fn start(
    source: &RoundState<RoleCounts>,
    dead_role_name: &str,
    dead_role_id: RoleId,
    ctx: &Context,
) -> Result<
    (
        Vec<(UserId, WereWolfRole)>,
        ChannelId,
        BTreeMap<String, ChannelId>,
    ),
    Box<dyn Error + Send + Sync>,
> {
    let default_permissions: Vec<PermissionOverwrite> = {
        let mut tmp: Vec<PermissionOverwrite> = source
            .state
            .participants
            .iter()
            .map(|user| PermissionOverwrite {
                allow: Permissions { bits: 0 },
                deny: Permissions::READ_MESSAGES,
                kind: PermissionOverwriteType::Member(user.clone()),
            })
            .collect();

        tmp.push(PermissionOverwrite {
            allow: Permissions::READ_MESSAGES,
            deny: Permissions { bits: 0 },
            kind: PermissionOverwriteType::Role(dead_role_id),
        });

        tmp
    };

    let guild_channel = source.guild.channels(&ctx.http).await.unwrap();

    let active_category_id =
        channels::setup_active_category(ctx, &source.guild, &guild_channel).await;

    let role_iter = source.state.roles.iter().map(|(role, _)| role);
    let role_channel = channels::setup_role_channels(
        role_iter,
        default_permissions.clone(),
        source.guild,
        &guild_channel,
        &active_category_id,
        ctx,
        &source.owner,
    )
    .await;

    let mod_channel = channels::setup_moderator_channel(
        default_permissions,
        source.guild,
        &guild_channel,
        &active_category_id,
        ctx,
        &source.owner,
    )
    .await;

    let participants = match roles::distribute_roles(
        source.state.participants.clone(),
        source.state.roles.clone(),
    ) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Distributing the Roles to the Participants");
            return Err(
                Box::new(ToOngoingTransitionError::Distributing) as Box<dyn Error + Send + Sync>
            );
        }
    };

    for (user_id, role) in participants.iter() {
        let access_permissions = channel_access_permissions(user_id.clone());

        let role_channels = role.channels();
        for tmp_c in role_channels {
            let channel = role_channel.get(&tmp_c).unwrap();

            channel
                .create_permission(&ctx.http, &access_permissions)
                .await
                .unwrap();
        }
    }

    // The Mod Message to inform the Moderators about all the Roles
    {
        let info_msg = format!("```
The Round has now been started and all the required Setup has been completed

If a Player has died, they should be given the '{}'-Role and the Bot will then update the Configuration \
to allow that Player to see all Channels again and watch the Round from the 'Outside'.

Once the Round is over, the Bot will automatically remove all the Round-Relevant Roles from the Players again \
and reorganize the relevant Channels to prepare for the next Round.
            ```", dead_role_name
        );
        mod_channel.say(&ctx.http, info_msg).await.unwrap();

        let msg = {
            let mut tmp = "Roles:\n".to_string();

            for (user_id, role) in participants.iter() {
                let user = user_id.to_user(&ctx.http).await.unwrap();
                let name = user.name;

                tmp.push_str(&format!("{}: {:?}\n", name, role));
            }

            tmp
        };
        mod_channel.say(&ctx.http, msg).await.unwrap();
    }

    Ok((participants, mod_channel, role_channel))
}
