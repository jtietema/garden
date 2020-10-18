use anyhow::Result;
use argparse;

use super::super::cmd;
use super::super::model;


/// Entry point for `garden help`
/// Parameters:
/// - options: `garden::model::CommandOptions`

pub fn main(options: &mut model::CommandOptions) -> Result<()> {
    let cmd_path = cmd::current_exe();
    let mut help_cmd = vec!(cmd_path);

    let mut cmd_name = String::new();
    {
        let mut ap = argparse::ArgumentParser::new();
        ap.set_description("garden help - command documentation");

        ap.refer(&mut cmd_name)
            .add_argument("command", argparse::Store,
                          "{add, cmd, eval, exec, ls, shell}");

        options.args.insert(0, "garden help".to_string());
        cmd::parse_args(ap, options.args.to_vec());
    }

    // garden help foo -> garden foo --help
    if !cmd_name.is_empty() {
        help_cmd.push(cmd_name.to_string());
    }

    help_cmd.push("--help".to_string());

    if options.verbose {
        debug!("help command");
        let mut i: i32 = 0;
        for arg in &help_cmd {
            debug!("help_cmd[{:02}] = {:?}", i, arg);
            i += 1;
        }
    }

    cmd::run(&help_cmd).map_err(|err| err.into())
}
