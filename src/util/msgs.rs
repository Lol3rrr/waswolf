use serenity::{http::Http, model::id::ChannelId};

/// This will send a message with the given Content in the given Channel and if an error
/// occures output it via tracing on the error level
pub async fn send_content(channel_id: ChannelId, http: &Http, content: &str) {
    if let Err(e) = channel_id.send_message(http, |m| m.content(content)).await {
        tracing::error!("Sending Message: {:?}", e);
    }
}
