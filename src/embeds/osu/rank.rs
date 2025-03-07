use std::{
    cmp::Ordering,
    fmt::{Display, Formatter, Result as FmtResult},
    iter,
};

use command_macros::EmbedData;
use rosu_v2::{model::score::Score, prelude::UserCompact};

use crate::{
    commands::osu::RankData,
    util::{
        builder::AuthorBuilder,
        numbers::{with_comma_float, with_comma_int},
        osu::{approx_more_pp, pp_missing, ExtractablePp, PpListUtil},
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct RankEmbed {
    description: String,
    title: String,
    thumbnail: String,
    author: AuthorBuilder,
}

impl RankEmbed {
    pub fn new(data: RankData, scores: Option<Vec<Score>>, each: Option<f32>) -> Self {
        let (title, description, user) = match data {
            RankData::Sub10k {
                user,
                rank,
                country,
                rank_holder,
            } => {
                let user_pp = user.statistics.as_ref().map_or(0.0, |stats| stats.pp);

                let rank_holder_pp = rank_holder
                    .statistics
                    .as_ref()
                    .map_or(0.0, |stats| stats.pp);

                let title = format!(
                    "How many pp is {name} missing to reach rank {country}{rank}?",
                    country = country.as_ref().map(|code| code.as_str()).unwrap_or("#"),
                    name = user.username.cow_escape_markdown(),
                );

                let description = if user.user_id == rank_holder.user_id {
                    format!(
                        "{} is already at rank #{rank}.",
                        user.username.cow_escape_markdown()
                    )
                } else if user_pp > rank_holder_pp {
                    format!(
                        "Rank {rank} is currently held by {holder_name} with \
                        **{holder_pp}pp**, so {name} is already above that with **{pp}pp**.",
                        rank = RankFormat::new(rank, country.is_none(), &rank_holder),
                        holder_name = rank_holder.username.cow_escape_markdown(),
                        holder_pp = with_comma_float(rank_holder_pp),
                        name = user.username.cow_escape_markdown(),
                        pp = with_comma_float(user_pp)
                    )
                } else if let Some(scores) = scores {
                    match (scores.last().and_then(|s| s.pp), each) {
                        (Some(last_pp), Some(each)) if each < last_pp => {
                            format!(
                                "Rank {rank} is currently held by {holder_name} with \
                                **{holder_pp}pp**, so {name} is missing **{missing}** raw pp.\n\
                                A new top100 score requires at least **{last_pp}pp** \
                                so {holder_pp} total pp can't be reached with {each}pp scores.",
                                rank = RankFormat::new(rank, country.is_none(), &rank_holder),
                                holder_name = rank_holder.username.cow_escape_markdown(),
                                holder_pp = with_comma_float(rank_holder_pp),
                                name = user.username.cow_escape_markdown(),
                                missing = with_comma_float(rank_holder_pp - user_pp),
                                last_pp = with_comma_float(last_pp),
                                each = with_comma_float(each),
                            )
                        }
                        (_, Some(each)) => {
                            let mut pps = scores.extract_pp();

                            let (required, idx) = if scores.len() == 100 {
                                approx_more_pp(&mut pps, 50);

                                pp_missing(user_pp, rank_holder_pp, pps.as_slice())
                            } else {
                                pp_missing(user_pp, rank_holder_pp, scores.as_slice())
                            };

                            if required < each {
                                format!(
                                    "Rank {rank} is currently held by {holder_name} with \
                                    **{holder_pp}pp**, so {name} is missing **{missing}** raw pp.\n\
                                    To reach {holder_pp}pp with one additional score, {name} needs to \
                                    perform a **{required}pp** score which would be the top {approx}#{idx}",
                                    rank = RankFormat::new(rank, country.is_none(), &rank_holder),
                                    holder_name = rank_holder.username.cow_escape_markdown(),
                                    holder_pp = with_comma_float(rank_holder_pp),
                                    name = user.username.cow_escape_markdown(),
                                    missing = with_comma_float(rank_holder_pp - user_pp),
                                    required = with_comma_float(required),
                                    approx = if idx >= 100 { "~" } else { "" },
                                    idx = idx + 1,
                                )
                            } else {
                                let idx = pps.iter().position(|&pp| pp < each).unwrap_or(pps.len());

                                let mut iter = pps
                                    .iter()
                                    .copied()
                                    .zip(0..)
                                    .map(|(pp, i)| pp * 0.95_f32.powi(i));

                                let mut top: f32 = (&mut iter).take(idx).sum();
                                let bot: f32 = iter.sum();

                                let bonus_pp = (user_pp - (top + bot)).max(0.0);
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

                                    if top + factor * each + bot >= rank_holder_pp {
                                        // requires n_each many new scores of `each` many pp and one additional score
                                        n_each = i - idx;
                                        break;
                                    }

                                    top += factor * each;
                                }

                                if n_each == len {
                                    format!(
                                        "Rank {rank} is currently held by {holder_name} with \
                                        **{holder_pp}pp**, so {name} is missing **{missing}** raw pp.\n\
                                        Filling up {name}'{genitiv} top scores with {amount} new \
                                        {each}pp score{plural} would only lead to {approx}**{top}pp** which \
                                        is still less than {holder_pp}pp.",
                                        rank = RankFormat::new(rank, country.is_none(), &rank_holder),
                                        holder_name = rank_holder.username.cow_escape_markdown(),
                                        holder_pp = with_comma_float(rank_holder_pp),
                                        amount = len - idx,
                                        each = with_comma_float(each),
                                        missing = with_comma_float(rank_holder_pp - user_pp),
                                        plural = if len - idx != 1 { "s" } else { "" },
                                        genitiv = if idx != 1 { "s" } else { "" },
                                        approx = if idx >= 100 { "roughly " } else { "" },
                                        top = with_comma_float(top),
                                        name = user.username.cow_escape_markdown(),
                                    )
                                } else {
                                    pps.extend(iter::repeat(each).take(n_each));

                                    pps.sort_unstable_by(|a, b| {
                                        b.partial_cmp(a).unwrap_or(Ordering::Equal)
                                    });

                                    let accum = pps.accum_weighted();

                                    // Calculate the pp of the missing score after adding `n_each` many `each` pp scores
                                    let total = accum + bonus_pp;
                                    let (required, _) =
                                        pp_missing(total, rank_holder_pp, pps.as_slice());

                                    format!(
                                        "Rank {rank} is currently held by {holder_name} with \
                                        **{holder_pp}pp**, so {name} is missing **{missing}** raw pp.\n\
                                        To reach {holder_pp}pp, {name} needs to perform **{n_each}** \
                                        more {each}pp score{plural} and one **{required}pp** score.",
                                        rank = RankFormat::new(rank, country.is_none(), &rank_holder),
                                        holder_name = rank_holder.username.cow_escape_markdown(),
                                        holder_pp = with_comma_float(rank_holder_pp),
                                        missing = with_comma_float(rank_holder_pp - user_pp),
                                        each = with_comma_float(each),
                                        plural = if n_each != 1 { "s" } else { "" },
                                        name = user.username.cow_escape_markdown(),
                                        required = with_comma_float(required),
                                    )
                                }
                            }
                        }
                        _ => {
                            let (required, idx) = if scores.len() == 100 {
                                let mut pps = scores.extract_pp();
                                approx_more_pp(&mut pps, 50);

                                pp_missing(user_pp, rank_holder_pp, pps.as_slice())
                            } else {
                                pp_missing(user_pp, rank_holder_pp, scores.as_slice())
                            };

                            format!(
                                "Rank {rank} is currently held by {holder_name} with \
                                **{holder_pp}pp**, so {name} is missing **{missing}** raw pp, achievable \
                                with a single score worth **{pp}pp** which would be the top {approx}#{idx}.",
                                rank = RankFormat::new(rank, country.is_none(), &rank_holder),
                                holder_name = rank_holder.username.cow_escape_markdown(),
                                holder_pp = with_comma_float(rank_holder_pp),
                                name = user.username.cow_escape_markdown(),
                                missing = with_comma_float(rank_holder_pp - user_pp),
                                pp = with_comma_float(required),
                                approx = if idx >= 100 { "~" } else { "" },
                                idx = idx + 1,
                            )
                        }
                    }
                } else {
                    format!(
                        "Rank {rank} is currently held by {holder_name} with \
                        **{holder_pp}pp**, so {name} is missing **{holder_pp}** raw pp, \
                        achievable with a single score worth **{holder_pp}pp**.",
                        rank = RankFormat::new(rank, country.is_none(), &rank_holder),
                        holder_name = rank_holder.username.cow_escape_markdown(),
                        holder_pp = with_comma_float(rank_holder_pp),
                        name = user.username.cow_escape_markdown(),
                    )
                };

                (title, description, user)
            }
            RankData::Over10k {
                user,
                rank,
                required_pp,
            } => {
                let user_pp = user.statistics.as_ref().unwrap().pp;

                let title = format!(
                    "How many pp is {name} missing to reach rank #{rank}?",
                    name = user.username.cow_escape_markdown(),
                    rank = with_comma_int(rank),
                );

                let description = if user_pp > required_pp {
                    format!(
                        "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                        so {name} is already above that with **{pp}pp**.",
                        rank = with_comma_int(rank),
                        required_pp = with_comma_float(required_pp),
                        name = user.username.cow_escape_markdown(),
                        pp = with_comma_float(user_pp)
                    )
                } else if let Some(scores) = scores {
                    match (scores.last().and_then(|s| s.pp), each) {
                        (Some(last_pp), Some(each)) if each < last_pp => {
                            format!(
                                "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                                so {name} is missing **{missing}** raw pp.\n\
                                A new top100 score requires at least **{last_pp}pp** \
                                so {required_pp} total pp can't be reached with {each}pp scores.",
                                required_pp = with_comma_float(required_pp),
                                name = user.username.cow_escape_markdown(),
                                missing = with_comma_float(required_pp - user_pp),
                                last_pp = with_comma_float(last_pp),
                                each = with_comma_float(each),
                            )
                        }
                        (_, Some(each)) => {
                            let mut pps = scores.extract_pp();

                            let (required, idx) = if scores.len() == 100 {
                                approx_more_pp(&mut pps, 50);

                                pp_missing(user_pp, required_pp, pps.as_slice())
                            } else {
                                pp_missing(user_pp, required_pp, scores.as_slice())
                            };

                            if required < each {
                                format!(
                                    "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                                    so {name} is missing **{missing}** raw pp.\n\
                                    To reach {required_pp}pp with one additional score, {name} needs to \
                                    perform a **{required}pp** score which would be the top {approx}#{idx}",
                                    name = user.username.cow_escape_markdown(),
                                    required_pp = with_comma_float(required_pp),
                                    missing = with_comma_float(required_pp - user_pp),
                                    required = with_comma_float(required),
                                    approx = if idx >= 100 { "~" } else { "" },
                                    idx = idx + 1,
                                )
                            } else {
                                let idx = pps.iter().position(|&pp| pp < each).unwrap_or(pps.len());

                                let mut iter = pps
                                    .iter()
                                    .copied()
                                    .zip(0..)
                                    .map(|(pp, i)| pp * 0.95_f32.powi(i));

                                let mut top: f32 = (&mut iter).take(idx).sum();
                                let bot: f32 = iter.sum();

                                let bonus_pp = (user_pp - (top + bot)).max(0.0);
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

                                    if top + factor * each + bot >= required_pp {
                                        // requires n_each many new scores of `each` many pp and one additional score
                                        n_each = i - idx;
                                        break;
                                    }

                                    top += factor * each;
                                }

                                if n_each == len {
                                    format!(
                                        "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                                        so {name} is missing **{missing}** raw pp.\n\
                                        Filling up {name}'{genitiv} top scores with {amount} new \
                                        {each}pp score{plural} would only lead to {approx}**{top}pp** which \
                                        is still less than {required_pp}pp.",
                                        required_pp = with_comma_float(required_pp),
                                        amount = len - idx,
                                        each = with_comma_float(each),
                                        missing = with_comma_float(required_pp - user_pp),
                                        plural = if len - idx != 1 { "s" } else { "" },
                                        genitiv = if idx != 1 { "s" } else { "" },
                                        approx = if idx >= 100 { "roughly " } else { "" },
                                        top = with_comma_float(top),
                                        name = user.username.cow_escape_markdown(),
                                    )
                                } else {
                                    pps.extend(iter::repeat(each).take(n_each));

                                    pps.sort_unstable_by(|a, b| {
                                        b.partial_cmp(a).unwrap_or(Ordering::Equal)
                                    });

                                    let accum = pps.accum_weighted();

                                    // Calculate the pp of the missing score after adding `n_each` many `each` pp scores
                                    let total = accum + bonus_pp;
                                    let (required, _) =
                                        pp_missing(total, required_pp, pps.as_slice());

                                    format!(
                                        "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                                        so {name} is missing **{missing}** raw pp.\n\
                                        To reach {required_pp}pp, {name} needs to perform **{n_each}** \
                                        more {each}pp score{plural} and one **{required}pp** score.",
                                        required_pp = with_comma_float(required_pp),
                                        missing = with_comma_float(required_pp - user_pp),
                                        each = with_comma_float(each),
                                        plural = if n_each != 1 { "s" } else { "" },
                                        name = user.username.cow_escape_markdown(),
                                        required = with_comma_float(required),
                                    )
                                }
                            }
                        }
                        _ => {
                            let (required, idx) = if scores.len() == 100 {
                                let mut pps = scores.extract_pp();
                                approx_more_pp(&mut pps, 50);

                                pp_missing(user_pp, required_pp, pps.as_slice())
                            } else {
                                pp_missing(user_pp, required_pp, scores.as_slice())
                            };

                            format!(
                                "Rank #{rank} currently requires approx. **{required_pp}pp**, so \
                                {name} is missing **{missing}** raw pp, achievable with a \
                                single score worth **{pp}pp** which would be the top {approx}#{idx}.",
                                rank = with_comma_int(rank),
                                required_pp = with_comma_float(required_pp),
                                name = user.username.cow_escape_markdown(),
                                missing = with_comma_float(required_pp - user_pp),
                                pp = with_comma_float(required),
                                approx = if idx >= 100 { "~" } else { "" },
                                idx = idx + 1,
                            )
                        }
                    }
                } else {
                    format!(
                        "Rank #{rank} currently requires approx. **{required_pp}pp**, \
                        so {name} is missing **{required_pp}** raw pp, \
                        achievable with a single score worth **{required_pp}pp**.",
                        rank = with_comma_int(rank),
                        required_pp = with_comma_float(required_pp),
                        name = user.username.cow_escape_markdown(),
                    )
                };

                (title, description, user)
            }
        };

        Self {
            title,
            description,
            author: author!(user),
            thumbnail: user.avatar_url,
        }
    }
}

struct RankFormat<'d> {
    rank: u32,
    global: bool,
    holder: &'d UserCompact,
}

impl<'d> RankFormat<'d> {
    fn new(rank: u32, global: bool, holder: &'d UserCompact) -> Self {
        Self {
            rank,
            global,
            holder,
        }
    }
}

impl Display for RankFormat<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        if self.global {
            return write!(f, "#{}", self.rank);
        }

        write!(f, "{}{}", self.holder.country_code, self.rank)?;

        let global_rank = self
            .holder
            .statistics
            .as_ref()
            .and_then(|stats| stats.global_rank);

        if let Some(global_rank) = global_rank {
            write!(f, " (#{global_rank})")?;
        }

        Ok(())
    }
}
