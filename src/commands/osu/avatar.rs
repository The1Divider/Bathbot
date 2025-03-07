use std::{borrow::Cow, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::{Report, Result};
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    util::{
        builder::{AuthorBuilder, EmbedBuilder, MessageBuilder},
        constants::{GENERAL_ISSUE, OSU_API_ISSUE, OSU_BASE},
        interaction::InteractionCommand,
        matcher,
        osu::flag_url,
        InteractionCommandExt,
    },
    Context,
};

use super::{get_user, require_link, UserArgs};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(name = "avatar")]
/// Display someone's osu! profile picture
pub struct Avatar<'a> {
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

pub async fn slash_avatar(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Avatar::from_interaction(command.input_data())?;

    avatar(ctx, (&mut command).into(), args).await
}

#[command]
#[desc("Display someone's osu! profile picture")]
#[alias("pfp")]
#[usage("[username]")]
#[example("Badewanne3")]
#[group(AllModes)]
async fn prefix_avatar(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    avatar(ctx, msg.into(), Avatar::args(args)).await
}

impl<'m> Avatar<'m> {
    fn args(mut args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;

        if let Some(arg) = args.next() {
            match matcher::get_mention_user(arg) {
                Some(id) => discord = Some(id),
                None => name = Some(arg.into()),
            }
        }

        Self { name, discord }
    }
}

async fn avatar(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Avatar<'_>) -> Result<()> {
    let name = match username!(ctx, orig, args) {
        Some(name) => name,
        None => match ctx.psql().get_user_osu(orig.user_id()?).await {
            Ok(Some(osu)) => osu.into_username(),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err.wrap_err("failed to get username"));
            }
        },
    };

    let user_args = UserArgs::new(name.as_str(), GameMode::Osu);

    let user = match get_user(&ctx, &user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let report = Report::new(err).wrap_err("failed to get user");

            return Err(report);
        }
    };

    let author = AuthorBuilder::new(user.username.into_string())
        .url(format!("{OSU_BASE}u/{}", user.user_id))
        .icon_url(flag_url(user.country_code.as_str()));

    let embed = EmbedBuilder::new()
        .author(author)
        .image(user.avatar_url)
        .build();

    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}
