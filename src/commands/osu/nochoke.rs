use std::{borrow::Cow, cmp::Ordering, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::{Report, Result, WrapErr};
use rosu_pp::{Beatmap as Map, CatchPP, CatchStars, OsuPP, TaikoPP};
use rosu_v2::prelude::{GameMode, OsuError, Score};
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::osu::{get_user_and_scores, ScoreArgs, UserArgs},
    core::commands::{prefix::Args, CommandOrigin},
    pagination::NoChokePagination,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        interaction::InteractionCommand,
        matcher,
        osu::prepare_beatmap_file,
        InteractionCommandExt, ScoreExt,
    },
    Context,
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "nochoke",
    help = "Remove all misses from top scores and make them full combos.\n\
    Then after recalculating their pp, check how many total pp a user could have had."
)]
/// How the top plays would look like with only full combos
pub struct Nochoke<'a> {
    #[command(help = "Specify a gamemode. \
        Since combo does not matter in mania, its scores can't be unchoked.")]
    /// Specify a gamemode
    mode: Option<NochokeGameMode>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(min_value = 0)]
    /// Only unchoke scores with at most this many misses
    miss_limit: Option<u32>,
    #[command(help = "Specify a version to unchoke scores.\n\
        - `Unchoke`: Make the score a full combo and transfer all misses to different hitresults. (default)\n\
        - `Perfect`: Make the score a full combo and transfer all misses to the best hitresults.")]
    /// Specify a version to unchoke scores
    version: Option<NochokeVersion>,
    /// Filter out certain scores
    filter: Option<NochokeFilter>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum NochokeGameMode {
    #[option(name = "osu", value = "osu")]
    Osu,
    #[option(name = "taiko", value = "taiko")]
    Taiko,
    #[option(name = "ctb", value = "ctb")]
    Catch,
}

impl From<NochokeGameMode> for GameMode {
    fn from(mode: NochokeGameMode) -> Self {
        match mode {
            NochokeGameMode::Osu => Self::Osu,
            NochokeGameMode::Taiko => Self::Taiko,
            NochokeGameMode::Catch => Self::Catch,
        }
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum NochokeVersion {
    #[option(name = "Unchoke", value = "unchoke")]
    Unchoke,
    #[option(name = "Perfect", value = "perfect")]
    Perfect,
}

impl Default for NochokeVersion {
    fn default() -> Self {
        Self::Unchoke
    }
}

impl NochokeVersion {
    async fn calculate(
        self,
        ctx: &Context,
        scores: Vec<Score>,
        miss_limit: Option<u32>,
    ) -> Result<Vec<(usize, Score, Score)>> {
        match self {
            NochokeVersion::Perfect => perfect_scores(ctx, scores, miss_limit).await,
            NochokeVersion::Unchoke => unchoke_scores(ctx, scores, miss_limit).await,
        }
    }
}

#[derive(CommandOption, CreateOption)]
pub enum NochokeFilter {
    #[option(name = "Only keep chokes", value = "only_chokes")]
    OnlyChokes,
    #[option(name = "Remove all chokes", value = "remove_chokes")]
    RemoveChokes,
}

impl<'m> Nochoke<'m> {
    fn args(mode: Option<NochokeGameMode>, args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;
        let mut miss_limit = None;

        for arg in args.take(2) {
            if let Ok(num) = arg.parse() {
                miss_limit = Some(num);
            } else if let Some(id) = matcher::get_mention_user(arg) {
                discord = Some(id);
            } else {
                name = Some(arg.into());
            }
        }

        Self {
            mode,
            name,
            miss_limit,
            version: None,
            filter: None,
            discord,
        }
    }
}

#[command]
#[desc("Unchoke a user's top100")]
#[help(
    "Display a user's top plays if no score in their top100 would be a choke.\n
    If a number is specified, I will only unchoke scores with at most that many misses"
)]
#[usage("[username] [number for miss limit]")]
#[examples("badewanne3", "vaxei 5")]
#[aliases("nc", "nochoke")]
#[group(Osu)]
async fn prefix_nochokes(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Nochoke::args(None, args);

    nochoke(ctx, msg.into(), args).await
}

