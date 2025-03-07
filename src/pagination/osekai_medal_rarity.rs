use command_macros::pagination;
use twilight_model::channel::embed::Embed;

use crate::{
    custom_client::OsekaiRarityEntry,
    embeds::{EmbedData, MedalRarityEmbed},
};

use super::Pages;

#[pagination(per_page = 10, entries = "ranking")]
pub struct MedalRarityPagination {
    ranking: Vec<OsekaiRarityEntry>,
}

impl MedalRarityPagination {
    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index;
        let limit = self.ranking.len().min(idx + pages.per_page);

        MedalRarityEmbed::new(&self.ranking[idx..limit], pages).build()
    }
}
