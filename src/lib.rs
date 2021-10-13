use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use lazy_static::lazy_static;
use messages::{AsyncTransition, MessageStateMachine, TransitionResult};
use serenity::{
    client::{bridge::gateway::GatewayIntents, Context, EventHandler},
    framework::standard::{
        macros::{command, group},
        Args, CommandResult, StandardFramework,
    },
    http::Http,
    model::{
        channel::Message,
        id::{GuildId, MessageId, UserId},
        prelude::Activity,
    },
    prelude::{Mutex, TypeMap, TypeMapKey},
    Client,
};

pub const MOD_ROLE_NAME: &str = "Game Master";
/// The Name of the Role used for Dead-Players
pub const DEAD_ROLE_NAME: &str = "W-Dead";

lazy_static! {
    static ref SMMap: lockfree::map::Map<MessageId, Mutex<MessageStateMachine<(), ()>>> =
        lockfree::map::Map::new();
    static ref NOTIFY_SM_QUEUE: notifier::NotifyQueue = notifier::NotifyQueue::new();
}

mod notifier;

mod roles;
mod rounds;

mod reactions;
pub use reactions::Reactions;
use storage::Storage;

mod util;

mod storage;

mod commands;

pub mod metrics;

pub mod messages;

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

    ready_metric: prometheus::IntGauge,
}

impl Handler {
    pub fn new(id: UserId, registry: &prometheus::Registry) -> Self {
        let ready_metric = prometheus::IntGauge::with_opts(prometheus::Opts::new(
            "ready",
            "Whether or not the Bot is ready",
        ))
        .unwrap();
        ready_metric.set(0);

        registry.register(Box::new(ready_metric.clone())).unwrap();

        Self { id, ready_metric }
    }

    async fn update_sm(
        guild_id: GuildId,
        message_id: MessageId,
        http: &Arc<Http>,
        storage: &Storage,
        event: messages::Event,
    ) {
        let sm_mutex = match SMMap.get(&message_id) {
            Some(s) => s,
            None => return,
        };

        let context = messages::Context::new(
            Some(http.clone()),
            Some(event),
            Some(storage.clone()),
            guild_id,
        );

        let mut sm = sm_mutex.val().lock().await;
        match sm.transition(context, ()).await.as_ref() {
            TransitionResult::NoTransition => {
                tracing::debug!("No Transition occured");
            }
            TransitionResult::Done(_) => {
                tracing::debug!("StateMachine is done");
            }
            TransitionResult::Error(e) => {
                tracing::error!("Transitioning: {:?}", e);
            }
        };
    }
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

        self.ready_metric.set(1);

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

        let data = ctx.data.read().await;
        let storage = data.get::<BotStorage>().unwrap();
        Self::update_sm(
            add_reaction.guild_id.unwrap(),
            add_reaction.message_id,
            &ctx.http,
            storage,
            messages::Event::AddReaction {
                reaction: add_reaction.clone(),
            },
        )
        .await;
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
        let storage = data.get::<BotStorage>().unwrap();

        Self::update_sm(
            removed_reaction.guild_id.unwrap(),
            removed_reaction.message_id,
            &ctx.http,
            storage,
            messages::Event::RemoveReaction {
                reaction: removed_reaction.clone(),
            },
        )
        .await;
    }

    #[tracing::instrument(skip(self, ctx, new_message))]
    async fn message(&self, ctx: Context, new_message: Message) {
        let ref_message = match &new_message.referenced_message {
            Some(m) => m.clone(),
            None => return,
        };
        let reply_id = ref_message.id;

        let data = ctx.data.read().await;
        let storage = data.get::<BotStorage>().unwrap();

        Self::update_sm(
            new_message.guild_id.unwrap(),
            reply_id,
            &ctx.http,
            storage,
            messages::Event::Reply {
                message: new_message.clone(),
            },
        )
        .await;
    }

    async fn guild_member_update(
        &self,
        _ctx: Context,
        _old_if_available: Option<serenity::model::guild::Member>,
        _new: serenity::model::guild::Member,
    ) {
    }
}

#[group]
#[commands(help, werewolf, add_role, remove_role, list_roles)]
struct General;

#[command]
async fn werewolf(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    commands::werewolf(ctx, msg).await
}

#[command]
async fn help(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    commands::help(ctx, msg).await
}

#[command]
#[aliases("list-roles")]
async fn list_roles(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    commands::list_roles(ctx, msg).await
}

#[command]
#[aliases("add-role")]
async fn add_role(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    commands::add_role(ctx, msg, args).await
}

#[command]
#[aliases("remove-role")]
async fn remove_role(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    commands::remove_role(ctx, msg, args).await
}

/// Initialize the Client instance with all the needed Data
/// to function properly
async fn init_bot_data(client: &Client, http: Arc<Http>, bot_storage: storage::Storage) {
    notifier::run_notifier(http, bot_storage.clone()).await;

    let mut c_data = client.data.write().await;
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

    let discord_storage = storage::discord::DiscordStorage::new(http.clone());
    let bot_storage = storage::Storage::new(discord_storage);

    let handler = Handler::new(bot_id, &metrics::REGISTRY);

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
    init_bot_data(&client, http, bot_storage).await;

    // Actually run the Bot
    if let Err(e) = client.start().await {
        tracing::error!("Listening for Events: {:?}", e);
    }
}
