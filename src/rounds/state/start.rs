use std::{collections::BTreeMap, error::Error, fmt::Display};

use serenity::{
    client::Context,
    model::{
        channel::{PermissionOverwrite, PermissionOverwriteType},
        id::{ChannelId, RoleId, UserId},
        Permissions,
    },
};

use crate::roles::{self, WereWolfRole};

use super::{channels, RoleCounts, RoundState};

/// Generates the Permission-Settings to allow the given User to access
/// whatever this is applied to
fn channel_access_permissions(user: UserId) -> PermissionOverwrite {
    PermissionOverwrite {
        allow: Permissions::READ_MESSAGES | Permissions::SEND_MESSAGES,
        deny: Permissions { bits: 0 },
        kind: PermissionOverwriteType::Member(user),
    }
}

#[derive(Debug, PartialEq)]
pub enum StartError {
    LoadingChannels,
    SettingUpCategory,
    SettingUpChannels,
    SettingUpModeratorChannel,
    DistributingRoles,
    AssignRolePermissions,
}

impl Display for StartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadingChannels => write!(f, "Loading Guild Channels"),
            Self::SettingUpCategory => write!(f, "Setting up Category for active Roles"),
            Self::SettingUpChannels => write!(f, "Setting up Channels for active Roles"),
            Self::SettingUpModeratorChannel => write!(f, "Setting up Channel for the Moderators"),
            Self::DistributingRoles => write!(f, "Distributing Roles to Players"),
            Self::AssignRolePermissions => {
                write!(f, "Assigning Role-Permissions to Users and Channels")
            }
        }
    }
}
impl Error for StartError {}

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
    StartError,
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

    let guild_channel = source
        .guild
        .channels(&ctx.http)
        .await
        .map_err(|_| StartError::LoadingChannels)?;

    let active_category_id = channels::setup_active_category(ctx, &source.guild, &guild_channel)
        .await
        .map_err(|_| StartError::SettingUpCategory)?;

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
    .await
    .map_err(|_| StartError::SettingUpChannels)?;

    let mod_channel = channels::setup_moderator_channel(
        default_permissions,
        source.guild,
        &guild_channel,
        &active_category_id,
        ctx,
        &source.owner,
    )
    .await
    .map_err(|_| StartError::SettingUpModeratorChannel)?;

    let participants = roles::distribute_roles(
        source.state.participants.clone(),
        source.state.roles.clone(),
    )
    .map_err(|_| StartError::DistributingRoles)?;

    // Set the Permissions for the Users and their corresponding Role-Channels
    for (user_id, role) in participants.iter() {
        let access_permissions = channel_access_permissions(user_id.clone());

        for tmp_c in role.channels() {
            let channel = role_channel
                .get(&tmp_c)
                .expect("There should be a Channel for the Role available");

            channel
                .create_permission(&ctx.http, &access_permissions)
                .await
                .map_err(|_| StartError::AssignRolePermissions)?;
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
        mod_channel
            .say(&ctx.http, info_msg)
            .await
            .map_err(|_| StartError::SettingUpModeratorChannel)?;

        let msg = {
            let mut tmp = "Roles:\n".to_string();

            for (user_id, role) in participants.iter() {
                let user = user_id
                    .to_user(&ctx.http)
                    .await
                    .map_err(|_| StartError::SettingUpModeratorChannel)?;
                let name = user.name;

                tmp.push_str(&format!("{}: {:?}\n", name, role));
            }

            tmp
        };
        mod_channel
            .say(&ctx.http, msg)
            .await
            .map_err(|_| StartError::SettingUpModeratorChannel)?;
    }

    Ok((participants, mod_channel, role_channel))
}
