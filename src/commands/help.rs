use serenity::{
    builder::CreateMessage, client::Context, framework::standard::CommandResult,
    model::channel::Message, utils::Color,
};

const COMMANDS: [(&str, &str); 4] = [
    ("werewolf", "Starts a new Werewolf Round"),
    (
        "add-role {name} {emoji} {multi-player} {masks role} {extra channels}",
        "Adds a new Werewolf Role based on the given Options",
    ),
    (
        "remove-role {name}",
        "Removes the Werewolf Role with the given Name again",
    ),
    ("list-roles", "Lists all the configured Werewolf Roles"),
];

fn generate_help_message(m: &mut CreateMessage) {
    m.embed(|e| {
        let mut e = e.title("Commands").color(Color::from_rgb(130, 10, 10));
        for (cmd, desc) in COMMANDS {
            e = e.field(cmd, desc, false);
        }
        e
    });
}

#[tracing::instrument(skip(ctx, msg))]
pub async fn help(ctx: &Context, msg: &Message) -> CommandResult {
    tracing::debug!("Received help Command");

    if let Err(e) = msg
        .channel_id
        .send_message(&ctx.http, |m| {
            generate_help_message(m);
            m
        })
        .await
    {
        tracing::error!("Sending Help-Message: {:?}", e);
    }

    Ok(())
}
