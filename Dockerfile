FROM rust:1.55 as builder

RUN USER=root cargo new --bin werewolf-bot
WORKDIR ./werewolf-bot

COPY . ./
RUN cargo build --release

RUN pwd

FROM debian:buster-slim
ARG APP=/usr/src/app

RUN apt-get update; apt-get upgrade -y; apt-get install libssl1.1 ca-certificates -y

RUN mkdir -p ${APP}

COPY --from=builder /werewolf-bot/target/release/werewolf-bot ${APP}/werewolf-bot

WORKDIR ${APP}

ENTRYPOINT ["./werewolf-bot"]
