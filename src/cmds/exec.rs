use anyhow::Result;

use super::super::cmd;
use super::super::errors;
use super::super::model;
use super::super::query;

/// Main entry point for the "garden exec" command
/// Parameters:
/// - options: `garden::model::CommandOptions`

pub fn main(app: &mut model::ApplicationContext) -> Result<()> {
    let mut query = String::new();
    let mut command: Vec<String> = Vec::new();
    parse_args(&mut app.options, &mut query, &mut command);

    let quiet = app.options.quiet;
    let verbose = app.options.verbose;
    let config = app.get_root_config_mut();
    exec(config, quiet, verbose, &query, &command)
}

/// Parse "exec" arguments
fn parse_args(options: &mut model::CommandOptions, query: &mut String, command: &mut Vec<String>) {
    let mut ap = argparse::ArgumentParser::new();
    ap.silence_double_dash(false);
    ap.stop_on_first_argument(true);
    ap.set_description("garden exec - Run commands inside gardens");

    ap.refer(query).required().add_argument(
        "query",
        argparse::Store,
        "Tree query for the gardens, groups or trees to run the command",
    );

    ap.refer(command).required().add_argument(
        "command",
        argparse::List,
        "Command to run in the resolved tree(s)",
    );

    options.args.insert(0, "garden exec".into());
    cmd::parse_args(ap, options.args.to_vec());

    if options.debug_level("exec") > 0 {
        debug!("command: exec");
        debug!("query: {}", query);
        debug!("command: {:?}", command);
    }
}

/// Execute a command over every tree in the evaluated tree query.
pub fn exec(
    config: &mut model::Configuration,
    quiet: bool,
    verbose: u8,
    query: &str,
    command: &[String],
) -> Result<()> {
    // Strategy: resolve the trees down to a set of tree indexes paired with an
    // an optional garden context.
    //
    // If the names resolve to gardens, each garden is processed independently.
    // Trees that exist in multiple matching gardens will be processed multiple
    // times.
    //
    // If the names resolve to trees, each tree is processed independently
    // with no garden context.

    // Resolve the tree query into a vector of tree contexts.
    let contexts = query::resolve_trees(config, query);
    let mut exit_status: i32 = 0;
    if command.is_empty() {
        return Err(
            errors::GardenError::Usage("a command to execute must be specified".into()).into(),
        );
    }

    // Loop over each context, evaluate the tree environment,
    // and run the command.
    for context in &contexts {
        // Skip symlink trees.
        if config.trees[context.tree].is_symlink {
            continue;
        }
        // Run the command in the current context.
        if let Err(errors::GardenError::ExitStatus(status)) =
            cmd::exec_in_context(config, context, quiet, verbose, command)
        {
            exit_status = status;
        }
    }

    // Return the last non-zero exit status.
    cmd::result_from_exit_status(exit_status).map_err(|err| err.into())
}
