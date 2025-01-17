use anyhow::Result;

use garden::build;
use garden::cmds;
use garden::config;
use garden::errors;
use garden::model;

fn main() -> Result<()> {
    // Return the appropriate exit code when a GardenError is encountered.
    if let Err(err) = cmd_main() {
        let exit_status: i32 = match err.downcast::<errors::GardenError>() {
            Ok(garden_err) => {
                match garden_err {
                    // ExitStatus exits without printing a message.
                    errors::GardenError::ExitStatus(status) => status,
                    // Other GardenError variants print a message before exiting.
                    _ => {
                        eprintln!("error: {:#}", garden_err);
                        garden_err.into()
                    }
                }
            }
            Err(other_err) => {
                eprintln!("error: {:#}", other_err);
                1
            }
        };
        std::process::exit(exit_status);
    }

    Ok(())
}

fn cmd_main() -> Result<()> {
    let mut options = parse_args();

    // The following commands run without a configuration file
    match options.subcommand {
        model::Command::Help => {
            return cmds::help::main(&mut options);
        }
        model::Command::Init => {
            return cmds::init::main(&mut options);
        }
        _ => (),
    }

    let config = config::from_options(&options)?;
    let mut app = build::context_from_config(config, options)?;

    match app.options.subcommand.clone() {
        model::Command::Cmd => cmds::cmd::main(&mut app),
        model::Command::Custom(cmd) => cmds::cmd::custom(&mut app, &cmd),
        model::Command::Exec => cmds::exec::main(&mut app),
        model::Command::Eval => cmds::eval::main(&mut app),
        model::Command::Grow => cmds::grow::main(&mut app),
        model::Command::Help => Ok(()), // Handled above
        model::Command::Init => Ok(()), // Handled above
        model::Command::Inspect => cmds::inspect::main(&mut app),
        model::Command::List => cmds::list::main(&mut app),
        model::Command::Plant => cmds::plant::main(&mut app),
        model::Command::Prune => cmds::prune::main(&mut app),
        model::Command::Shell => cmds::shell::main(&mut app),
    }
}

fn parse_args() -> model::CommandOptions {
    let color_names = model::ColorMode::names();
    let color_help = format!("Set color mode {{{}}}", color_names);

    let mut options = model::CommandOptions::new();
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("garden - Cultivate git trees");
        ap.stop_on_first_argument(true);

        ap.refer(&mut options.filename_str).add_option(
            &["-c", "--config"],
            argparse::Store,
            "Set the config file to use",
        );

        ap.refer(&mut options.chdir).add_option(
            &["-C", "--chdir"],
            argparse::Store,
            "Change directories before searching for garden files",
        );

        ap.refer(&mut options.color_mode)
            .add_option(&["--color"], argparse::Store, &color_help);

        ap.refer(&mut options.debug).add_option(
            &["-d", "--debug"],
            argparse::Collect,
            "Increase verbosity for a debug category",
        );

        ap.refer(&mut options.root).add_option(
            &["-r", "--root"],
            argparse::Store,
            "Set the garden tree root (default: ${GARDEN_ROOT})",
        );

        ap.refer(&mut options.variables).add_option(
            &["-s", "--set"],
            argparse::Collect,
            "Set variables using 'name=value' expressions",
        );

        ap.refer(&mut options.verbose).add_option(
            &["-v", "--verbose"],
            argparse::IncrBy(1),
            "Increase verbosity level (default: 0)",
        );

        ap.refer(&mut options.quiet).add_option(
            &["-q", "--quiet"],
            argparse::StoreTrue,
            "Be quiet",
        );

        ap.refer(&mut options.subcommand).required().add_argument(
            "command",
            argparse::Store,
            "{cmd, eval, exec, grow, help, init, inspect, ls, plant, prune, shell, <custom>}",
        );

        ap.refer(&mut options.args)
            .add_argument("arguments", argparse::List, "Command arguments");

        ap.parse_args_or_exit();
    }
    options.update();

    options
}
