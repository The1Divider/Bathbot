use command_macros::EmbedData;
use twilight_model::channel::embed::EmbedField;

use crate::{
    custom_client::SnipeCountryStatistics,
    embeds::attachment,
    util::{
        builder::FooterBuilder,
        numbers::{round, with_comma_int},
        osu::flag_url,
        CountryCode, CowUtils,
    },
};

#[derive(EmbedData)]
pub struct CountrySnipeStatsEmbed {
    thumbnail: String,
    title: String,
    footer: FooterBuilder,
    image: String,
    fields: Vec<EmbedField>,
}

impl CountrySnipeStatsEmbed {
    pub fn new(country: Option<(String, CountryCode)>, statistics: SnipeCountryStatistics) -> Self {
        let mut fields = Vec::with_capacity(2);

        if let Some(top_gain) = statistics.top_gain {
            let value = format!(
                "{} ({:+})",
                top_gain.username.cow_escape_markdown(),
                top_gain.difference
            );

            fields![fields { "Most gained", value, true }];
        }

        if let Some(top_loss) = statistics.top_loss {
            let value = format!(
                "{} ({:+})",
                top_loss.username.cow_escape_markdown(),
                top_loss.difference
            );

            fields![fields { "Most losses", value, true }];
        }

        let percent = round(100.0 * statistics.unplayed_maps as f32 / statistics.total_maps as f32);

        let (title, thumbnail) = match country {
            Some((country, code)) => {
                let title = format!(
                    "{country}{} #1 statistics",
                    if country.ends_with('s') { "'" } else { "'s" }
                );

                let thumbnail = flag_url(code.as_str());

                (title, thumbnail)
            }
            None => ("Global #1 statistics".to_owned(), String::new()),
        };

        let footer = FooterBuilder::new(format!(
            "Unplayed maps: {}/{} ({percent}%)",
            with_comma_int(statistics.unplayed_maps),
            with_comma_int(statistics.total_maps),
        ));

        Self {
            fields,
            thumbnail,
            title,
            footer,
            image: attachment("stats_graph.png"),
        }
    }
}
