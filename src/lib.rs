use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use serenity::{
    client::{bridge::gateway::GatewayIntents, Context, EventHandler},
    framework::standard::{
        macros::{command, group},
        Args, CommandResult, StandardFramework,
    },
    http::{CacheHttp, Http},
    model::{
        channel::Message,
        id::{GuildId, MessageId, UserId},
        prelude::Activity,
    },
    prelude::{Mutex, TypeMap, TypeMapKey},
    Client,
};

mod roles;
mod rounds;
use rounds::{Round, RoundsMap};

mod reactions;
pub use reactions::Reactions;

use crate::{roles::WereWolfRoleConfig, rounds::BotContext};

mod util;

mod storage;

struct Rounds;
impl TypeMapKey for Rounds {
    type Value = RoundsMap;
}

struct RoleCount;
impl TypeMapKey for RoleCount {
    type Value = Mutex<HashMap<MessageId, GuildId>>;
}

struct BotStorage;
impl TypeMapKey for BotStorage {
    type Value = storage::Storage;
}

/// The general Handler for the Bot
struct Handler {
    /// The UserID of the Bot itself
    id: UserId,
}

impl Handler {
    pub fn new(id: UserId) -> Self {
        Self { id }
    }
}

fn get_rounds(map: &TypeMap) -> &RoundsMap {
    map.get::<Rounds>()
        .expect("The shared Rounds Datastructure should always exist on a running Bot-Instance")
}

fn get_storage(map: &TypeMap) -> &storage::Storage {
    map.get::<BotStorage>()
        .expect("The Shared Storage Backend should always exist on a running Bot-Instance")
}

/// The Bot-Prefix used for recognizing Commands
#[cfg(not(debug_assertions))]
const PREFIX: &str = "/";
#[cfg(debug_assertions)]
const PREFIX: &str = "!";

#[async_trait]
impl EventHandler for Handler {
    async fn ready(
        &self,
        ctx: serenity::client::Context,
        _data_about_bot: serenity::model::prelude::Ready,
    ) {
        ctx.set_activity(Activity::listening(PREFIX)).await;

        tracing::info!("Bot is ready");
    }

    #[tracing::instrument(skip(self, ctx, add_reaction))]
    async fn reaction_add(&self, ctx: Context, add_reaction: serenity::model::channel::Reaction) {
        // If the Reaction came from the Bot, we will simply ignore it and return early
        match add_reaction.user_id {
            Some(id) if id == self.id => {
                return;
            }
            Some(_) => {}
            None => {
                tracing::error!("A Reaction should always contain a User-ID");
                return;
            }
        };

        // Get access to the Round itself for the current Guild
        let data = ctx.data.read().await;
        let rounds = get_rounds(&data);
        let round_mutex = match rounds.get_from_reaction(&add_reaction) {
            Some(r) => r,
            None => return,
        };

        let mut round = round_mutex.lock().await;
        if let Err(e) = round
            .handle_add_react(self.id, &ctx, add_reaction.clone())
            .await
        {
            tracing::error!("Handling Reaction for Round: {}", e);
            let error_msg = format!("Error handling Round: {}", e);
            round.update_msg(&ctx, &error_msg).await;

            drop(round);
            drop(data);
            let mut data = ctx.data.write().await;
            let rounds = data.get_mut::<Rounds>().expect(
                "The shared Rounds-Datastructure should always exist in a running Instance",
            );
            rounds.remove_from_reaction(&add_reaction);

            return;
        }

        // If the Round is already marked as being done we should simply return early as there is
        // nothing more for us to do
        if !round.is_done() {
            return;
        }

        drop(round);
        drop(data);
        let mut data = ctx.data.write().await;
        let rounds = data
            .get_mut::<Rounds>()
            .expect("The shared Rounds-Datastructure should always exist in a running Instance");
        rounds.remove_from_reaction(&add_reaction);
    }

