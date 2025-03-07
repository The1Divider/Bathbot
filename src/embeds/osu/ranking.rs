use std::{
    collections::{btree_map::Range, BTreeMap},
    fmt::{self, Write},
};

use rosu_v2::prelude::{GameMode, Username};
use twilight_model::{
    id::{marker::GuildMarker, Id},
    util::ImageHash,
};

use crate::{
    commands::osu::UserValue,
    database::UserStatsColumn,
    embeds::EmbedData,
    games::hl::HlVersion,
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, EmbedBuilder, FooterBuilder},
        CountryCode,
    },
};

pub struct RankingEntry {
    pub value: UserValue,
    pub name: Username,
    pub country: Option<CountryCode>,
}

enum EmbedHeader {
    Author(AuthorBuilder),
    Title { text: String, url: String },
}

impl EmbedHeader {
    fn title(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self::Title {
            text: text.into(),
            url: url.into(),
        }
    }
}

pub enum RankingKindData {
    BgScores {
        global: bool,
        scores: Vec<(u64, u32)>,
    },
    HlScores {
        scores: Vec<(u64, u32)>,
        version: HlVersion,
    },
    OsekaiRarity,
    OsekaiMedalCount,
    OsekaiReplays,
    OsekaiTotalPp,
    OsekaiStandardDeviation,
    OsekaiBadges,
    OsekaiRankedMapsets,
    OsekaiLovedMapsets,
    OsekaiSubscribers,
    PpCountry {
        country: String,
        country_code: CountryCode,
        mode: GameMode,
    },
    PpGlobal {
        mode: GameMode,
    },
    RankedScore {
        mode: GameMode,
    },
    UserStats {
        guild_icon: Option<(Id<GuildMarker>, ImageHash)>,
        kind: UserStatsColumn,
    },
}

