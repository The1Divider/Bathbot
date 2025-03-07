use std::fmt::Write;

use command_macros::EmbedData;

use crate::{
    custom_client::OsuStatsPlayer,
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::{AVATAR_URL, OSU_BASE},
        numbers::with_comma_int,
        osu::flag_url,
        CountryCode, CowUtils,
    },
};

#[derive(EmbedData)]
pub struct OsuStatsListEmbed {
    author: AuthorBuilder,
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
}

impl OsuStatsListEmbed {
    pub fn new(
        players: &[OsuStatsPlayer],
        country: &Option<CountryCode>,
        first_place_id: u32,
        pages: &Pages,
    ) -> Self {
        let mut author = AuthorBuilder::new("Most global leaderboard scores");

        if let Some(country) = country {
            author = author.icon_url(flag_url(country.as_str()));
        }

        let mut description = String::with_capacity(1024);

        for (player, i) in players.iter().zip(pages.index + 1..) {
            let _ = writeln!(
                description,
                "**{i}. [{}]({OSU_BASE}users/{})**: {}",
                player.username.cow_escape_markdown(),
                player.user_id,
                with_comma_int(player.count)
            );
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        Self {
            author,
            description,
            footer: FooterBuilder::new(format!("Page {page}/{pages}")),
            thumbnail: format!("{AVATAR_URL}{first_place_id}"),
        }
    }
}