#[command]
#[desc("Unchoke a user's taiko top100")]
#[help(
    "Display a user's top plays if no score in their top100 would be a choke.\n\
    If a number is specified, I will only unchoke scores with at most that many misses.\n\
    Note: As for all commands, numbers for scores on converted maps are wack and \
    are ignored when unchoking."
)]
#[usage("[username] [number for miss limit]")]
#[examples("badewanne3", "vaxei 5")]
#[alias("nct", "nochoketaiko")]
#[group(Taiko)]
async fn prefix_nochokestaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Nochoke::args(Some(NochokeGameMode::Taiko), args);

    nochoke(ctx, msg.into(), args).await
}

#[command]
#[desc("Unchoke a user's ctb top100")]
#[help(
    "Display a user's top plays if no score in their top100 would be a choke.\n\
    If a number is specified, I will only unchoke scores with at most that many misses.\n\
    Note: As for all commands, numbers for scores on converted maps are wack and \
    are ignored when unchoking."
)]
#[usage("[username] [number for miss limit]")]
#[examples("badewanne3", "vaxei 5")]
#[alias("ncc", "nochokectb", "nochokecatch", "nochokescatch")]
#[group(Catch)]
async fn prefix_nochokesctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let args = Nochoke::args(Some(NochokeGameMode::Catch), args);

    nochoke(ctx, msg.into(), args).await
}

async fn slash_nochoke(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Nochoke::from_interaction(command.input_data())?;

    nochoke(ctx, (&mut command).into(), args).await
}

async fn nochoke(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Nochoke<'_>) -> Result<()> {
    let (name, mut mode) = name_mode!(ctx, orig, args);

    if mode == GameMode::Mania {
        mode = GameMode::Osu;
    }

    let Nochoke {
        miss_limit,
        version,
        filter,
        ..
    } = args;

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(name.as_str(), mode);
    let score_args = ScoreArgs::top(100).with_combo();

    #[allow(unused_mut)]
    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user or scores");

            return Err(report);
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    #[cfg(feature = "osutracking")]
    crate::tracking::process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    let version = version.unwrap_or_default();

    let mut scores_data = match version.calculate(&ctx, scores, miss_limit).await {
        Ok(scores_data) => scores_data,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to calculate version"));
        }
    };

    // Calculate bonus pp
    let actual_pp: f32 = scores_data
        .iter()
        .filter_map(|(_, s, ..)| s.weight)
        .map(|weight| weight.pp)
        .sum();

    let bonus_pp = user.statistics.as_ref().unwrap().pp - actual_pp;

    // Sort by unchoked pp
    scores_data.sort_unstable_by(|(_, _, s1), (_, _, s2)| {
        s2.pp.partial_cmp(&s1.pp).unwrap_or(Ordering::Equal)
    });

    // Calculate total user pp without chokes
    let mut unchoked_pp: f32 = scores_data
        .iter()
        .enumerate()
        .map(|(i, (_, _, s))| s.pp.unwrap_or(0.0) * 0.95_f32.powi(i as i32))
        .sum();

    unchoked_pp = (100.0 * (unchoked_pp + bonus_pp)).round() / 100.0;

    match filter {
        Some(NochokeFilter::OnlyChokes) => scores_data.retain(|(_, a, b)| a != b),
        Some(NochokeFilter::RemoveChokes) => scores_data.retain(|(_, a, b)| a == b),
        None => {}
    }

    let rank = match ctx.psql().approx_rank_from_pp(unchoked_pp, mode).await {
        Ok(rank) => Some(rank),
        Err(err) => {
            warn!("{:?}", err.wrap_err("Failed to get rank pp"));

            None
        }
    };

    let mut content = format!(
        "{version} top {mode}scores for `{name}`",
        version = match version {
            NochokeVersion::Perfect => "Perfect",
            NochokeVersion::Unchoke => "No-choke",
        },
        mode = match mode {
            GameMode::Osu => "",
            GameMode::Taiko => "taiko ",
            GameMode::Catch => "ctb ",
            GameMode::Mania => panic!("can not unchoke mania scores"),
        },
    );

    match filter {
        Some(NochokeFilter::OnlyChokes) => content.push_str(" (only chokes)"),
        Some(NochokeFilter::RemoveChokes) => content.push_str(" (removed chokes)"),
        None => {}
    }

    content.push(':');

    NoChokePagination::builder(user, scores_data, unchoked_pp, rank)
        .content(content)
        .start_by_update()
        .defer_components()
        .start(ctx, orig)
        .await
}

