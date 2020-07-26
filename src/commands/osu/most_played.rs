use crate::{
    arguments::{Args, NameArgs},
    embeds::{EmbedData, MostPlayedEmbed},
    pagination::{MostPlayedPagination, Pagination},
    util::{constants::OSU_API_ISSUE, numbers, MessageExt},
    BotResult, Context,
};

use rosu::backend::requests::UserRequest;
use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Display the 10 most played maps of a user")]
#[usage("[username]")]
#[example("badewanne3")]
async fn mostplayed(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameArgs::new(args);
    let name = if let Some(name) = args.name {
        name
    } else {
        let data = ctx.data.read().await;
        let links = data.get::<DiscordLinks>().unwrap();
        match links.get(msg.author.id.as_u64()) {
            Some(name) => name.clone(),
            None => {
                msg.channel_id
                    .say(
                        ctx,
                        "Either specify an osu name or link your discord \
                        to an osu profile via `<link osuname`",
                    )
                    .await?
                    .reaction_delete(ctx, msg.author.id)
                    .await;
                return Ok(());
            }
        }
    };

    // Retrieve the user
    let (user, maps) = {
        let user_req = UserRequest::with_username(&name);
        let data = ctx.data.read().await;
        let user = {
            let osu = data.get::<Osu>().unwrap();
            match user_req.queue_single(&osu).await {
                Ok(result) => match result {
                    Some(user) => user,
                    None => {
                        let content = format!("User `{}` was not found", name);
                        msg.respond(&ctx, content).await?;
                        return Ok(());
                    }
                },
                Err(why) => {
                    msg.respond(&ctx, OSU_API_ISSUE).await?;
                    return Err(why.into());
                }
            }
        };
        let maps = {
            let scraper = data.get::<Scraper>().unwrap();
            match scraper.get_most_played(user.user_id, 50).await {
                Ok(maps) => maps,
                Err(why) => {
                    msg.respond(&ctx, OSU_API_ISSUE).await?;
                    return Err(why.into());
                }
            }
        };
        (user, maps)
    };

    // Accumulate all necessary data
    let pages = numbers::div_euclid(10, maps.len());
    let data = MostPlayedEmbed::new(&user, maps.iter().take(10), (1, pages));

    // Creating the embed
    let resp = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        resp.reaction_delete(ctx, msg.author.id).await;
        return Ok(());
    }

    // Pagination
    let pagination = MostPlayedPagination::new(ctx, resp, msg.author.id, user, maps).await;
    let cache = Arc::clone(&ctx.cache);
    let http = Arc::clone(&ctx.http);
    tokio::spawn(async move {
        if let Err(why) = pagination.start(cache, http).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}