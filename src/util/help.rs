use serenity::{builder::CreateMessage, utils::Color};

/// This is used to generate the Help message for the Bot itself
pub fn generate_help_message(m: &mut CreateMessage) {
    m.embed(|e| {
        e.title("Commands")
            .field("werewolf", "Start a new Werewolf-Round", false)
            .color(Color::from_rgb(130, 10, 10))
    });
}
