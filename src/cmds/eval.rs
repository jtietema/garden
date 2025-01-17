use anyhow::Result;

use super::super::cmd;
use super::super::eval;
use super::super::model;
use super::super::query;

pub fn main(app: &mut model::ApplicationContext) -> Result<()> {
    let mut expr = String::new();
    let mut tree = String::new();
    let mut garden = String::new();
    parse_args(&mut app.options, &mut expr, &mut tree, &mut garden);

    let config = app.get_root_config_mut();
    if tree.is_empty() {
        println!("{}", eval::value(config, &expr));
        return Ok(());
    }

    let mut garden_opt: Option<&str> = None;
    if !garden.is_empty() {
        garden_opt = Some(&garden);
    }

    // Evaluate and print the garden expression.
    let ctx = query::tree_context(config, &tree, garden_opt)?;
    let value = eval::tree_value(config, &expr, ctx.tree, ctx.garden);
    println!("{}", value);

    Ok(())
}

/// Parse "eval" arguments.
fn parse_args(
    options: &mut model::CommandOptions,
    expr: &mut String,
    tree: &mut String,
    garden: &mut String,
) {
    let mut ap = argparse::ArgumentParser::new();
    ap.set_description("garden eval - Evaluate garden expressions");

    ap.refer(expr)
        .required()
        .add_argument("expr", argparse::Store, "Expression to evaluate");

    ap.refer(tree)
        .add_argument("tree", argparse::Store, "Tree within which to evaluate.");

    ap.refer(garden).add_argument(
        "garden",
        argparse::Store,
        "Garden within which to evaluate.",
    );

    options.args.insert(0, "garden eval".into());
    cmd::parse_args(ap, options.args.to_vec());
}
