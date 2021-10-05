use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serenity::model::channel::{Message, ReactionType};

mod cfg_reactions;

mod roles_msg;
pub use roles_msg::get_roles_msg;

mod distribute;
pub use distribute::distribute_roles;

use crate::rounds::BotContext;

pub async fn cfg_role_msg_reactions(
    message: &Message,
    ctx: &dyn BotContext,
    roles: &[WereWolfRole],
    page: usize,
) {
    let reactions = cfg_reactions::reactions(roles, page);
    for reaction in reactions {
        if let Err(e) = message.react(ctx.get_http(), reaction).await {
            tracing::error!("Adding Reaction: {:?}", e);
        }
    }
}

/// The Config for a Custom Werewolf Role
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct WereWolfRoleConfig {
    /// The Name of the Role used for Displaying it as well as for the Channel names
    name: String,
    /// The Emoji used to select the Role itself when creating the Round and the like
    emoji: String,
    /// Whether or not this Role can be assigned to mutliple Players in a single Round, notable
    /// examples of this would be the "Werewolf" Role itself
    mutli_player: bool,
    /// Whether or not this Role "masks" another Role, meaning that it also needs one more Role
    /// which will also be assigned to the Player and will be used by the Player at some Point in
    /// the Game
    masks_role: bool,
}

impl Display for WereWolfRoleConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}({}) - Multiple Players: {} - Contains another Role: {}",
            self.name, self.emoji, self.mutli_player, self.masks_role
        )
    }
}

impl WereWolfRoleConfig {
    pub fn new(name: String, emoji: String, mutli_player: bool, masks_role: bool) -> Self {
        Self {
            name,
            emoji,
            mutli_player,
            masks_role,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn emoji(&self) -> &str {
        &self.emoji
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum WereWolfRole {
    Werwolf,
    Amor,
    Gerber,
    HarterBursche,
    Hexe,
    Hure,
    Leibwächter,
    Seher,
    AlteVettel,
    AlterMann,
    Aussätzige,
    Beschwörerin,
    Brandstifter,
    DoppelGängerin,
    Geist,
    Händler,
    Jäger,
    Lynkanthrophin,
    Metzger,
    ParanormalerErmittler,
    Prinz,
    Priester,
    Trunkenbold(Option<Box<WereWolfRole>>),
    Russe,
    SeherLehrling,
    Strolch,
    UnruheStifter,
    Verfluchter,
    Zaubermeisterin,
    FreiMaurer,
    Vampire,
    EinsamerWolf,
    WeißerWolf,
}

impl Display for WereWolfRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Werwolf => write!(f, "Werwolf"),
            Self::Amor => write!(f, "Amor"),
            Self::Gerber => write!(f, "Gerber"),
            Self::HarterBursche => write!(f, "HarterBursche"),
            Self::Hexe => write!(f, "Hexe"),
            Self::Hure => write!(f, "Hure"),
            Self::Leibwächter => write!(f, "Leibwächter"),
            Self::Seher => write!(f, "Seher"),
            Self::AlteVettel => write!(f, "AlteVettel"),
            Self::AlterMann => write!(f, "AlterMann"),
            Self::Aussätzige => write!(f, "Aussätzige"),
            Self::Beschwörerin => write!(f, "Beschwörerin"),
            Self::Brandstifter => write!(f, "Brandstifter"),
            Self::DoppelGängerin => write!(f, "DoppelGängerin"),
            Self::Geist => write!(f, "Geist"),
            Self::Händler => write!(f, "Händler"),
            Self::Jäger => write!(f, "Jäger"),
            Self::Lynkanthrophin => write!(f, "Lynkanthrophin"),
            Self::Metzger => write!(f, "Metzger"),
            Self::ParanormalerErmittler => write!(f, "ParanormalerErmittler"),
            Self::Prinz => write!(f, "Prinz"),
            Self::Priester => write!(f, "Priester"),
            Self::Trunkenbold(_) => write!(f, "Trunkenbold"),
            Self::Russe => write!(f, "Russe"),
            Self::SeherLehrling => write!(f, "SeherLehrling"),
            Self::Strolch => write!(f, "Strolch"),
            Self::UnruheStifter => write!(f, "UnruheStifter"),
            Self::Verfluchter => write!(f, "Verfluchter"),
            Self::Zaubermeisterin => write!(f, "Zaubermeisterin"),
            Self::FreiMaurer => write!(f, "Freimaurer"),
            Self::Vampire => write!(f, "Vampire"),
            Self::EinsamerWolf => write!(f, "Einsamer Wolf"),
            Self::WeißerWolf => write!(f, "Weißer Wolf"),
        }
    }
}

impl WereWolfRole {
    pub fn all_roles() -> Vec<WereWolfRole> {
        vec![
            Self::Werwolf,
            Self::Amor,
            Self::Gerber,
            Self::HarterBursche,
            Self::Hexe,
            Self::Hure,
            Self::Leibwächter,
            Self::Seher,
            Self::AlteVettel,
            Self::AlterMann,
            Self::Aussätzige,
            Self::Beschwörerin,
            Self::Brandstifter,
            Self::DoppelGängerin,
            Self::Geist,
            Self::Händler,
            Self::Jäger,
            Self::Lynkanthrophin,
            Self::Metzger,
            Self::ParanormalerErmittler,
            Self::Prinz,
            Self::Priester,
            Self::Trunkenbold(None),
            Self::Russe,
            Self::SeherLehrling,
            Self::Strolch,
            Self::UnruheStifter,
            Self::Verfluchter,
            Self::Zaubermeisterin,
            Self::FreiMaurer,
            Self::Vampire,
            Self::EinsamerWolf,
            Self::WeißerWolf,
        ]
    }