async fn unchoke_scores(
    ctx: &Context,
    scores: Vec<Score>,
    miss_limit: Option<u32>,
) -> Result<Vec<(usize, Score, Score)>> {
    let mut scores_data = Vec::with_capacity(scores.len());

    for (score, i) in scores.into_iter().zip(1..) {
        let map = score.map.as_ref().unwrap();
        let mut unchoked = score.clone();

        let many_misses = miss_limit
            .filter(|&limit| score.statistics.count_miss > limit)
            .is_some();

        // Skip unchoking because it has too many misses or because its a convert
        if many_misses || map.convert {
            scores_data.push((i, score, unchoked));
            continue;
        }

        let map_path = prepare_beatmap_file(ctx, map.map_id)
            .await
            .wrap_err("failed to prepare map")?;

        let rosu_map = Map::from_path(map_path)
            .await
            .wrap_err("failed to parse map")?;

        let mods = score.mods.bits();
        let max_combo = map.max_combo.unwrap_or(0);

        if !score.is_fc(map.mode, max_combo) {
            match map.mode {
                GameMode::Osu => {
                    let total_objects = map.count_objects() as usize;

                    let mut count300 = score.statistics.count_300 as usize;

                    let count_hits = total_objects - score.statistics.count_miss as usize;
                    let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                    let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

                    count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
                    let count100 = (score.statistics.count_100 + new100s) as usize;
                    let count50 = score.statistics.count_50 as usize;

                    let pp_result = OsuPP::new(&rosu_map)
                        .mods(mods)
                        .n300(count300)
                        .n100(count100)
                        .n50(count50)
                        .calculate();

                    unchoked.statistics.count_300 = count300 as u32;
                    unchoked.statistics.count_100 = count100 as u32;
                    unchoked.max_combo = max_combo;
                    unchoked.statistics.count_miss = 0;
                    unchoked.pp = Some(pp_result.pp as f32);
                    unchoked.grade = unchoked.grade(None);
                    unchoked.accuracy = unchoked.accuracy();
                    unchoked.score = 0; // distinguishing from original
                }
                GameMode::Catch => {
                    let attributes = CatchStars::new(&rosu_map).mods(mods).calculate();

                    let total_objects = attributes.max_combo();
                    let passed_objects = (score.statistics.count_300
                        + score.statistics.count_100
                        + score.statistics.count_miss)
                        as usize;

                    let missing = total_objects.saturating_sub(passed_objects);
                    let missing_fruits = missing.saturating_sub(
                        attributes
                            .n_droplets
                            .saturating_sub(score.statistics.count_100 as usize),
                    );
                    let missing_droplets = missing - missing_fruits;

                    let n_fruits = score.statistics.count_300 as usize + missing_fruits;
                    let n_droplets = score.statistics.count_100 as usize + missing_droplets;
                    let n_tiny_droplet_misses = score.statistics.count_katu as usize;
                    let n_tiny_droplets = score.statistics.count_50 as usize;

                    let pp_result = CatchPP::new(&rosu_map)
                        .attributes(attributes)
                        .mods(mods)
                        .fruits(n_fruits)
                        .droplets(n_droplets)
                        .tiny_droplets(n_tiny_droplets)
                        .tiny_droplet_misses(n_tiny_droplet_misses)
                        .calculate();

                    let hits = n_fruits + n_droplets + n_tiny_droplets;
                    let total = hits + n_tiny_droplet_misses;

                    let acc = if total == 0 {
                        0.0
                    } else {
                        100.0 * hits as f32 / total as f32
                    };

                    unchoked.statistics.count_300 = n_fruits as u32;
                    unchoked.statistics.count_katu = n_tiny_droplet_misses as u32;
                    unchoked.statistics.count_100 = n_droplets as u32;
                    unchoked.statistics.count_50 = n_tiny_droplets as u32;
                    unchoked.max_combo = total_objects as u32;
                    unchoked.statistics.count_miss = 0;
                    unchoked.pp = Some(pp_result.pp as f32);
                    unchoked.grade = unchoked.grade(Some(acc));
                    unchoked.accuracy = unchoked.accuracy();
                    unchoked.score = 0; // distinguishing from original
                }
                GameMode::Taiko => {
                    let total_objects = map.count_circles as usize;
                    let passed_objects = score.total_hits() as usize;

                    let mut count300 = score.statistics.count_300 as usize
                        + total_objects.saturating_sub(passed_objects);

                    let count_hits = total_objects - score.statistics.count_miss as usize;
                    let ratio = 1.0 - (count300 as f32 / count_hits as f32);
                    let new100s = (ratio * score.statistics.count_miss as f32).ceil() as u32;

                    count300 += score.statistics.count_miss.saturating_sub(new100s) as usize;
                    let count100 = (score.statistics.count_100 + new100s) as usize;

                    let acc = 100.0 * (2 * count300 + count100) as f32 / (2 * total_objects) as f32;

                    let pp_result = TaikoPP::new(&rosu_map)
                        .mods(mods)
                        .accuracy(acc as f64)
                        .calculate();

                    unchoked.statistics.count_300 = count300 as u32;
                    unchoked.statistics.count_100 = count100 as u32;
                    unchoked.statistics.count_miss = 0;
                    unchoked.max_combo = map.count_circles;
                    unchoked.pp = Some(pp_result.pp as f32);
                    unchoked.grade = unchoked.grade(Some(acc));
                    unchoked.accuracy = unchoked.accuracy();
                    unchoked.score = 0; // distinguishing from original
                }
                GameMode::Mania => panic!("can not unchoke mania scores"),
            }
        }

        scores_data.push((i, score, unchoked));
    }

    Ok(scores_data)
}

