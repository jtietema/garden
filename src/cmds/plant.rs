use anyhow::Result;
use yaml_rust::yaml::Hash as YamlHash;
use yaml_rust::yaml::Yaml;

use super::super::cmd;
use super::super::config;
use super::super::errors;
use super::super::git;
use super::super::model;
use super::super::path;
use super::super::query;

pub fn main(app: &mut model::ApplicationContext) -> Result<()> {
    let mut output = String::new();
    let mut paths: Vec<String> = Vec::new();
    parse_args(&mut app.options, &mut output, &mut paths);

    // Read existing configuration
    let verbose = app.options.verbose;
    let config = app.get_root_config_mut();
    let mut doc = config::reader::read_yaml(config.get_path()?)?;

    // Output filename defaults to the input filename.
    if output.is_empty() {
        output = config.get_path()?.to_string_lossy().into();
    }

    // Mutable YAML scope.
    {
        // Get a mutable reference to top-level document hash.
        let doc_hash: &mut YamlHash = match doc {
            Yaml::Hash(ref mut hash) => hash,
            _ => {
                error!("invalid config: not a hash");
            }
        };

        // Get a mutable reference to the "trees" hash.
        let key = Yaml::String("trees".into());
        let trees: &mut YamlHash = match doc_hash.get_mut(&key) {
            Some(Yaml::Hash(ref mut hash)) => hash,
            _ => {
                error!("invalid trees: not a hash");
            }
        };

        for path in &paths {
            if let Err(msg) = plant_path(config, verbose, path, trees) {
                error!("{}", msg);
            }
        }
    }

    // Emit the YAML configuration into a string
    Ok(config::writer::write_yaml(&doc, &output)?)
}

fn parse_args(options: &mut model::CommandOptions, output: &mut String, paths: &mut Vec<String>) {
    let mut ap = argparse::ArgumentParser::new();
    ap.set_description("garden plant - Add pre-existing worktrees to a garden file");

    ap.refer(output).add_option(
        &["-o", "--output"],
        argparse::Store,
        "File to write (default: garden.yaml)",
    );

    ap.refer(paths)
        .required()
        .add_argument("paths", argparse::List, "Trees to plant");

    options.args.insert(0, "garden plant".into());
    cmd::parse_args(ap, options.args.to_vec());
}

