use super::request_user;
use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, OsuStatsCountsEmbed},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu_v2::model::GameMode;
use std::sync::Arc;
use twilight_model::channel::Message;

async fn osustats_main(
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

    let user = match request_user(&ctx, &name, Some(mode)).await {
        Ok(user) => user,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let counts = match super::get_globals_count(&ctx, &user.username, mode).await {
        Ok(counts) => counts,
        Err(why) => {
            let content = "Some issue with the osustats website, blame bade";
            let _ = msg.error(&ctx, content).await;

            return Err(why);
        }
    };

    let data = OsuStatsCountsEmbed::new(user, mode, counts);
    let embed = data.build_owned().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;

    Ok(())
}

#[command]
#[short_desc("Count how often a user appears on top of a map's leaderboard")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `osu` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("osc", "osustatscounts")]
pub async fn osustatscount(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Count how often a user appears on top of a mania map's leaderboard")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `mania` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("oscm", "osustatscountsmania")]
pub async fn osustatscountmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Count how often a user appears on top of a taiko map's leaderboard")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `taiko` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("osct", "osustatscountstaiko")]
pub async fn osustatscounttaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Count how often a user appears on top of a ctb map's leaderboard")]
#[long_desc(
    "Display in how many top 1-50 map leaderboards the user has a score.\n\
    This command shows the same stats as the globals count section for the \
    `ctb` command.\n\
    Check https://osustats.ppy.sh/ for more info."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("oscc", "osustatscountsctb")]
pub async fn osustatscountctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    osustats_main(GameMode::CTB, ctx, msg, args).await
}