impl RankingKindData {
    fn embed_header(&self) -> EmbedHeader {
        match self {
            Self::BgScores { global, .. } => {
                let text = if *global {
                    "Global leaderboard for correct guesses"
                } else {
                    "Server leaderboard for correct guesses"
                };

                EmbedHeader::Author(AuthorBuilder::new(text))
            }
            Self::HlScores { version, .. } => {
                let text = match version {
                    HlVersion::ScorePp => "Server leaderboard for Higherlower (Score PP)",
                    HlVersion::FarmMaps => "Server leaderboard for Higherlower (Farm)",
                };

                EmbedHeader::Author(AuthorBuilder::new(text))
            }
            Self::OsekaiRarity => {
                let text = "Medal Ranking based on rarity";
                let url = "https://osekai.net/rankings/?ranking=Medals&type=Rarity";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiMedalCount => {
                let text = "User Ranking based on amount of owned medals";
                let url = "https://osekai.net/rankings/?ranking=Medals&type=Users";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiReplays => {
                let text = "User Ranking based on watched replays";
                let url = "https://osekai.net/rankings/?ranking=All+Mode&type=Replays";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiTotalPp => {
                let text = "User Ranking based on total pp across all modes";
                let url = "https://osekai.net/rankings/?ranking=All+Mode&type=Total+pp";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiStandardDeviation => {
                let text = "User Ranking based on pp standard deviation of all modes";
                let url = "https://osekai.net/rankings/?ranking=All+Mode&type=Standard+Deviation";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiBadges => {
                let text = "User Ranking based on amount of badges";
                let url = "https://osekai.net/rankings/?ranking=Badges&type=Badges";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiRankedMapsets => {
                let text = "User Ranking based on created ranked mapsets";
                let url = "https://osekai.net/rankings/?ranking=Mappers&type=Ranked+Mapsets";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiLovedMapsets => {
                let text = "User Ranking based on created loved mapsets";
                let url = "https://osekai.net/rankings/?ranking=Mappers&type=Loved+Mapsets";

                EmbedHeader::title(text, url)
            }
            Self::OsekaiSubscribers => {
                let text = "User Ranking based on amount of mapping subscribers";
                let url = "https://osekai.net/rankings/?ranking=Mappers&type=Subscribers";

                EmbedHeader::title(text, url)
            }
            Self::PpCountry {
                country,
                country_code,
                mode,
            } => {
                let text = format!(
                    "{country}'{plural} Performance Ranking for osu!{mode}",
                    plural = if country.ends_with('s') { "" } else { "s" },
                    mode = mode_str(*mode),
                );

                let url = format!(
                    "https://osu.ppy.sh/rankings/{mode}/performance?country={country_code}",
                );

                EmbedHeader::title(text, url)
            }
            Self::PpGlobal { mode } => {
                let text = format!("Performance Ranking for osu!{mode}", mode = mode_str(*mode));
                let url = format!("https://osu.ppy.sh/rankings/{mode}/performance");

                EmbedHeader::title(text, url)
            }
            Self::RankedScore { mode } => {
                let text = format!(
                    "Ranked Score Ranking for osu!{mode}",
                    mode = mode_str(*mode),
                );

                let url = format!("https://osu.ppy.sh/rankings/{mode}/score");

                EmbedHeader::title(text, url)
            }
            Self::UserStats { guild_icon, kind } => {
                let mode = kind.mode();

                let kind = match kind {
                    UserStatsColumn::Badges => "Badges",
                    UserStatsColumn::Comments => "Comments",
                    UserStatsColumn::Followers => "Followers",
                    UserStatsColumn::ForumPosts => "Forum posts",
                    UserStatsColumn::GraveyardMapsets => "Graveyard mapsets",
                    UserStatsColumn::JoinDate => "Join date",
                    UserStatsColumn::KudosuAvailable => "Kudosu available",
                    UserStatsColumn::KudosuTotal => "Kudosu total",
                    UserStatsColumn::LovedMapsets => "Loved mapsets",
                    UserStatsColumn::MappingFollowers => "Mapping followers",
                    UserStatsColumn::Medals => "Medals",
                    UserStatsColumn::PlayedMaps => "Played maps",
                    UserStatsColumn::RankedMapsets => "Ranked mapsets",
                    UserStatsColumn::Usernames => "Namechange count",
                    UserStatsColumn::Accuracy { .. } => "Accuracy",
                    UserStatsColumn::AverageHits { .. } => "Average hits per play",
                    UserStatsColumn::CountSsh { .. } => "Count SSH",
                    UserStatsColumn::CountSs { .. } => "Count SS",
                    UserStatsColumn::TotalSs { .. } => "Total SS",
                    UserStatsColumn::CountSh { .. } => "Count SH",
                    UserStatsColumn::CountS { .. } => "Count S",
                    UserStatsColumn::TotalS { .. } => "Total S",
                    UserStatsColumn::CountA { .. } => "Count A",
                    UserStatsColumn::Level { .. } => "Level",
                    UserStatsColumn::MaxCombo { .. } => "Max combo",
                    UserStatsColumn::Playcount { .. } => "Playcount",
                    UserStatsColumn::Playtime { .. } => "Playtime",
                    UserStatsColumn::Pp { .. } => "PP",
                    UserStatsColumn::RankCountry { .. } => "Country rank",
                    UserStatsColumn::RankGlobal { .. } => "Global rank",
                    UserStatsColumn::Replays { .. } => "Replays watched",
                    UserStatsColumn::ScoreRanked { .. } => "Ranked score",
                    UserStatsColumn::ScoreTotal { .. } => "Total score",
                    UserStatsColumn::ScoresFirst { .. } => "Global #1s",
                    UserStatsColumn::TotalHits { .. } => "Total hits",
                };

                let mut author_text = "Server leaderboard".to_owned();

                if let Some(mode) = mode {
                    let _ = write!(author_text, " for osu!{mode}", mode = mode_str(mode));
                }

                let _ = write!(author_text, ": {kind}");

                let mut author = AuthorBuilder::new(author_text);

                if let Some((id, icon)) = guild_icon {
                    let ext = if icon.is_animated() { "gif" } else { "webp" };
                    let url = format!("https://cdn.discordapp.com/icons/{id}/{icon}.{ext}");
                    author = author.icon_url(url);
                }

                EmbedHeader::Author(author)
            }
        }
    }

    fn footer(
        &self,
        curr_page: usize,
        total_pages: usize,
        author_idx: Option<usize>,
    ) -> FooterBuilder {
        let mut text = format!("Page {curr_page}/{total_pages}");

        if let Some(idx) = author_idx {
            let _ = write!(text, " • Your position: {}", idx + 1);
        }

        match self {
            RankingKindData::OsekaiRarity
            | RankingKindData::OsekaiMedalCount
            | RankingKindData::OsekaiReplays
            | RankingKindData::OsekaiTotalPp
            | RankingKindData::OsekaiStandardDeviation
            | RankingKindData::OsekaiBadges
            | RankingKindData::OsekaiRankedMapsets
            | RankingKindData::OsekaiLovedMapsets
            | RankingKindData::OsekaiSubscribers => {
                text.push_str(" • Check out osekai.net for more info")
            }
            _ => {}
        };

        FooterBuilder::new(text)
    }
}

pub struct RankingEmbed {
    description: String,
    footer: FooterBuilder,
    header: EmbedHeader,
}

type RankingMap = BTreeMap<usize, RankingEntry>;

impl RankingEmbed {
    pub fn new(
        users: &RankingMap,
        data: &RankingKindData,
        author_idx: Option<usize>,
        pages: &Pages,
    ) -> Self {
        let page = pages.curr_page();
        let pages = pages.last_page();

        let index = (page - 1) * 20;

        let mut buf = String::new();

        let left_lengths = Lengths::new(&mut buf, users.range(index..index + 10));
        let right_lengths = Lengths::new(&mut buf, users.range(index + 10..index + 20));

        let mut description = String::with_capacity(1024);

        // Ensuring the right side has ten elements for the zip
        let user_iter = users
            .range(index..index + 10)
            .zip((10..20).map(|i| users.get(&(index + i))));

        for ((i, left_entry), right) in user_iter {
            let idx = i + 1;

            buf.clear();
            let _ = write!(buf, "{}", left_entry.value);

            let _ = write!(
                description,
                "`#{idx:<idx_len$}`{country}`{name:<name_len$}` `{buf:>value_len$}`",
                idx_len = left_lengths.idx,
                country = CountryFormatter::new(left_entry),
                name = left_entry.name,
                name_len = left_lengths.name,
                value_len = left_lengths.value,
            );

            if let Some(right_entry) = right {
                buf.clear();
                let _ = write!(buf, "{}", right_entry.value);

                let _ = write!(
                    description,
                    "|`#{idx:<idx_len$}`{country}`{name:<name_len$}` `{buf:>value_len$}`",
                    idx = idx + 10,
                    idx_len = right_lengths.idx,
                    country = CountryFormatter::new(right_entry),
                    name = right_entry.name,
                    name_len = right_lengths.name,
                    value_len = right_lengths.value,
                );
            }

            description.push('\n');
        }

        Self {
            description,
            footer: data.footer(page, pages, author_idx),
            header: data.embed_header(),
        }
    }
}

impl EmbedData for RankingEmbed {
    fn build(self) -> twilight_model::channel::embed::Embed {
        let builder = EmbedBuilder::new()
            .description(self.description)
            .footer(self.footer);

        match self.header {
            EmbedHeader::Author(author) => builder.author(author).build(),
            EmbedHeader::Title { text, url } => builder.title(text).url(url).build(),
        }
    }
}

fn mode_str(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Osu => "",
        GameMode::Taiko => "taiko",
        GameMode::Catch => "ctb",
        GameMode::Mania => "mania",
    }
}

struct Lengths {
    idx: usize,
    name: usize,
    value: usize,
}

impl Lengths {
    fn new(buf: &mut String, iter: Range<'_, usize, RankingEntry>) -> Self {
        let mut idx_len = 0;
        let mut name_len = 0;
        let mut value_len = 0;

        for (i, entry) in iter {
            let mut idx = i + 1;
            let mut len = 0;

            while idx > 0 {
                len += 1;
                idx /= 10;
            }

            idx_len = idx_len.max(len);
            name_len = name_len.max(entry.name.chars().count());

            buf.clear();
            let _ = write!(buf, "{}", entry.value);
            value_len = value_len.max(buf.len());
        }

        Lengths {
            idx: idx_len,
            name: name_len,
            value: value_len,
        }
    }
}

struct CountryFormatter<'e> {
    entry: &'e RankingEntry,
}

impl<'e> CountryFormatter<'e> {
    fn new(entry: &'e RankingEntry) -> Self {
        Self { entry }
    }
}

impl fmt::Display for CountryFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref country) = self.entry.country {
            write!(f, ":flag_{}:", country.to_ascii_lowercase())
        } else {
            f.write_str(" ")
        }
    }
}
