use crate::{
    embeds::{EmbedData, InviteEmbed},
    util::MessageExt,
    Args, BotResult, Context,
};

use std::sync::Arc;
use twilight::model::channel::Message;

#[command]
#[short_desc("Invite me to your server")]
#[aliases("inv")]
async fn invite(ctx: Arc<Context>, msg: &Message, _: Args) -> BotResult<()> {
    let embed = InviteEmbed::new().build().build();
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}