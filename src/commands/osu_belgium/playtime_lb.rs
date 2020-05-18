use crate::{commands::checks::*, util::numbers};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
    prelude::Context,
};

#[command]
#[checks(MainGuild)]
#[description = "Show the playtime leaderboard among all linked members in this server.\n\
                If no mode is specified it defaults to osu!standard."]
#[usage = "[mania / taiko / ctb]"]
async fn playtime(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mode = super::get_mode(args);
    let (users, next_update) =
        super::member_users(ctx, msg.channel_id, msg.guild_id.unwrap(), mode).await?;

    // Map to playtime, sort, then format
    let mut users: Vec<_> = users
        .into_iter()
        .map(|u| (u.username, u.total_seconds_played / 3600))
        .collect();
    users.sort_by(|(_, a), (_, b)| b.cmp(&a));
    let users: Vec<_> = users
        .into_iter()
        .map(|(name, hours)| (name, numbers::with_comma_u64(hours as u64) + " hrs"))
        .collect();

    // Send response
    super::send_response(ctx, users, next_update, msg).await
}