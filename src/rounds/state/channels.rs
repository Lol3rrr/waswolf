use std::collections::{BTreeMap, BTreeSet, HashMap};

use serenity::{
    client::Context,
    model::{
        channel::{ChannelType, GuildChannel, PermissionOverwrite, PermissionOverwriteType},
        id::{ChannelId, GuildId, UserId},
        Permissions,
    },
};

use crate::roles::WereWolfRole;

#[derive(Debug, PartialEq)]
pub enum GetChannelError {
    UpdatingPermissions,
    CreatingChannel,
}

/// Attempts to get a Channel from a Guild, by either reusing an already
/// existing one or creating a new one.
/// Either way the given Permissions are applied to the Channel.
async fn get_channel(
    channel_name: &str,
    ctx: &Context,
    guild_id: &GuildId,
    guild_channel: &HashMap<ChannelId, GuildChannel>,
    default_permissions: &[PermissionOverwrite],
) -> Result<ChannelId, GetChannelError> {
    let guild_channel_id_result = guild_channel
        .iter()
        .find(|(_, channel)| channel.name == channel_name);
    let id = match guild_channel_id_result {
        Some((id, _)) => {
            // Deny everyone access to the channel
            for permission in default_permissions.iter() {
                id.create_permission(&ctx.http, permission)
                    .await
                    .map_err(|_| GetChannelError::UpdatingPermissions)?;
            }

            id.clone()
        }
        None => {
            guild_id
                .create_channel(&ctx.http, |c| {
                    c.name(channel_name)
                        .kind(ChannelType::Text)
                        .permissions(default_permissions.to_vec())
                })
                .await
                .map_err(|_| GetChannelError::CreatingChannel)?
                .id
        }
    };
    Ok(id)
}

fn channel_access_permissions(user: UserId) -> PermissionOverwrite {
    PermissionOverwrite {
        allow: Permissions::READ_MESSAGES | Permissions::SEND_MESSAGES,
        deny: Permissions { bits: 0 },
        kind: PermissionOverwriteType::Member(user),
    }
}

#[derive(Debug, PartialEq)]
pub enum GetCategoryError {
    CreatingCategory,
}

/// Gets or creates a Category with the given Name
async fn get_category(
    name: &str,
    ctx: &Context,
    guild: &GuildId,
    guild_channel: &HashMap<ChannelId, GuildChannel>,
) -> Result<ChannelId, GetCategoryError> {
    let guild_channel_id_result = guild_channel
        .iter()
        .find(|(_, channel)| match channel.kind {
            ChannelType::Category => channel.name == name,
            _ => false,
        });

    let id = match guild_channel_id_result {
        Some((id, _)) => id.clone(),
        None => {
            let category = guild
                .create_channel(&ctx.http, |c| c.name(name).kind(ChannelType::Category))
                .await
                .map_err(|_| GetCategoryError::CreatingCategory)?;
            category.id
        }
    };
    Ok(id)
}

const ACTIVE_CATEGORY_NAME: &str = "W-Active";
const INACTIVE_CATEGORY_NAME: &str = "W-Inactive";

pub async fn setup_active_category(
    ctx: &Context,
    guild: &GuildId,
    guild_channel: &HashMap<ChannelId, GuildChannel>,
) -> Result<ChannelId, GetCategoryError> {
    get_category(
        &ACTIVE_CATEGORY_NAME.to_lowercase(),
        ctx,
        guild,
        guild_channel,
    )
    .await
}
pub async fn setup_inactive_category(
    ctx: &Context,
    guild: &GuildId,
    guild_channel: &HashMap<ChannelId, GuildChannel>,
) -> Result<ChannelId, GetCategoryError> {
    get_category(
        &INACTIVE_CATEGORY_NAME.to_lowercase(),
        ctx,
        guild,
        guild_channel,
    )
    .await
}

#[derive(Debug, PartialEq)]
pub enum SetupChannelError {
    GetChannel(GetChannelError),
    MoveChannel,
    UpdatingChannelPermissions,
}

impl From<GetChannelError> for SetupChannelError {
    fn from(e: GetChannelError) -> Self {
        Self::GetChannel(e)
    }
}

pub async fn setup_role_channels(
    roles: impl Iterator<Item = &WereWolfRole>,
    default_permissions: Vec<PermissionOverwrite>,
    guild: GuildId,
    guild_channel: &HashMap<ChannelId, GuildChannel>,
    category_id: &ChannelId,
    ctx: &Context,
    moderators: &BTreeSet<UserId>,
) -> Result<BTreeMap<String, ChannelId>, SetupChannelError> {
    let mut role_channel: BTreeMap<String, ChannelId> = BTreeMap::new();

    for role in roles {
        let channel_name = format!("{}", role).to_lowercase();

        let channel_id = get_channel(
            &channel_name,
            ctx,
            &guild,
            &guild_channel,
            &default_permissions,
        )
        .await?;

        channel_id
            .edit(&ctx.http, |c| c.category(*category_id))
            .await
            .map_err(|_| SetupChannelError::MoveChannel)?;

        // Give the Moderator access to the Channel
        for moderator in moderators.iter() {
            channel_id
                .create_permission(&ctx.http, &channel_access_permissions(moderator.clone()))
                .await
                .map_err(|_| SetupChannelError::UpdatingChannelPermissions)?;
        }

        role_channel.insert(format!("{}", role), channel_id);
    }

    Ok(role_channel)
}

const MOD_CHANNEL_NAME: &str = "Moderator";

pub async fn setup_moderator_channel(
    default_permissions: Vec<PermissionOverwrite>,
    guild: GuildId,
    guild_channel: &HashMap<ChannelId, GuildChannel>,
    category_id: &ChannelId,
    ctx: &Context,
    moderators: &BTreeSet<UserId>,
) -> Result<ChannelId, SetupChannelError> {
    let channel_id = get_channel(
        &MOD_CHANNEL_NAME.to_lowercase(),
        ctx,
        &guild,
        guild_channel,
        &default_permissions,
    )
    .await?;

    channel_id
        .edit(&ctx.http, |c| c.category(*category_id))
        .await
        .map_err(|_| SetupChannelError::MoveChannel)?;

    for moderator in moderators.iter() {
        let access_permissions = channel_access_permissions(moderator.clone());
        channel_id
            .create_permission(&ctx.http, &access_permissions)
            .await
            .map_err(|_| SetupChannelError::UpdatingChannelPermissions)?;
    }

    Ok(channel_id)
}
