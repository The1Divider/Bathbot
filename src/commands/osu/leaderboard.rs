use std::{borrow::Cow, sync::Arc};

use command_macros::{command, HasMods, SlashCommand};
use eyre::{Report, Result};
use rosu_v2::error::OsuError;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::channel::{message::MessageType, Message};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    database::OsuData,
    pagination::LeaderboardPagination,
    pp::PpCalculator,
    util::{
        constants::{AVATAR_URL, GENERAL_ISSUE, OSU_API_ISSUE, OSU_WEB_ISSUE},
        interaction::InteractionCommand,
        matcher,
        osu::{MapIdType, ModSelection},
        ChannelExt, InteractionCommandExt,
    },
    Context,
};

use super::{HasMods, ModsResult};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "leaderboard")]
/// Display the global leaderboard of a map
pub struct Leaderboard<'a> {
    #[command(help = "Specify a map either by map url or map id.\n\
        If none is specified, it will search in the recent channel history \
        and pick the first map it can find.")]
    /// Specify a map url or map id
    map: Option<Cow<'a, str>>,
    #[command(
        help = "Specify mods either directly or through the explicit `+mod!` / `+mod` syntax, \
        e.g. `hdhr` or `+hdhr!`, and filter out all scores that don't match those mods."
    )]
    /// Specify mods e.g. hdhr or nm
    mods: Option<Cow<'a, str>>,
}

#[derive(HasMods)]
struct LeaderboardArgs<'a> {
    map: Option<MapIdType>,
    mods: Option<Cow<'a, str>>,
}

impl<'m> LeaderboardArgs<'m> {
    fn args(msg: &Message, args: Args<'m>) -> Result<Self, String> {
        let mut map = None;
        let mut mods = None;

        for arg in args.take(2) {
            if let Some(id) = matcher::get_osu_map_id(arg)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(arg).map(MapIdType::Set))
            {
                map = Some(id);
            } else if matcher::get_mods(arg).is_some() {
                mods = Some(arg.into());
            } else {
                let content = format!(
                    "Failed to parse `{arg}`.\n\
                    Must be either a map id, map url, or mods.",
                );

                return Err(content);
            }
        }

        let reply = msg
            .referenced_message
            .as_deref()
            .filter(|_| msg.kind == MessageType::Reply);

        if let Some(id) = reply.and_then(MapIdType::from_msg) {
            map = Some(id);
        }

        Ok(Self { map, mods })
    }
}

impl<'a> TryFrom<Leaderboard<'a>> for LeaderboardArgs<'a> {
    type Error = &'static str;

    fn try_from(args: Leaderboard<'a>) -> Result<Self, Self::Error> {
        let map = match args.map {
            Some(map) => {
                if let Some(id) = matcher::get_osu_map_id(&map)
                    .map(MapIdType::Map)
                    .or_else(|| matcher::get_osu_mapset_id(&map).map(MapIdType::Set))
                {
                    Some(id)
                } else {
                    return Err(
                        "Failed to parse map url. Be sure you specify a valid map id or url to a map.",
                    );
                }
            }
            None => None,
        };

        Ok(Self {
            map,
            mods: args.mods,
        })
    }
}

#[command]
#[desc("Display the global leaderboard of a map")]
#[help(
    "Display the global leaderboard of a given map.\n\
    If no map is given, I will choose the last map \
    I can find in the embeds of this channel.\n\
    Mods can be specified."
)]
#[usage("[map url / map id] [mods]")]
#[example("2240404", "https://osu.ppy.sh/beatmapsets/902425#osu/2240404")]
#[alias("lb")]
#[group(AllModes)]
async fn prefix_leaderboard(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    match LeaderboardArgs::args(msg, args) {
        Ok(args) => leaderboard(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_leaderboard(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Leaderboard::from_interaction(command.input_data())?;

    match LeaderboardArgs::try_from(args) {
        Ok(args) => leaderboard(ctx, (&mut command).into(), args).await,
        Err(content) => {
            command.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn leaderboard(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: LeaderboardArgs<'_>,
) -> Result<()> {
    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
            If you want included mods, specify it e.g. as `+hrdt`.\n\
            If you want exact mods, specify it e.g. as `+hdhr!`.\n\
            And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            return orig.error(&ctx, content).await;
        }
    };

    let owner = orig.user_id()?;

    let map_id = match args.map {
        Some(MapIdType::Map(id)) => id,
        Some(MapIdType::Set(_)) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";

            return orig.error(&ctx, content).await;
        }
        None => {
            let msgs = match ctx.retrieve_channel_history(orig.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            };

            match MapIdType::map_from_msgs(&msgs, 0) {
                Some(id) => id,
                None => {
                    let content = "No beatmap specified and none found in recent channel history. \
                        Try specifying a map either by url to the map, or just by map id.";

                    return orig.error(&ctx, content).await;
                }
            }
        }
    };

    let author_name = match ctx.psql().get_user_osu(owner).await {
        Ok(osu) => osu.map(OsuData::into_username),
        Err(err) => {
            warn!("{:?}", err.wrap_err("failed to get username"));

            None
        }
    };

    // Retrieving the beatmap
    let mut map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                // Add map to database if its not in already
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", err.wrap_err("Failed to insert map in database"));
                }

                ctx.map_garbage_collector(&map).execute(&ctx);

                map
            }
            Err(OsuError::NotFound) => {
                let content = format!(
                    "Could not find beatmap with id `{map_id}`. \
                    Did you give me a mapset id instead of a map id?",
                );

                return orig.error(&ctx, content).await;
            }
            Err(err) => {
                let _ = orig.error(&ctx, OSU_API_ISSUE).await;
                let report = Report::new(err).wrap_err("failed to get beatmap");

                return Err(report);
            }
        },
    };

    if let Some(ModSelection::Include(m) | ModSelection::Exact(m)) = mods {
        match PpCalculator::new(&ctx, map_id).await {
            Ok(calc) => map.stars = calc.mods(m).stars() as f32,
            Err(err) => warn!("{:?}", err.wrap_err("Failed to get pp calculator")),
        }
    }

    let mods = match mods {
        Some(ModSelection::Exclude(_)) | None => None,
        Some(ModSelection::Include(m)) | Some(ModSelection::Exact(m)) => Some(m),
    };

    // Retrieve the map's leaderboard
    let scores_future = ctx.client().get_leaderboard(map_id, mods, map.mode);

    let scores = match scores_future.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_WEB_ISSUE).await;

            return Err(err.wrap_err("failed to get leaderboard"));
        }
    };

    let amount = scores.len();

    // Accumulate all necessary data
    let first_place_icon = scores.first().map(|s| format!("{AVATAR_URL}{}", s.user_id));

    // Sending the embed
    let content =
        format!("I found {amount} scores with the specified mods on the map's leaderboard");

    LeaderboardPagination::builder(map, scores, author_name, first_place_icon)
        .start_by_update()
        .content(content)
        .start(ctx, orig)
        .await
}