    async fn reaction_remove(
        &self,
        ctx: Context,
        removed_reaction: serenity::model::channel::Reaction,
    ) {
        // Check the User-ID of the Reaction and return early if there is none or if the Reaction
        // came from the bot itself
        match removed_reaction.user_id {
            Some(id) if id == self.id => {
                return;
            }
            Some(_) => {}
            None => {
                tracing::error!("A removed Reaction should always contain a UserID");
                return;
            }
        };

        let data = ctx.data.read().await;
        let rounds = get_rounds(&data);

        let round_mutex = match rounds.get_from_reaction(&removed_reaction) {
            Some(r) => r,
            None => return,
        };

        let mut round = round_mutex.lock().await;
        round.handle_remove_react(&ctx, removed_reaction).await;
    }

    async fn message(&self, ctx: Context, new_message: Message) {
        let mut ref_message = match &new_message.referenced_message {
            Some(m) => m.clone(),
            None => return,
        };
        let reply_id = ref_message.id;

        let data = ctx.data.read().await;
        let role_count = data.get::<RoleCount>().expect("The shared Role-Count-Messages Datastructure should always exist in a running Instance");
        let mut role_count = role_count.lock().await;

        let round_id = match role_count.remove(&reply_id) {
            Some(r) => r,
            None => return,
        };

        let rounds = get_rounds(&data);

        let round_mutex = match rounds.get(&round_id) {
            Some(r) => r,
            None => return,
        };

        let mut round = round_mutex.lock().await;
        if let Err(e) = round.role_reply(self.id, &ctx, reply_id, new_message).await {
            tracing::error!("{:?}", e);

            if let Err(e) = ref_message
                .edit(&ctx.http, |edit| {
                    edit.content("Error setting up the Moderator Channel")
                })
                .await
            {
                tracing::error!("Updating Message with Error: {:?}", e);
            };

            {
                let mut data = ctx.data.write().await;
                let rounds = data.get_mut::<Rounds>().expect(
                    "The shared Rounds-Datastructure should always exist in a running Instance",
                );
                rounds.remove(&round_id);
            }
        }
    }

    async fn guild_member_update(
        &self,
        ctx: Context,
        _old_if_available: Option<serenity::model::guild::Member>,
        new: serenity::model::guild::Member,
    ) {
        let data = ctx.data.read().await;
        let rounds = get_rounds(&data);

        if let Some(round_mutex) = rounds.get(&new.guild_id) {
            let mut round = round_mutex.lock().await;
            round.handle_member_update(&ctx, new).await;
            return;
        }
    }
}

#[group]
#[commands(help, werewolf, add_role, remove_role, list_roles)]
struct General;

#[command]
async fn werewolf(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    tracing::debug!("Received werewolf command");

    let guild_id = match msg.guild_id {
        Some(gid) => gid,
        None => return Ok(()),
    };
    let channel_id = msg.channel_id;

    let mut data = ctx.data.write().await;
    let rounds = data
        .get_mut::<Rounds>()
        .expect("The shared Rounds-Datastructure should always exist in a running Instance");
    if rounds.get(&guild_id).is_some() {
        tracing::error!("There is already a Round running on the Guild");
        if let Err(e) = channel_id
            .say(
                &ctx.http,
                "There exists already a running Round in this Guild",
            )
            .await
        {
            tracing::error!("Sending Error-Message {:?}", e);
        }

        return Ok(());
    }

    tracing::info!("Starting new Round");

    const MOD_ROLE_NAME: &str = "Game Master";
    let mod_role = match util::roles::find_role(MOD_ROLE_NAME, guild_id, ctx.get_http()).await {
        Ok(r) => r,
        Err(util::roles::FindRoleError::NotFound) => {
            tracing::error!("'Game Master'-Role does not exist on the Guild");
            if let Err(e) = channel_id
                .send_message(ctx.get_http(), |m| {
                    m.content("Could not start a new Round as it could not find a Role with the Name 'Game Master'")
                })
                .await
            {
                tracing::error!("Sending Discord error message: {:?}", e);
            }

            return Ok(());
        }
        Err(e) => {
            tracing::error!("Error getting 'Game Master'-Role for Guild: {:?}", e);
            return Ok(());
        }
    };
    let mods = util::roles::role_users(mod_role, guild_id, ctx.get_http()).await;

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
        Mutex::new(Round::new(mods, msg_id, result.channel_id, guild_id).await),
    );

    Ok(())
}

