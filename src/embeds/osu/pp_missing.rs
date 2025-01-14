use std::{cmp::Ordering, iter};

use rosu_v2::prelude::{Score, User};
use twilight_model::channel::embed::Embed;

use crate::{
    embeds::EmbedData,
    util::{
        builder::{AuthorBuilder, EmbedBuilder, FooterBuilder},
        numbers::{with_comma_float, with_comma_int},
        osu::{approx_more_pp, pp_missing, ExtractablePp, PpListUtil},
        CowUtils,
    },
};

pub struct PPMissingEmbed {
    author: AuthorBuilder,
    description: String,
    footer: Option<FooterBuilder>,
    thumbnail: String,
    title: String,
}

impl PPMissingEmbed {
    pub fn new(
        user: User,
        scores: &mut [Score],
        goal_pp: f32,
        rank: Option<u32>,
        each: Option<f32>,
    ) -> Self {
        let stats_pp = user.statistics.as_ref().unwrap().pp;

        let title = format!(
            "What scores is {name} missing to reach {goal_pp}pp?",
            name = user.username.cow_escape_markdown(),
            goal_pp = with_comma_float(goal_pp),
        );

        let description = match (scores.last().and_then(|s| s.pp), each) {
            // No top scores
            (None, _) => "No top scores found".to_owned(),
            // Total pp already above goal
            _ if stats_pp > goal_pp => format!(
                "{name} has {pp_raw}pp which is already more than {pp_given}pp.",
                name = user.username.cow_escape_markdown(),
                pp_raw = with_comma_float(stats_pp),
                pp_given = with_comma_float(goal_pp),
            ),
            // Reach goal with only one score
            (Some(_), None) => {
                let (required, idx) = if scores.len() == 100 {
                    let mut pps = scores.extract_pp();
                    approx_more_pp(&mut pps, 50);

                    pp_missing(stats_pp, goal_pp, pps.as_slice())
                } else {
                    pp_missing(stats_pp, goal_pp, &*scores)
                };

                format!(
                    "To reach {pp}pp with one additional score, {user} needs to perform \
                    a **{required}pp** score which would be the top {approx}#{idx}",
                    pp = with_comma_float(goal_pp),
                    user = user.username.cow_escape_markdown(),
                    required = with_comma_float(required),
                    approx = if idx >= 100 { "~" } else { "" },
                    idx = idx + 1,
                )
            }
            // Given score pp is below last top 100 score pp
            (Some(last_pp), Some(each)) if each < last_pp => {
                format!(
                    "New top100 scores require at least **{last_pp}pp** for {user} \
                    so {pp} total pp can't be reached with {each}pp scores.",
                    pp = with_comma_float(goal_pp),
                    last_pp = with_comma_float(last_pp),
                    each = with_comma_float(each),
                    user = user.username.cow_escape_markdown(),
                )
            }
            // Given score pp would be in top 100
            (Some(_), Some(each)) => {
                let mut pps = scores.extract_pp();

                let (required, idx) = if scores.len() == 100 {
                    approx_more_pp(&mut pps, 50);

                    pp_missing(stats_pp, goal_pp, pps.as_slice())
                } else {
                    pp_missing(stats_pp, goal_pp, &*scores)
                };

                if required < each {
                    format!(
                        "To reach {pp}pp with one additional score, {user} needs to perform \
                        a **{required}pp** score which would be the top {approx}#{idx}",
                        pp = with_comma_float(goal_pp),
                        user = user.username.cow_escape_markdown(),
                        required = with_comma_float(required),
                        approx = if idx >= 100 { "~" } else { "" },
                        idx = idx + 1,
                    )
                } else {
                    let idx = pps.iter().position(|&pp| pp < each).unwrap_or(scores.len());

                    let mut iter = pps
                        .iter()
                        .copied()
                        .zip(0..)
                        .map(|(pp, i)| pp * 0.95_f32.powi(i));

                    let mut top: f32 = (&mut iter).take(idx).sum();
                    let bot: f32 = iter.sum();

                    let bonus_pp = (stats_pp - (top + bot)).max(0.0);
                    top += bonus_pp;
                    let len = pps.len();

                    let mut n_each = len;

                    for i in idx..len {
                        let bot = pps[idx..]
                            .iter()
                            .copied()
                            .zip(i as i32 + 1..)
                            .fold(0.0, |sum, (pp, i)| sum + pp * 0.95_f32.powi(i));

                        let factor = 0.95_f32.powi(i as i32);

                        if top + factor * each + bot >= goal_pp {
                            // requires n_each many new scores of `each` many pp and one additional score
                            n_each = i - idx;
                            break;
                        }

                        top += factor * each;
                    }

                    if n_each == len {
                        format!(
                            "Filling up {user}'{genitiv} top scores with {amount} new {each}pp score{plural} \
                            would only lead to {approx}**{top}pp** which is still less than {pp}pp.",
                            amount = len - idx,
                            each = with_comma_float(each),
                            plural = if len - idx != 1 { "s" } else { "" },
                            genitiv = if idx != 1 { "s" } else { "" },
                            pp = with_comma_float(goal_pp),
                            approx = if idx >= 100 { "roughly " } else { "" },
                            top = with_comma_float(top),
                            user = user.username.cow_escape_markdown(),
                        )
                    } else {
                        pps.extend(iter::repeat(each).take(n_each));
                        pps.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

                        let accum = pps.accum_weighted();

                        // Calculate the pp of the missing score after adding `n_each` many `each` pp scores
                        let total = accum + bonus_pp;
                        let (required, _) = pp_missing(total, goal_pp, pps.as_slice());

                        format!(
                            "To reach {pp}pp, {user} needs to perform **{n_each}** more \
                            {each}pp score{plural} and one **{required}pp** score.",
                            each = with_comma_float(each),
                            plural = if n_each != 1 { "s" } else { "" },
                            pp = with_comma_float(goal_pp),
                            user = user.username.cow_escape_markdown(),
                            required = with_comma_float(required),
                        )
                    }
                }
            }
        };

        let footer = rank.map(|rank| {
            FooterBuilder::new(format!(
                "The current rank for {pp}pp is approx. #{rank}",
                pp = with_comma_float(goal_pp),
                rank = with_comma_int(rank),
            ))
        });

        Self {
            author: author!(user),
            description,
            footer,
            thumbnail: user.avatar_url,
            title,
        }
    }
}

impl EmbedData for PPMissingEmbed {
    fn build(self) -> Embed {
        let builder = EmbedBuilder::new()
            .author(self.author)
            .description(self.description)
            .thumbnail(self.thumbnail)
            .title(self.title);

        if let Some(footer) = self.footer {
            builder.footer(footer).build()
        } else {
            builder.build()
        }
    }
}
