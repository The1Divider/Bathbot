use std::fmt::Write;

use command_macros::EmbedData;
use rosu_v2::prelude::{MostPlayedMap, User};

use crate::{
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::OSU_BASE,
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct MostPlayedEmbed {
    description: String,
    author: AuthorBuilder,
    footer: FooterBuilder,
    thumbnail: String,
    title: &'static str,
}

impl MostPlayedEmbed {
    pub fn new<'m, M>(user: &User, maps: M, pages: &Pages) -> Self
    where
        M: Iterator<Item = &'m MostPlayedMap>,
    {
        let thumbnail = user.avatar_url.to_owned();
        let mut description = String::with_capacity(10 * 70);

        for most_played in maps {
            let map = &most_played.map;
            let mapset = &most_played.mapset;

            let _ = writeln!(
                description,
                "**[{count}]** [{artist} - {title} [{version}]]({OSU_BASE}b/{map_id}) [{stars:.2}★]",
                count = most_played.count,
                title = mapset.title.cow_escape_markdown(),
                artist = mapset.artist.cow_escape_markdown(),
                version = map.version.cow_escape_markdown(),
                map_id = map.map_id,
                stars = map.stars,
            );
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        Self {
            thumbnail,
            description,
            title: "Most played maps:",
            author: author!(user),
            footer: FooterBuilder::new(format!("Page {page}/{pages}")),
        }
    }
}