async fn perfect_scores(
    ctx: &Context,
    scores: Vec<Score>,
    miss_limit: Option<u32>,
) -> Result<Vec<(usize, Score, Score)>> {
    let mut scores_data = Vec::with_capacity(scores.len());

    for (score, i) in scores.into_iter().zip(1..) {
        let map = score.map.as_ref().unwrap();
        let mut unchoked = score.clone();

        let many_misses = miss_limit
            .filter(|&limit| score.statistics.count_miss > limit)
            .is_some();

        // Skip unchoking because it has too many misses or because its a convert
        if many_misses || map.convert {
            scores_data.push((i, score, unchoked));
            continue;
        }

        let map_path = prepare_beatmap_file(ctx, map.map_id)
            .await
            .wrap_err("failed to prepare map")?;

        let rosu_map = Map::from_path(map_path)
            .await
            .wrap_err("failed to parse map")?;

        let mods = score.mods.bits();
        let total_hits = score.total_hits();

        match map.mode {
            GameMode::Osu if score.statistics.count_300 != total_hits => {
                unchoked.statistics.count_300 = total_hits;
                unchoked.statistics.count_100 = 0;
                unchoked.statistics.count_50 = 0;
                unchoked.statistics.count_miss = 0;

                let pp_result = OsuPP::new(&rosu_map).mods(mods).calculate();

                unchoked.max_combo = map
                    .max_combo
                    .unwrap_or_else(|| pp_result.max_combo() as u32);

                unchoked.pp = Some(pp_result.pp as f32);
                unchoked.grade = unchoked.grade(Some(100.0));
                unchoked.accuracy = 100.0;
                unchoked.score = 0; // distinguishing from original
            }
            GameMode::Catch if (100.0 - score.accuracy).abs() > f32::EPSILON => {
                let pp_result = CatchPP::new(&rosu_map).mods(mods).calculate();

                unchoked.statistics.count_300 = pp_result.difficulty.n_fruits as u32;
                unchoked.statistics.count_katu = 0;
                unchoked.statistics.count_100 = pp_result.difficulty.n_droplets as u32;
                unchoked.statistics.count_50 = pp_result.difficulty.n_tiny_droplets as u32;
                unchoked.max_combo = pp_result.max_combo() as u32;
                unchoked.statistics.count_miss = 0;
                unchoked.pp = Some(pp_result.pp as f32);
                unchoked.grade = unchoked.grade(Some(100.0));
                unchoked.accuracy = 100.0;
                unchoked.score = 0; // distinguishing from original
            }
            GameMode::Taiko if score.statistics.count_miss > 0 => {
                let pp_result = TaikoPP::new(&rosu_map).mods(mods).calculate();

                unchoked.statistics.count_300 = map.count_circles;
                unchoked.statistics.count_100 = 0;
                unchoked.statistics.count_miss = 0;
                unchoked.max_combo = map.count_circles;
                unchoked.pp = Some(pp_result.pp as f32);
                unchoked.grade = unchoked.grade(Some(100.0));
                unchoked.accuracy = 100.0;
                unchoked.score = 0; // distinguishing from original
            }
            GameMode::Mania => bail!("can not unchoke mania scores"),
            _ => {} // Nothing to unchoke
        }

        scores_data.push((i, score, unchoked));
    }

    Ok(scores_data)
}