fn plant_path(
    config: &model::Configuration,
    verbose: u8,
    raw_path: &str,
    trees: &mut YamlHash,
) -> Result<()> {
    // Garden root path
    let root = config.root_path.canonicalize().map_err(|err| {
        errors::GardenError::ConfigurationError(format!(
            "unable to canonicalize config root: {:?}",
            err
        ))
    })?;

    let pathbuf = std::path::PathBuf::from(raw_path);
    if !pathbuf.exists() {
        return Err(errors::GardenError::ConfigurationError(format!(
            "invalid tree path: {}",
            raw_path
        ))
        .into());
    }

    let mut is_worktree = false;
    let mut parent_tree_name = String::new();
    let parent_pathbuf;
    let worktree_details = git::worktree_details(&pathbuf)?;

    // If this is a worktree child then automatically "garden plant" the parent worktree.
    if let model::GitTreeType::Worktree(parent_path_abspath) = worktree_details.tree_type {
        parent_pathbuf = std::path::PathBuf::from(parent_path_abspath);
        let parent_path = path::strip_prefix_into_string(&root, &parent_pathbuf)?;
        is_worktree = true;

        parent_tree_name = match query::tree_name_from_abspath(config, &parent_pathbuf) {
            Some(tree_name) => tree_name,
            None => {
                return Err(errors::GardenError::WorktreeParentNotPlantedError {
                    parent: parent_path,
                    tree: raw_path.into(),
                }
                .into())
            }
        };
    }

    // Get a canonical tree path for comparison with the canonical root.
    let path = pathbuf.canonicalize().map_err(|err| {
        errors::GardenError::ConfigurationError(format!(
            "unable to canonicalize {:?}: {:?}",
            raw_path, err
        ))
    })?;

    // Build the tree's path
    let tree_path = path::strip_prefix_into_string(&root, &path)?;

    // Tree name is updated when an existing tree is found.
    let tree_name = match query::tree_name_from_abspath(config, &path) {
        Some(value) => value,
        None => tree_path,
    };

    // Key for the tree entry
    let key = Yaml::String(tree_name.clone());

    // Update an existing tree entry if it already exists.
    // Add a new entry otherwise.
    let mut entry: YamlHash = YamlHash::new();
    if let Some(tree_yaml) = trees.get(&key) {
        if let Some(tree_hash) = tree_yaml.as_hash() {
            if verbose > 0 {
                eprintln!("{}: found existing tree", tree_name);
            }
            entry = tree_hash.clone();
        }
    }

    // If this is a child worktree then record a "worktree" entry only.
    if is_worktree {
        entry.insert(
            Yaml::String("worktree".to_string()),
            Yaml::String(parent_tree_name),
        );
        entry.insert(
            Yaml::String("branch".to_string()),
            Yaml::String(worktree_details.branch.to_string()),
        );

        // Move the entry into the trees container
        if let Some(tree_entry) = trees.get_mut(&key) {
            *tree_entry = Yaml::Hash(entry);
        } else {
            trees.insert(key, Yaml::Hash(entry));
        }

        return Ok(());
    }

    let remotes_key = Yaml::String("remotes".into());
    let has_remotes = match entry.get(&remotes_key) {
        Some(remotes_yaml) => remotes_yaml.as_hash().is_some(),
        None => false,
    };

    // Gather remote names
    let mut remote_names: Vec<String> = Vec::new();
    {
        let command = ["git", "remote"];
        let exec = cmd::exec_in_dir(&command, &path);
        if let Ok(x) = cmd::capture_stdout(exec) {
            let output = cmd::trim_stdout(&x);

            for line in output.lines() {
                // Skip "origin" since it is defined by the "url" entry.
                if line == "origin" {
                    continue;
                }
                // Any other remotes are part of the "remotes" hash.
                remote_names.push(line.into());
            }
        }
    }

    // Gather remote urls
    let mut remotes: Vec<(String, String)> = Vec::new();
    {
        for remote in &remote_names {
            let cmd = ["git", "config", &format!("remote.{}.url", remote)];
            let exec = cmd::exec_in_dir(&cmd, &path);
            if let Ok(x) = cmd::capture_stdout(exec) {
                let output = cmd::trim_stdout(&x);
                remotes.push((remote.clone(), output));
            }
        }
    }

    if !remotes.is_empty() {
        if !has_remotes {
            entry.insert(remotes_key.clone(), Yaml::Hash(YamlHash::new()));
        }

        let remotes_hash: &mut YamlHash = match entry.get_mut(&remotes_key) {
            Some(Yaml::Hash(ref mut hash)) => hash,
            _ => {
                return Err(errors::GardenError::ConfigurationError(
                    "trees: not a hash".to_string(),
                )
                .into());
            }
        };

        for (k, v) in &remotes {
            let remote = Yaml::String(k.clone());
            let value = Yaml::String(v.clone());

            if let Some(remote_entry) = remotes_hash.get_mut(&remote) {
                *remote_entry = value;
            } else {
                remotes_hash.insert(remote, value);
            }
        }
    }

    let url_key = Yaml::String("url".into());
    if verbose > 0 && entry.contains_key(&url_key) {
        eprintln!("{}: no url", tree_name);
    }

    // Update the "url" field.
    {
        let command = ["git", "config", "remote.origin.url"];
        let exec = cmd::exec_in_dir(&command, &path);
        if let Ok(cmd_stdout) = cmd::capture_stdout(exec) {
            let origin_url = cmd::trim_stdout(&cmd_stdout);
            entry.insert(url_key, Yaml::String(origin_url));
        }
    }

    // Update the "bare" field.
    {
        let bare_key = Yaml::String("bare".into());
        let command = ["git", "config", "--bool", "core.bare"];
        let exec = cmd::exec_in_dir(&command, &path);
        if let Ok(cmd_stdout) = cmd::capture_stdout(exec) {
            let is_bare = cmd::trim_stdout(&cmd_stdout);
            if is_bare == "true" {
                entry.insert(bare_key, Yaml::Boolean(true));
            }
        }
    }

    // Move the entry into the trees container
    if let Some(tree_entry) = trees.get_mut(&key) {
        *tree_entry = Yaml::Hash(entry);
    } else {
        trees.insert(key, Yaml::Hash(entry));
    }

    Ok(())
}
