use crate::{
    commands::osu::{request_user, ProfileSize},
    database::UserConfig,
    embeds::{ConfigEmbed, EmbedData},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        ApplicationCommandExt, CowUtils, MessageExt,
    },
    Args, BotResult, CommandData, Context, Name,
};

use rosu_v2::prelude::{GameMode, OsuError};
use std::{borrow::Cow, fmt::Write, sync::Arc};
use twilight_model::application::{
    command::{ChoiceCommandOptionData, Command, CommandOption, CommandOptionChoice},
    interaction::{application_command::CommandDataOption, ApplicationCommand},
};

#[command]
#[short_desc("Adjust your default configuration for commands")]
#[long_desc(
    "Adjust your default configuration for commands.\n\
    All arguments must be of the form `key=value`.\n\n\
    These are all keys and their values:\n\
    - `name`: Specify an osu! username. Don't forget to encapsulate it with `\"` if it contains whitespace.\n\
    - `mode`: `osu`, `taiko`, `ctb`, `mania`, or `none`. \
    If configured, you won't need to specify the mode for commands anymore e.g. \
    you can use the `recent` command instead of `recentmania` to show a recent mania score.\n\
    - `profile`: `compact`, `medium`, or `full`. Specify the initial size for the embed of profile commands.\n\
    - `retries`: `show` or `hide`. Whether I should show how many retries it took you \
    whenever you use the `recent` command.\n\
    - `embeds`: `minimized` or `maximized`. When using the `recent` command, choose whether the embed should \
    initially be maximized and get minimized after some delay, or if it should be minimized from the beginning. \
    This will also apply to the `compare`, `simulaterecent`, and indexed `top` command.\n\n\
    **NOTE:** If the mode is configured to anything non-standard, \
    you will __NOT__ be able to use __any__ command for osu!standard anymore."
)]
#[usage(
    "[name=username] [mode=osu/taiko/ctb/mania/none] [profile=compact/medium/full] \
    [retries=show/hide] [embeds=maximized/minimized]"
)]
#[example(
    "mode=mania name=\"freddie benson\" embeds=minimized",
    "name=peppy profile=full",
    "profile=medium retries=hide mode=ctb"
)]
async fn config(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => match ConfigArgs::args(&mut args) {
            Ok(config_args) => {
                _config(ctx, CommandData::Message { msg, args, num }, config_args).await
            }
            Err(content) => msg.error(&ctx, content).await,
        },
        CommandData::Interaction { command } => slash_config(ctx, *command).await,
    }
}

