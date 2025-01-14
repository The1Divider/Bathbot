use std::fmt::Write;

use command_macros::EmbedData;

use crate::{
    commands::osu::SnipeCountryListOrder,
    custom_client::SnipeCountryPlayer,
    pagination::Pages,
    util::{
        builder::FooterBuilder,
        constants::OSU_BASE,
        numbers::{with_comma_float, with_comma_int},
        osu::flag_url,
        CountryCode, CowUtils,
    },
};

#[derive(EmbedData)]
pub struct CountrySnipeListEmbed {
    thumbnail: String,
    description: String,
    title: String,
    footer: FooterBuilder,
}

impl CountrySnipeListEmbed {
    pub fn new<'i, S>(
        country: Option<&(String, CountryCode)>,
        order: SnipeCountryListOrder,
        players: S,
        author_idx: Option<usize>,
        pages: &Pages,
    ) -> Self
    where
        S: Iterator<Item = &'i (usize, SnipeCountryPlayer)>,
    {
        let order_text = match order {
            SnipeCountryListOrder::Count => "#1 count",
            SnipeCountryListOrder::Pp => "average pp of #1s",
            SnipeCountryListOrder::Stars => "average stars of #1s",
            SnipeCountryListOrder::WeightedPp => "weighted pp from #1s",
        };

        let (title, thumbnail) = match country {
            Some((country, code)) => {
                let title = format!(
                    "{country}{} #1 list, sorted by {order_text}",
                    if country.ends_with('s') { "'" } else { "'s" },
                );

                let thumbnail = flag_url(code.as_str());

                (title, thumbnail)
            }
            None => (
                format!("Global #1 statistics, sorted by {order_text}"),
                String::new(),
            ),
        };

        let mut description = String::with_capacity(512);

        for (idx, player) in players {
            let _ = writeln!(
                description,
                "**{idx}. [{name}]({OSU_BASE}users/{id})**: {w}Weighted pp: {weighted}{w}\n\
                {c}Count: {count}{c} ~ {p}Avg pp: {pp}{p} ~ {s}Avg stars: {stars:.2}★{s}",
                name = player.username.cow_escape_markdown(),
                id = player.user_id,
                c = if order == SnipeCountryListOrder::Count {
                    "__"
                } else {
                    ""
                },
                p = if order == SnipeCountryListOrder::Pp {
                    "__"
                } else {
                    ""
                },
                s = if order == SnipeCountryListOrder::Stars {
                    "__"
                } else {
                    ""
                },
                w = if order == SnipeCountryListOrder::WeightedPp {
                    "__"
                } else {
                    ""
                },
                count = with_comma_int(player.count_first),
                pp = with_comma_float(player.avg_pp),
                stars = player.avg_sr,
                weighted = with_comma_float(player.pp),
            );
        }

        description.pop();

        let page = pages.curr_page();
        let pages = pages.last_page();
        let mut footer_text = format!("Page {page}/{pages}");

        if let Some(idx) = author_idx {
            let _ = write!(footer_text, " ~ Your position: {}", idx + 1);
        }

        Self {
            description,
            title,
            thumbnail,
            footer: FooterBuilder::new(footer_text),
        }
    }
}
