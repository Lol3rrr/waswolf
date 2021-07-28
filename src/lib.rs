use std::collections::HashMap;

use async_trait::async_trait;
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
    prelude::{Mutex, TypeMapKey},
    utils::Color,
    Client,
};

mod roles;
mod rounds;
use rounds::{Round, RoundsMap};

mod reactions;
pub use reactions::Reactions;

struct Rounds;
impl TypeMapKey for Rounds {
    type Value = RoundsMap;
}

struct RoleCount;
impl TypeMapKey for RoleCount {
    type Value = Mutex<HashMap<MessageId, GuildId>>;
}

struct Handler {
    id: UserId,
}

const PREFIX: &str = "/";

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
        let user_id = add_reaction
            .user_id
            .expect("A Reaction should always contain a User-ID");
        if user_id == self.id {
            return;
        }

        let data = ctx.data.read().await;
        let rounds = data
            .get::<Rounds>()
            .expect("The shared Rounds-Datastructure should always exist in a running Instance");
        let round_mutex = match rounds.get_from_reaction(&add_reaction) {
            Some(r) => r,
            None => return,
        };

        let mut round = round_mutex.lock().await;
        match round.handle_add_react(&ctx, add_reaction.clone()).await {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Handling Reaction for Round: {}", e);
                let error_msg = format!("Error handling Round: {}", e);
                round.update_msg(&ctx, &error_msg).await;

                drop(round);
                drop(rounds);
                drop(data);
                let mut data = ctx.data.write().await;
                let rounds = data.get_mut::<Rounds>().expect(
                    "The shared Rounds-Datastructure should always exist in a running Instance",
                );
                rounds.remove_from_reaction(&add_reaction);

                return;
            }
        };

        if !round.is_done() {
            return;
        }

        drop(round);
        drop(rounds);
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
        let user_id = removed_reaction
            .user_id
            .expect("A Reaction should always contain a User-ID");
        if user_id == self.id {
            return;
        }

        let data = ctx.data.read().await;
        let rounds = data
            .get::<Rounds>()
            .expect("The shared Rounds-Datastructure should always exist in a running Instance");
        match rounds.get_from_reaction(&removed_reaction) {
            Some(round_mutex) => {
                let mut round = round_mutex.lock().await;
                round.handle_remove_react(&ctx, removed_reaction).await;
                return;
            }
            None => {}
        };
    }

    async fn message(&self, ctx: Context, new_message: Message) {
        let ref_message = match &new_message.referenced_message {
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

        let rounds = data
            .get::<Rounds>()
            .expect("The shared Rounds-Datastructure should always exist in a running Instance");
        match rounds.get(&round_id) {
            Some(round_mutex) => {
                let mut round = round_mutex.lock().await;
                if let Err(e) = round.role_reply(&ctx, reply_id, new_message).await {
                    tracing::error!("{:?}", e);

                    {
                        let mut data = ctx.data.write().await;
                        let rounds = data.get_mut::<Rounds>().expect("The shared Rounds-Datastructure should always exist in a running Instance");
                        rounds.remove(&round_id);
                    }
                }
                return;
            }
            None => {}
        };
    }

    async fn guild_member_update(
        &self,
        ctx: Context,
        _old_if_available: Option<serenity::model::guild::Member>,
        new: serenity::model::guild::Member,
    ) {
        let data = ctx.data.read().await;
        let rounds = data
            .get::<Rounds>()
            .expect("The general Rounds-Map should always exist");
        match rounds.get(&new.guild_id) {
            Some(round_mutex) => {
                let mut round = round_mutex.lock().await;
                round.handle_member_update(&ctx, new).await;
                return;
            }
            None => {}
        };
    }
}

#[group]
#[commands(help, werewolf)]
struct General;

#[command]
async fn werewolf(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
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

    let entry_msg = format!("Creating new Round.\nReact with:\n{}: Enter as a Player\n{}: Enter as a Moderator\n{}: Start the Round itself", Reactions::Entry, Reactions::ModEntry, Reactions::Confirm);

    let result = match channel_id
        .send_message(&ctx.http, |m| {
            m.content(entry_msg).reactions(&[
                Reactions::Entry,
                Reactions::ModEntry,
                Reactions::Confirm,
            ])
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
        Mutex::new(Round::new(
            msg.author.id,
            msg_id,
            result.channel_id,
            guild_id,
        )),
    );

    Ok(())
}

#[command]
async fn help(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    tracing::debug!("Help Command");

    if let Err(e) = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title("Commands")
                    .field("werewolf", "Starts a new Werewolf-Round", false)
                    .color(Color::from_rgb(130, 10, 10))
            })
        })
        .await
    {
        tracing::error!("Sending Help-Message: {:?}", e);
    }

    Ok(())
}

/// Actually starts the Bot itself
pub async fn start(token: String) {
    tracing::info!("Starting Bot...");

    let framework = StandardFramework::new()
        .configure(|c| c.with_whitespace(false).prefix(PREFIX))
        .group(&GENERAL_GROUP);

    let http = Http::new_with_token(&token);
    let bot_id = {
        let user = http.get_current_user().await.unwrap();
        user.id
    };

    let mut client = Client::builder(token)
        .event_handler(Handler { id: bot_id })
        .framework(framework)
        .intents(
            GatewayIntents::GUILD_MEMBERS
                | GatewayIntents::GUILDS
                | GatewayIntents::GUILD_MESSAGES
                | GatewayIntents::GUILD_MESSAGE_REACTIONS,
        )
        .await
        .unwrap();

    {
        let mut c_data = client.data.write().await;
        c_data.insert::<Rounds>(RoundsMap::new());
        c_data.insert::<RoleCount>(Mutex::new(HashMap::default()));
    }

    if let Err(e) = client.start().await {
        tracing::error!("Listening for Events: {:?}", e);
    }
}
