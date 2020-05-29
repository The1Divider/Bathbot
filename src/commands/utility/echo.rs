use crate::{commands::checks::*, util::discord};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::prelude::Message,
    prelude::Context,
    utils::{content_safe, ContentSafeOptions},
};

#[command]
#[checks(Authority)]
#[description = "Make me repeat your message but without any pings"]
#[usage = "[sentence]"]
async fn echo(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let channel = msg.channel_id;
    msg.delete(ctx).await?;
    let content = content_safe(&ctx.cache, args.rest(), &ContentSafeOptions::default()).await;
    let response = channel.say(ctx, content).await?;
    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}
