use crate::{
    util::{datetime::how_long_ago, discord},
    BootTime,
};

use chrono::{DateTime, Utc};
use serenity::{
    framework::standard::{macros::command, CommandResult},
    model::prelude::Message,
    prelude::Context,
    utils::Colour,
};
use sysinfo::{get_current_pid, ProcessExt, System, SystemExt};

#[command]
#[description = "Displaying some information about this bot"]
#[aliases("info")]
fn about(ctx: &mut Context, msg: &Message) -> CommandResult {
    let owner = ctx.http.get_current_application_info()?.owner;

    let system = System::new_all();
    let pid = get_current_pid()?;
    let process = system.get_process(pid).unwrap();
    let cpu_usage = (process.cpu_usage() * 100.0).round() / 100.0;
    let memory = process.memory() / 1000;

    let cache = &ctx.cache.read();
    let name = cache.user.name.clone();
    let avatar = cache.user.avatar_url().unwrap();
    let users = cache.users.len().to_string();
    let guilds = cache.guilds.len().to_string();
    let channels = cache.channels.len().to_string();

    let response = {
        let data = ctx.data.read();
        let boot_time: &DateTime<Utc> = data.get::<BootTime>().expect("Could not get BootTime");
        msg.channel_id.send_message(&ctx.http, |m| {
            m.embed(|e| {
                e.title(format!("About {}", name))
                    .color(Colour::DARK_GREEN)
                    .thumbnail(avatar)
                    .fields(vec![
                        ("Guilds", guilds, true),
                        ("Users", users, true),
                        ("Channels", channels, true),
                        ("CPU", format!("{}%", cpu_usage), true),
                        ("RAM", format!("{} MB", memory), true),
                        ("Boot time", how_long_ago(&boot_time), true),
                    ])
                    .footer(|f| {
                        f.text(format!("Owner: {}", owner.tag()))
                            .icon_url(owner.avatar_url().unwrap())
                    })
            })
        })?
    };

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}