    pub fn needs_multiple(&self) -> bool {
        matches!(self, Self::Werwolf | Self::FreiMaurer | Self::Vampire)
    }

    pub fn needs_other_role(&self) -> bool {
        matches!(self, Self::Trunkenbold(None))
    }

    pub const fn to_emoji(&self) -> char {
        match self {
            Self::Werwolf => '🐺',
            Self::Amor => '💛',
            Self::Gerber => '🇬',
            Self::HarterBursche => '💪',
            Self::Hexe => '🧙',
            Self::Hure => '🏩',
            Self::Leibwächter => '🥷',
            Self::Seher => '🔮',
            Self::AlteVettel => '👵',
            Self::AlterMann => '👴',
            Self::Aussätzige => '🇦',
            Self::Beschwörerin => '🤫',
            Self::Brandstifter => '🔥',
            Self::DoppelGängerin => '👯',
            Self::Geist => '👻',
            Self::Händler => '💰',
            Self::Jäger => '🏹',
            Self::Lynkanthrophin => '🐈',
            Self::Metzger => '🔪',
            Self::ParanormalerErmittler => '👮',
            Self::Prinz => '🤴',
            Self::Priester => '⛪',
            Self::Trunkenbold(_) => '🍺',
            Self::Russe => '🪆',
            Self::SeherLehrling => '👀',
            Self::Strolch => '🇸',
            Self::UnruheStifter => '💥',
            Self::Verfluchter => '🧟',
            Self::Zaubermeisterin => '🪄',
            Self::FreiMaurer => '🧱',
            Self::Vampire => '🧛',
            Self::EinsamerWolf => '🐻',
            Self::WeißerWolf => '🦝',
        }
    }

    pub fn from_emoji(emoji: ReactionType) -> Option<WereWolfRole> {
        let mut data = emoji.as_data();
        match data.remove(0) {
            '🐺' => Some(Self::Werwolf),
            '💛' => Some(Self::Amor),
            '🇬' => Some(Self::Gerber),
            '💪' => Some(Self::HarterBursche),
            '🧙' => Some(Self::Hexe),
            '🏩' => Some(Self::Hure),
            '🥷' => Some(Self::Leibwächter),
            '🔮' => Some(Self::Seher),
            '👵' => Some(Self::AlteVettel),
            '👴' => Some(Self::AlterMann),
            '🇦' => Some(Self::Aussätzige),
            '🤫' => Some(Self::Beschwörerin),
            '🔥' => Some(Self::Brandstifter),
            '👯' => Some(Self::DoppelGängerin),
            '👻' => Some(Self::Geist),
            '💰' => Some(Self::Händler),
            '🏹' => Some(Self::Jäger),
            '🐈' => Some(Self::Lynkanthrophin),
            '🔪' => Some(Self::Metzger),
            '👮' => Some(Self::ParanormalerErmittler),
            '🤴' => Some(Self::Prinz),
            '⛪' => Some(Self::Priester),
            '🍺' => Some(Self::Trunkenbold(None)),
            '🪆' => Some(Self::Russe),
            '👀' => Some(Self::SeherLehrling),
            '🇸' => Some(Self::Strolch),
            '💥' => Some(Self::UnruheStifter),
            '🧟' => Some(Self::Verfluchter),
            '🪄' => Some(Self::Zaubermeisterin),
            '🧱' => Some(Self::FreiMaurer),
            '🧛' => Some(Self::Vampire),
            '🐻' => Some(Self::EinsamerWolf),
            '🦝' => Some(Self::WeißerWolf),
            _ => None,
        }
    }

    pub fn channels(&self) -> Vec<String> {
        match self {
            Self::Werwolf => vec![format!("{}", self)],
            Self::Amor => vec![format!("{}", self)],
            Self::Gerber => vec![format!("{}", self)],
            Self::HarterBursche => vec![format!("{}", self)],
            Self::Hexe => vec![format!("{}", self)],
            Self::Hure => vec![format!("{}", self)],
            Self::Leibwächter => vec![format!("{}", self)],
            Self::Seher => vec![format!("{}", self)],
            Self::AlteVettel => vec![format!("{}", self)],
            Self::AlterMann => vec![format!("{}", self)],
            Self::Aussätzige => vec![format!("{}", self)],
            Self::Beschwörerin => vec![format!("{}", self)],
            Self::Brandstifter => vec![format!("{}", self)],
            Self::DoppelGängerin => vec![format!("{}", self)],
            Self::Geist => vec![format!("{}", self)],
            Self::Händler => vec![format!("{}", self)],
            Self::Jäger => vec![format!("{}", self)],
            Self::Lynkanthrophin => vec![format!("{}", self)],
            Self::Metzger => vec![format!("{}", self)],
            Self::ParanormalerErmittler => vec![format!("{}", self)],
            Self::Prinz => vec![format!("{}", self)],
            Self::Priester => vec![format!("{}", self)],
            Self::Trunkenbold(_) => vec![format!("{}", self)],
            Self::Russe => vec![format!("{}", self)],
            Self::SeherLehrling => vec![format!("{}", self)],
            Self::Strolch => vec![format!("{}", self)],
            Self::UnruheStifter => vec![format!("{}", self)],
            Self::Verfluchter => vec![format!("{}", self)],
            Self::Zaubermeisterin => vec![format!("{}", self)],
            Self::FreiMaurer => vec![format!("{}", self)],
            Self::Vampire => vec![format!("{}", self)],
            Self::EinsamerWolf => vec![format!("{}", self), format!("{}", Self::Werwolf)],
            Self::WeißerWolf => vec![format!("{}", self), format!("{}", Self::Werwolf)],
        }
    }
}
