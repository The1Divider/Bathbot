use crate::{
    embeds::{EmbedData, RankingCountriesEmbed},
    pagination::{Pagination, RankingCountriesPagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers, MessageExt,
    },
    BotResult, CommandData, Context,
};

use eyre::Report;
use rosu_v2::prelude::GameMode;
use std::{collections::BTreeMap, sync::Arc};

pub(super) async fn _countryranking(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    mut mode: GameMode,
) -> BotResult<()> {
    let author_id = data.author()?.id;

    if mode == GameMode::STD {
        mode = match ctx.user_config(author_id).await {
            Ok(config) => config.mode.unwrap_or(GameMode::STD),
            Err(why) => {
                let _ = data.error(&ctx, GENERAL_ISSUE).await;

                return Err(why);
            }
        };
    }

    let mut ranking = match ctx.osu().country_rankings(mode).await {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    // Creating the embed
    let pages = numbers::div_euclid(15, ranking.total as usize);
    let countries: BTreeMap<_, _> = ranking.ranking.drain(..).enumerate().collect();
    let embed_data = RankingCountriesEmbed::new(mode, &countries, (1, pages));
    let builder = embed_data.into_builder().build().into();
    let response = data.create_message(&ctx, builder).await?.model().await?;

    // Pagination
    let pagination = RankingCountriesPagination::new(
        response,
        mode,
        Arc::clone(&ctx),
        ranking.total as usize,
        countries,
    );

    let owner = author_id;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display the osu! rankings for countries")]
#[aliases("cr")]
pub async fn countryranking(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    _countryranking(ctx, data, GameMode::STD).await
}

#[command]
#[short_desc("Display the osu!mania rankings for countries")]
#[aliases("crm")]
pub async fn countryrankingmania(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    _countryranking(ctx, data, GameMode::MNA).await
}

#[command]
#[short_desc("Display the osu!taiko rankings for countries")]
#[aliases("crt")]
pub async fn countryrankingtaiko(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    _countryranking(ctx, data, GameMode::TKO).await
}

#[command]
#[short_desc("Display the osu!ctb rankings for countries")]
#[aliases("crc")]
pub async fn countryrankingctb(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    _countryranking(ctx, data, GameMode::CTB).await
}