async fn _config(ctx: Arc<Context>, data: CommandData<'_>, args: ConfigArgs) -> BotResult<()> {
    let author = data.author()?;

    let ConfigArgs {
        mode,
        name,
        profile_size,
        embeds_maximized,
        show_retries,
    } = args;

    let name = match name.as_deref() {
        Some(name) => {
            if name.chars().count() > 15 {
                let content = "That name is too long, must be at most 15 characters";

                return data.error(&ctx, content).await;
            }

            match request_user(&ctx, name, None).await {
                Ok(user) => Some(user.username.into()),
                Err(OsuError::NotFound) => {
                    let mut content = format!("No user with the name `{}` was found.", name);

                    if name.contains('_') {
                        let _ = write!(
                            content,
                            "\nIf the name contains whitespace, be sure to encapsulate \
                                it inbetween quotation marks, e.g `\"{}\"`.",
                            name.replace('_', " "),
                        );
                    }

                    return data.error(&ctx, content).await;
                }
                Err(why) => {
                    let _ = data.error(&ctx, OSU_API_ISSUE).await;

                    return Err(why.into());
                }
            }
        }
        None => None,
    };

    let mut config = match ctx.psql().get_user_config(author.id).await {
        Ok(Some(config)) => config,
        Ok(None) => UserConfig::default(),
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    if let Some(mode) = mode {
        config.mode = mode;
    }

    if let Some(name) = name {
        config.name = Some(name);
    }

    if let Some(size) = profile_size {
        config.profile_size = Some(size);
    }

    if let Some(maximize) = embeds_maximized {
        config.embeds_maximized = maximize;
    }

    if let Some(retries) = show_retries {
        config.show_retries = retries;
    }

    if let Err(why) = ctx.psql().insert_user_config(author.id, &config).await {
        let _ = data.error(&ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let embed_data = ConfigEmbed::new(author, config);
    let builder = embed_data.into_builder().build().into();
    data.create_message(&ctx, builder).await?;

    Ok(())
}

struct ConfigArgs {
    embeds_maximized: Option<bool>,
    mode: Option<Option<GameMode>>,
    name: Option<Name>,
    profile_size: Option<ProfileSize>,
    show_retries: Option<bool>,
}

impl ConfigArgs {
    fn args(args: &mut Args) -> Result<Self, Cow<'static, str>> {
        let mut mode = None;
        let mut name = None;
        let mut profile_size = None;
        let mut embeds_maximized = None;
        let mut show_retries = None;

        for arg in args.map(CowUtils::cow_to_ascii_lowercase) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "mode" | "gamemode" | "m" => match value {
                        "none" => mode = Some(None),
                        "osu" | "osu!" | "0" | "standard" | "std" => {
                            mode = Some(Some(GameMode::STD))
                        }
                        "taiko" | "tko" | "1" => mode = Some(Some(GameMode::TKO)),
                        "ctb" | "catch the beat" | "2" | "catch" => {
                            mode = Some(Some(GameMode::CTB))
                        }
                        "mania" | "mna" | "3" => mode = Some(Some(GameMode::MNA)),
                        _ => {
                            let content = "Failed to parse `mode`. Must be either `osu`, `taiko`, `ctb`, or `mania`.";

                            return Err(content.into());
                        }
                    },
                    "name" | "username" | "n" | "u" => name = Some(value.into()),
                    "embeds" | "recent" => {
                        embeds_maximized = match value {
                            "minimized" | "minimize" | "min" | "false" => Some(false),
                            "maximized" | "maximize" | "max" | "true" => Some(true),
                            _ => {
                                let content = "Failed to parse `recent`. Must be either `minimized` or `maximized`.";

                                return Err(content.into());
                            }
                        }
                    }
                    "retries" | "r" => match value {
                        "show" => show_retries = Some(true),
                        "hide" => show_retries = Some(false),
                        _ => {
                            let content =
                                "Failed to parse `retries`. Must be either `show` or `hide`.";

                            return Err(content.into());
                        }
                    },
                    "profile" => match value {
                        "compact" | "small" => profile_size = Some(ProfileSize::Compact),
                        "medium" => profile_size = Some(ProfileSize::Medium),
                        "full" | "big" => profile_size = Some(ProfileSize::Full),
                        _ => {
                            let content = "Failed to parse `profile`. Must be either `compact`, `medium`, or `full`.";

                            return Err(content.into());
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{}`.\n\
                            Available options are: `embeds`, `mode`, `name`, `profile`, and `retries`.",
                            key
                        );

                        return Err(content.into());
                    }
                }
            } else {
                let content = format!(
                    "All arguments must be of the form `key=value` (`{}` wasn't).\n\
                    Available keys are: `embeds`, `mode`, `name`, `profile`, and `retries`.",
                    arg
                );

                return Err(content.into());
            }
        }

        let args = Self {
            name,
            mode,
            profile_size,
            embeds_maximized,
            show_retries,
        };

        Ok(args)
    }

    fn slash(command: &mut ApplicationCommand) -> BotResult<Self> {
        let mut mode = None;
        let mut username = None;
        let mut profile_size = None;
        let mut embeds_maximized = None;
        let mut show_retries = None;

        for option in command.yoink_options() {
            match option {
                CommandDataOption::String { name, value } => match name.as_str() {
                    "mode" => {
                        mode = match value.as_str() {
                            "none" => Some(None),
                            "osu" => Some(Some(GameMode::STD)),
                            "taiko" => Some(Some(GameMode::TKO)),
                            "catch" => Some(Some(GameMode::CTB)),
                            "mania" => Some(Some(GameMode::MNA)),
                            _ => bail_cmd_option!("config mode", string, value),
                        }
                    }
                    "profile" => match value.as_str() {
                        "compact" => profile_size = Some(ProfileSize::Compact),
                        "medium" => profile_size = Some(ProfileSize::Medium),
                        "full" => profile_size = Some(ProfileSize::Full),
                        _ => bail_cmd_option!("config profile", string, value),
                    },
                    "embeds" => match value.as_str() {
                        "maximized" => embeds_maximized = Some(true),
                        "minimized" => embeds_maximized = Some(false),
                        _ => bail_cmd_option!("config embeds", string, value),
                    },
                    "retries" => match value.as_str() {
                        "show" => show_retries = Some(true),
                        "hide" => show_retries = Some(false),
                        _ => bail_cmd_option!("config retries", string, value),
                    },
                    "name" => username = Some(value.into()),
                    _ => bail_cmd_option!("config", string, name),
                },
                CommandDataOption::Integer { name, .. } => {
                    bail_cmd_option!("config", integer, name)
                }
                CommandDataOption::Boolean { name, .. } => {
                    bail_cmd_option!("config", boolean, name)
                }
                CommandDataOption::SubCommand { name, .. } => {
                    bail_cmd_option!("config", subcommand, name)
                }
            }
        }

        let args = Self {
            mode,
            name: username,
            profile_size,
            embeds_maximized,
            show_retries,
        };

        Ok(args)
    }
}

pub async fn slash_config(ctx: Arc<Context>, mut command: ApplicationCommand) -> BotResult<()> {
    let args = ConfigArgs::slash(&mut command)?;

    _config(ctx, command.into(), args).await
}

pub fn slash_config_command() -> Command {
    Command {
        application_id: None,
        guild_id: None,
        name: "config".to_owned(),
        default_permission: None,
        description: "Adjust your default configuration for commands".to_owned(),
        id: None,
        options: vec![
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![],
                description: "Specify a username".to_owned(),
                name: "name".to_owned(),
                required: false,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![
                    CommandOptionChoice::String {
                        name: "none".to_owned(),
                        value: "none".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "osu".to_owned(),
                        value: "osu".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "taiko".to_owned(),
                        value: "taiko".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "catch".to_owned(),
                        value: "catch".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "mania".to_owned(),
                        value: "mania".to_owned(),
                    },
                ],
                description: "Specify a gamemode (NOTE: Only use for non-std modes if you NEVER use std commands)".to_owned(),
                name: "mode".to_owned(),
                required: false,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![
                    CommandOptionChoice::String {
                        name: "compact".to_owned(),
                        value: "compact".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "medium".to_owned(),
                        value: "medium".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "full".to_owned(),
                        value: "full".to_owned(),
                    },
                ],
                description: "What initial size should the profile command be?".to_owned(),
                name: "profile".to_owned(),
                required: false,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![
                    CommandOptionChoice::String {
                        name: "maximized".to_owned(),
                        value: "maximized".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "minimized".to_owned(),
                        value: "minimized".to_owned(),
                    },
                ],
                description: "What initial size should the recent, compare, simulate, ... commands be?".to_owned(),
                name: "embeds".to_owned(),
                required: false,
            }),
            CommandOption::String(ChoiceCommandOptionData {
                choices: vec![
                    CommandOptionChoice::String {
                        name: "show".to_owned(),
                        value: "show".to_owned(),
                    },
                    CommandOptionChoice::String {
                        name: "hide".to_owned(),
                        value: "hide".to_owned(),
                    },
                ],
                description: "Should the amount of retries be shown for the `recent` command?".to_owned(),
                name: "retries".to_owned(),
                required: false,
            }),
        ],
    }
}
