use std::collections::{BTreeMap, BTreeSet, HashMap};

use serenity::{
    http::Http,
    model::{
        channel::{ChannelType, GuildChannel, PermissionOverwrite, PermissionOverwriteType},
        id::{ChannelId, GuildId, UserId},
        Permissions,
    },
};

use crate::roles::WereWolfRole;

use super::BotContext;

#[derive(Debug)]
pub enum GetChannelError {
    UpdatingPermissions,
    CreatingChannel(serenity::Error),
}

/// Attempts to get a Channel from a Guild, by either reusing an already
/// existing one or creating a new one.
/// Either way the given Permissions are applied to the Channel.
async fn get_channel(
    channel_name: &str,
    ctx: &dyn BotContext,
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
                id.create_permission(ctx.get_http(), permission)
                    .await
                    .map_err(|_| GetChannelError::UpdatingPermissions)?;
            }

            *id
        }
        None => {
            guild_id
                .create_channel(ctx.get_http(), |c| {
                    c.name(channel_name)
                        .kind(ChannelType::Text)
                        .permissions(default_permissions.to_vec())
                })
                .await
                .map_err(GetChannelError::CreatingChannel)?
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
    ctx_http: &Http,
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
        Some((id, _)) => *id,
        None => {
            let category = guild
                .create_channel(ctx_http, |c| c.name(name).kind(ChannelType::Category))
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
    ctx: &dyn BotContext,
    guild: &GuildId,
    guild_channel: &HashMap<ChannelId, GuildChannel>,
) -> Result<ChannelId, GetCategoryError> {
    get_category(
        &ACTIVE_CATEGORY_NAME.to_lowercase(),
        ctx.get_http(),
        guild,
        guild_channel,
    )
    .await
}
pub async fn setup_inactive_category(
    ctx: &dyn BotContext,
    guild: &GuildId,
    guild_channel: &HashMap<ChannelId, GuildChannel>,
) -> Result<ChannelId, GetCategoryError> {
    get_category(
        &INACTIVE_CATEGORY_NAME.to_lowercase(),
        ctx.get_http(),
        guild,
        guild_channel,
    )
    .await
}

#[derive(Debug)]
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

async fn setup_channel<I>(
    name: &str,
    guild: &GuildId,
    guild_channel: &HashMap<ChannelId, GuildChannel>,
    category_id: ChannelId,
    default_permissions: &[PermissionOverwrite],
    extra_users: I,
    ctx: &dyn BotContext,
) -> Result<ChannelId, SetupChannelError>
where
    I: Iterator<Item = UserId>,
{
    let lowercase_name = name.to_lowercase();

    let channel_id = get_channel(
        &lowercase_name,
        ctx,
        guild,
        guild_channel,
        default_permissions,
    )
    .await?;

    channel_id
        .edit(ctx.get_http(), |c| c.category(category_id))
        .await
        .map_err(|_| SetupChannelError::MoveChannel)?;

    for user in extra_users {
        let access_permissions = channel_access_permissions(user);
        channel_id
            .create_permission(ctx.get_http(), &access_permissions)
            .await
            .map_err(|_| SetupChannelError::UpdatingChannelPermissions)?;
    }

    Ok(channel_id)
}

pub async fn setup_role_channels(
    roles: impl Iterator<Item = &WereWolfRole>,
    default_permissions: Vec<PermissionOverwrite>,
    guild: GuildId,
    guild_channel: &HashMap<ChannelId, GuildChannel>,
    category_id: &ChannelId,
    ctx: &dyn BotContext,
    moderators: &BTreeSet<UserId>,
) -> Result<BTreeMap<String, ChannelId>, SetupChannelError> {
    let mut role_channel: BTreeMap<String, ChannelId> = BTreeMap::new();

    for role in roles {
        let channel_name = format!("{}", role).to_lowercase();

        let channel_id = setup_channel(
            &channel_name,
            &guild,
            guild_channel,
            *category_id,
            &default_permissions,
            moderators.iter().map(|id| *id),
            ctx,
        )
        .await?;

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
    ctx: &dyn BotContext,
    moderators: &BTreeSet<UserId>,
) -> Result<ChannelId, SetupChannelError> {
    setup_channel(
        &MOD_CHANNEL_NAME,
        &guild,
        guild_channel,
        *category_id,
        &default_permissions,
        moderators.iter().map(|id| *id),
        ctx,
    )
    .await
}