#[command]
async fn help(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    tracing::debug!("Received help Command");

    if let Err(e) = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            util::help::generate_help_message(m);
            m
        })
        .await
    {
        tracing::error!("Sending Help-Message: {:?}", e);
    }

    Ok(())
}

#[command]
#[aliases("list-roles")]
async fn list_roles(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
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

#[command]
#[aliases("add-role")]
async fn add_role(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    tracing::debug!("Received add-role Command");

    let channel_id = msg.channel_id;
    let guild_id = msg.guild_id.unwrap();

    let name = "test-role".to_string();
    let emoji = "test-emoji".to_string();
    let mutli_player = false;
    let masks_role = false;
    let new_config = WereWolfRoleConfig::new(name, emoji, mutli_player, masks_role);

    let data = ctx.data.read().await;
    let storage = get_storage(&data);

    // TODO
    // Check if the Role already exists

    match storage.backend().set_role(guild_id, new_config).await {
        Ok(_) => {
            tracing::debug!("Created new Role");

            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| m.content("Successfully added the Role"))
                .await
            {
                tracing::error!("Sending Confirmation message: {:?}", e);
            }
        }
        Err(e) => {
            tracing::error!("Setting Role: {:?}", e);

            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| m.content("Could not add the Role"))
                .await
            {
                tracing::error!("Sending Error Message: {:?}", e);
            }
        }
    };

    Ok(())
}

#[command]
#[aliases("remove-role")]
async fn remove_role(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    tracing::debug!("Received remove-role Command");

    let channel_id = msg.channel_id;
    let guild_id = msg.guild_id.unwrap();

    let role_name = match args.current() {
        Some(r) => r,
        None => {
            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| {
                    m.content("Must supply the Name of the Role to remove")
                })
                .await
            {
                tracing::error!("Sending invalid Args Message: {:?}", e);
            }

            return Ok(());
        }
    };

    let data = ctx.data.read().await;
    let storage = get_storage(&data);

    match storage.backend().remove_role(guild_id, role_name).await {
        Ok(_) => {
            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| {
                    m.content(format!("Removed Role \"{}\"", role_name))
                })
                .await
            {
                tracing::error!("Sending Confirmation message: {:?}", e);
            }
        }
        Err(e) => {
            tracing::error!("Removing Role: {:?}", e);

            if let Err(e) = channel_id
                .send_message(ctx.http(), |m| {
                    m.content(format!("Could not remove Role \"{}\"", role_name))
                })
                .await
            {
                tracing::error!("Sending Error message: {:?}", e);
            }
        }
    };

    Ok(())
}

/// Initialize the Client instance with all the needed Data
/// to function properly
async fn init_bot_data(client: &Client, bot_storage: storage::Storage) {
    let mut c_data = client.data.write().await;
    c_data.insert::<Rounds>(RoundsMap::new());
    c_data.insert::<RoleCount>(Mutex::new(HashMap::default()));
    c_data.insert::<BotStorage>(bot_storage);
}

/// Actually starts the Bot itself
pub async fn start(token: String) {
    tracing::info!("Starting Bot...");

    // Setup the general Framework for the Discord-Bot instance
    let framework = StandardFramework::new()
        .configure(|c| c.with_whitespace(false).prefix(PREFIX))
        .group(&GENERAL_GROUP);

    // Create the HTTP-Instance for the Bot to use
    let http = Arc::new(Http::new_with_token(&token));
    let bot_id = {
        let user = http.get_current_user().await.unwrap();
        user.id
    };

    let discord_storage = storage::discord::DiscordStorage::new(http);
    let bot_storage = storage::Storage::new(discord_storage);

    let handler = Handler::new(bot_id);

    // Actually create the Bot instance with all the needed Settings/Configs
    let mut client = Client::builder(token)
        .event_handler(handler)
        .framework(framework)
        .intents(
            GatewayIntents::GUILD_MEMBERS
                | GatewayIntents::GUILDS
                | GatewayIntents::GUILD_MESSAGES
                | GatewayIntents::GUILD_MESSAGE_REACTIONS,
        )
        .await
        .unwrap();

    // Initialize the Bots inner State
    init_bot_data(&client, bot_storage).await;

    // Actually run the Bot
    if let Err(e) = client.start().await {
        tracing::error!("Listening for Events: {:?}", e);
    }
}
