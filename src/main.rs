use std::env;

use werewolf_bot::start;

fn main() {
    let token = env::var("BOT_TOKEN").expect("Needs a Discord-Bot-Token to operate");

    // Setting up the logging/tracing stuff
    let tracing_directive_str =
        env::var("RUST_LOG").unwrap_or_else(|_| "werewolf_bot=info".to_owned());
    let tracing_sub = tracing_subscriber::FmtSubscriber::builder()
        .with_level(true)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing_directive_str.parse().unwrap()),
        )
        .finish();
    tracing::subscriber::set_global_default(tracing_sub)
        .expect("Setting initial Tracing-Subscriber");

    // Setting up the Tokio-Runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    // Actually running the Bot
    runtime.block_on(start(token));
}
