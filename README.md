# Werewolf-Bot
A custom Werewolf-Discord-Bot

## Running
### Environment-Variables
* `BOT_TOKEN`: The Discord Bot Token to use
### Docker
Running the latest Version of the Bot:
`docker run -e BOT_TOKEN={your bot token} ghcr.io/lol3rrr/waswolf:master`

## Permissions
268446800
## Scope
bot

## Changes
* Moderator based off the Roles instead of reactions
* Easier addition and modification of roles
* Change Channel-Names for the Bot

* Szenarios: A Set of Roles to use for a Game
* Manage Roles using a Chat
* Manage "Szenarios" using Chat

### Testing
Add automated End-to-End testing by writing a small second bot which will execute
the Commands and all that and check for the correct Reactions by the actual Bot
