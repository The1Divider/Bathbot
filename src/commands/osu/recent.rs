use crate::{
    arguments::{Args, NameArgs},
    bail,
    embeds::{EmbedData, RecentEmbed},
    pagination::{Pagination, RecentPagination},
    tracking::process_tracking,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        MessageExt,
    },
    BotResult, Context,
};

use rosu::model::{
    ApprovalStatus::{Approved, Loved, Qualified, Ranked},
    GameMode,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::time::{delay_for, Duration};
use twilight_model::channel::Message;

#[allow(clippy::cognitive_complexity)]
async fn recent_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    // Retrieve the user and their recent scores
    let user_fut = ctx.osu().user(name.as_str()).mode(mode);
    let scores_fut = ctx.osu().recent_scores(name.as_str()).mode(mode).limit(50);
    let (user, scores) = match tokio::try_join!(user_fut, scores_fut) {
        Ok((user, scores)) => {
            if scores.is_empty() {
                let content = format!(
                    "No recent {}plays found for user `{}`",
                    match mode {
                        GameMode::STD => "",
                        GameMode::TKO => "taiko ",
                        GameMode::CTB => "ctb ",
                        GameMode::MNA => "mania ",
                    },
                    name
                );
                return msg.error(&ctx, content).await;
            } else if let Some(user) = user {
                (user, scores)
            } else {
                let content = format!("User `{}` was not found", name);
                return msg.error(&ctx, content).await;
            }
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Get all relevant maps from the database
    let mut map_ids: HashSet<u32> = scores.iter().filter_map(|s| s.beatmap_id).collect();
    let mut maps = {
        let dedubed_ids: Vec<u32> = map_ids.iter().copied().collect();
        let map_result = ctx.psql().get_beatmaps(&dedubed_ids).await;
        match map_result {
            Ok(maps) => maps,
            Err(why) => {
                warn!("Error while retrieving maps from DB: {}", why);
                HashMap::default()
            }
        }
    };

    // Memoize which maps are already in the DB
    map_ids.retain(|id| maps.contains_key(&id));

    // Retrieve the first map
    let first_score = scores.first().unwrap();
    let first_id = first_score.beatmap_id.unwrap();
    #[allow(clippy::map_entry)]
    if !maps.contains_key(&first_id) {
        let map = match ctx.osu().beatmap().map_id(first_id).await {
            Ok(Some(map)) => map,
            Ok(None) => {
                let content = format!("The API returned no beatmap for map id {}", first_id);
                return msg.error(&ctx, content).await;
            }
            Err(why) => {
                let _ = msg.error(&ctx, OSU_API_ISSUE).await;
                return Err(why.into());
            }
        };
        maps.insert(first_id, map);
    }

    // Prepare retrieval of the map's global top 50 and the user's top 100
    let first_map = maps.get(&first_id).unwrap();
    let global_fut = async {
        match first_map.approval_status {
            Ranked | Loved | Qualified | Approved => {
                Some(first_map.get_global_leaderboard(ctx.osu()).limit(50).await)
            }
            _ => None,
        }
    };
    let best_fut = async {
        match first_map.approval_status {
            Ranked => Some(user.get_top_scores(ctx.osu()).limit(100).mode(mode).await),
            _ => None,
        }
    };

    // Retrieve and parse response
    let (globals_result, best_result) = tokio::join!(global_fut, best_fut);
    let mut global = HashMap::with_capacity(scores.len());
    match globals_result {
        None => {}
        Some(Ok(scores)) => {
            global.insert(first_id, scores);
        }
        Some(Err(why)) => warn!("Error while getting global scores: {}", why),
    }
    let best = match best_result {
        None => None,
        Some(Ok(scores)) => Some(scores),
        Some(Err(why)) => {
            warn!("Error while getting top scores: {}", why);
            None
        }
    };

    // Accumulate all necessary data
    let tries = scores
        .iter()
        .take_while(|s| {
            s.beatmap_id.unwrap() == first_id && s.enabled_mods == first_score.enabled_mods
        })
        .count();
    let global_scores = global.get(&first_id).map(|global| global.as_slice());
    let first_map = maps.get(&first_id).unwrap();
    let data = match RecentEmbed::new(
        &ctx,
        &user,
        first_score,
        first_map,
        best.as_deref(),
        global_scores,
    )
    .await
    {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            bail!("error while creating embed: {}", why);
        }
    };

    // Creating the embed
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .content(format!("Try #{}", tries))?
        .embed(data.build().build()?)?
        .await?;
    ctx.store_msg(response.id);

    // Process user and their top scores for tracking
    if let Some(ref scores) = best {
        process_tracking(&ctx, mode, scores, Some(&user), &mut maps).await;
    }

    // Skip pagination if too few entries
    if scores.len() <= 1 {
        response.reaction_delete(&ctx, msg.author.id);
        let msg_id = msg.id;
        tokio::spawn(async move {
            delay_for(Duration::from_secs(60)).await;
            if !ctx.remove_msg(msg_id) {
                return;
            }
            let embed_result = ctx
                .http
                .update_message(response.channel_id, response.id)
                .embed(data.minimize().build().unwrap());
            match embed_result {
                Ok(m) => {
                    if let Err(why) = m.await {
                        warn!("Error minimizing recent msg: {}", why);
                    }
                }

                Err(why) => warn!("Error while creating `recent` minimize embed: {}", why),
            }
        });
        return Ok(());
    }

    // Pagination
    let pagination = RecentPagination::new(
        Arc::clone(&ctx),
        response,
        user,
        scores,
        maps,
        best,
        global,
        map_ids,
        data,
    );
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error (recent): {}", why)
        }
    });
    Ok(())
}

#[command]
#[short_desc("Display a user's most recent play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("r", "rs")]
pub async fn recent(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's most recent mania play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rm")]
pub async fn recentmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's most recent taiko play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rt")]
pub async fn recenttaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display a user's most recent ctb play")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("rc")]
pub async fn recentctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    recent_main(GameMode::CTB, ctx, msg, args).await
}
