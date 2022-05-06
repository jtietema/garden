use indextree::{Arena, NodeId};
use std::cell::RefCell;

use super::errors;
use super::eval;
use super::syntax;

/// Tree index into config.trees
pub type TreeIndex = usize;

/// Group index into config.groups
pub type GroupIndex = usize;

/// Garden index into config.gardens
pub type GardenIndex = usize;

/// Configuration Node IDs
pub type ConfigId = NodeId;

/// Config files can define a sequence of variables that are
/// iteratively calculated.  Variables can reference other
/// variables in their Tree, Garden, and Configuration scopes.
///
/// The config values can contain either plain values,
/// string ${expressions} that resolve against other Variables,
/// or exec expressions that evaluate to a command whose stdout is
/// captured and placed into the value of the variable.
///
/// An exec expression can use shell-like ${variable} references as which
/// are substituted when evaluating the command, just like a regular
/// string expression.  An exec expression is denoted by using a "$ "
/// (dollar-sign followed by space) before the value.  For example,
/// using "$ echo foo" will place the value "foo" in the variable.
#[derive(Clone, Debug, Default)]
pub struct Variable {
    expr: String,
    value: RefCell<Option<String>>,
}

impl_display_brief!(Variable);

impl Variable {
    pub fn new(expr: String, value: Option<String>) -> Self {
        Variable {
            expr,
            value: RefCell::new(value),
        }
    }

    pub fn get_expr(&self) -> &String {
        &self.expr
    }

    pub fn get_expr_mut(&mut self) -> &mut String {
        &mut self.expr
    }

    pub fn set_expr(&mut self, expr: String) {
        self.expr = expr;
    }

    pub fn set_value(&self, value: String) {
        *self.value.borrow_mut() = Some(value);
    }

    /// Transform the RefCell<Option<String>> value into an Option<&String>.
    pub fn get_value(&self) -> Option<&String> {
        let ptr = self.value.as_ptr();
        unsafe { (*ptr).as_ref() }
    }

    pub fn reset(&self) {
        *self.value.borrow_mut() = None;
    }
}

// Named variables with a single value
#[derive(Clone, Debug)]
pub struct NamedVariable {
    name: String,
    variable: Variable,
}

impl_display_brief!(NamedVariable);

impl NamedVariable {
    pub fn new(name: String, expr: String, value: Option<String>) -> Self {
        NamedVariable {
            name,
            variable: Variable::new(expr, value),
        }
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_expr(&self) -> &String {
        self.variable.get_expr()
    }

    pub fn set_expr(&mut self, expr: String) {
        self.variable.set_expr(expr);
    }

    pub fn set_value(&self, value: String) {
        self.variable.set_value(value);
    }

    pub fn get_value(&self) -> Option<&String> {
        self.variable.get_value()
    }

    pub fn reset(&self) {
        self.variable.reset();
    }
}

// Named variables with multiple values
#[derive(Clone, Debug)]
pub struct MultiVariable {
    name: String,
    variables: Vec<Variable>,
}

impl_display!(MultiVariable);

impl MultiVariable {
    pub fn new(name: String, variables: Vec<Variable>) -> Self {
        MultiVariable { name, variables }
    }

    pub fn get(&self, idx: usize) -> &Variable {
        &self.variables[idx]
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn len(&self) -> usize {
        self.variables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    pub fn reset(&self) {
        for var in &self.variables {
            var.reset();
        }
    }

    pub fn iter(&self) -> std::slice::Iter<Variable> {
        self.variables.iter()
    }
}

// Trees represent a single worktree
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub commands: Vec<MultiVariable>,
    pub environment: Vec<MultiVariable>,
    pub gitconfig: Vec<NamedVariable>,
    pub is_symlink: bool,
    pub remotes: Vec<NamedVariable>,
    pub symlink: Variable,
    pub templates: Vec<String>,
    pub variables: Vec<NamedVariable>,
    pub clone_depth: i64,

    name: String,
    path: Variable,
}

impl_display!(Tree);

impl Tree {
    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    pub fn get_path(&self) -> &Variable {
        &self.path
    }

    pub fn get_path_mut(&mut self) -> &mut Variable {
        &mut self.path
    }

    pub fn path_is_valid(&self) -> bool {
        self.path.get_value().is_some()
    }

    pub fn path_as_ref(&self) -> Result<&String, errors::GardenError> {
        match self.path.get_value() {
            Some(value) => Ok(value),
            None => Err(errors::GardenError::ConfigurationError(format!(
                "unset tree path for {}",
                self.name
            ))),
        }
    }

    pub fn symlink_as_ref(&self) -> Result<&String, errors::GardenError> {
        match self.symlink.get_value() {
            Some(ref value) => Ok(value),
            None => Err(errors::GardenError::ConfigurationError(format!(
                "unset tree path for {}",
                self.name
            ))),
        }
    }

    pub fn reset_variables(&self) {
        // self.path is a variable but it is not reset because
        // the tree path is evaluated once when the configuration
        // is first read, and never again.
        for var in &self.variables {
            var.reset();
        }

        for cfg in &self.gitconfig {
            cfg.reset();
        }

        for env in &self.environment {
            env.reset();
        }

        for cmd in &self.commands {
            cmd.reset();
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Group {
    name: String,
    index: GroupIndex,
    pub members: Vec<String>,
}

impl_display!(Group);

impl Group {
    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    pub fn get_index(&self) -> GardenIndex {
        self.index
    }
}

#[derive(Clone, Debug, Default)]
pub struct Template {
    pub commands: Vec<MultiVariable>,
    pub environment: Vec<MultiVariable>,
    pub extend: Vec<String>,
    pub gitconfig: Vec<NamedVariable>,
    pub remotes: Vec<NamedVariable>,
    pub variables: Vec<NamedVariable>,
    pub clone_depth: i64,
    name: String,
}

impl_display!(Template);

impl Template {
    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_name_mut(&mut self) -> &mut String {
        &mut self.name
    }
}

// Gardens aggregate trees
#[derive(Clone, Debug, Default)]
pub struct Garden {
    pub commands: Vec<MultiVariable>,
    pub environment: Vec<MultiVariable>,
    pub gitconfig: Vec<NamedVariable>,
    pub groups: Vec<String>,
    pub trees: Vec<String>,
    pub variables: Vec<NamedVariable>,
    name: String,
    index: GardenIndex,
}

impl_display!(Garden);

impl Garden {
    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_name_mut(&mut self) -> &mut String {
        &mut self.name
    }

    pub fn get_index(&self) -> GardenIndex {
        self.index
    }
}

// Configuration represents an instantiated garden configuration
#[derive(Clone, Debug, Default)]
pub struct Configuration {
    pub commands: Vec<MultiVariable>,
    pub debug: std::collections::HashSet<String>,
    pub environment: Vec<MultiVariable>,
    pub gardens: Vec<Garden>,
    pub grafts: Vec<Graft>,
    pub groups: Vec<Group>,
    pub path: Option<std::path::PathBuf>,
    pub dirname: Option<std::path::PathBuf>,
    pub root: Variable,
    pub root_path: std::path::PathBuf,
    pub shell: String,
    pub templates: Vec<Template>,
    pub tree_search_path: Vec<std::path::PathBuf>,
    pub trees: Vec<Tree>,
    pub variables: Vec<NamedVariable>,
    pub verbose: bool,
    id: Option<ConfigId>,
    parent_id: Option<ConfigId>,
}

impl_display!(Configuration);

impl Configuration {
    /// Create a default Configuration
    pub fn new() -> Self {
        Configuration {
            id: None,
            parent_id: None,
            shell: "zsh".into(),
            ..std::default::Default::default()
        }
    }

    pub fn initialize(&mut self) {
        // Evaluate garden.root
        let expr = self.root.get_expr().to_string();
        let value = eval::value(self, &expr);
        // Store the resolved garden.root
        self.root_path = std::path::PathBuf::from(&value);
        self.root.set_value(value);

        // Resolve tree paths
        self.update_tree_paths();

        // Assign garden.index to each garden
        self.update_indexes();

        // Reset variables
        self.reset();
    }

    pub fn reset(&mut self) {
        // Reset variables to allow for tree-scope evaluation
        self.reset_variables();

        // Add custom variables
        self.reset_builtin_variables()
    }

    fn reset_builtin_variables(&mut self) {
        // Update GARDEN_ROOT at position 0.
        if !self.variables.is_empty() && self.variables[0].get_name() == "GARDEN_ROOT" {
            if let Some(value) = self.root.get_value() {
                self.variables[0].set_expr(value.to_string());
                self.variables[0].set_value(value.to_string());
            }
        }

        for tree in self.trees.iter_mut() {
            if tree.variables.len() >= 2 {
                // Extract the tree's path.  Skip invalid/unset entries.
                let tree_path = match tree.path_as_ref() {
                    Ok(path) => path,
                    Err(_) => continue,
                }
                .to_string();
                // Update TREE_NAME at position 0.
                let tree_name = tree.get_name().to_string();
                if tree.variables[0].get_name() == "TREE_NAME" {
                    tree.variables[0].set_expr(tree_name.to_string());
                    tree.variables[0].set_value(tree_name.to_string());
                }
                // Update TREE_PATH at position 1.
                if tree.variables[1].get_name() == "TREE_PATH" {
                    tree.variables[1].set_expr(tree_path.to_string());
                    tree.variables[1].set_value(tree_path.to_string());
                }
            }
        }
    }

    fn update_indexes(&mut self) {
        for (idx, group) in self.groups.iter_mut().enumerate() {
            group.index = idx as GroupIndex;
        }

        for (idx, garden) in self.gardens.iter_mut().enumerate() {
            garden.index = idx as GardenIndex;
        }
    }

    // Calculate the "path" field for each tree.
    // If specified as a relative path, it will be relative to garden.root.
    // If specified as an asbolute path, it will be left as-is.
    fn update_tree_paths(&mut self) {
        // Gather path and symlink expressions.
        let mut path_values = Vec::new();
        let mut symlink_values = Vec::new();
        for (idx, tree) in self.trees.iter().enumerate() {
            path_values.push(tree.path.expr.clone());
            if tree.is_symlink {
                symlink_values.push((idx, tree.symlink.expr.clone()));
            }
        }

        // Evaluate the "path" expression.
        for (idx, value) in path_values.iter().enumerate() {
            let result = self.eval_tree_path(value);
            self.trees[idx].path.set_value(result);
        }

        // Evaluate the "symlink" expression.
        for (idx, value) in &symlink_values {
            let result = self.eval_tree_path(value);
            self.trees[*idx].symlink.set_value(result);
        }
    }

    /// Return a path string relative to the garden root
    pub fn tree_path(&self, path: &str) -> String {
        if std::path::PathBuf::from(path).is_absolute() {
            // Absolute path, nothing to do
            path.into()
        } else {
            // Make path relative to root_path
            let mut path_buf = self.root_path.to_path_buf();
            path_buf.push(path);

            path_buf.to_string_lossy().into()
        }
    }

    /// Evaluate and return a path string relative to the garden root.
    pub fn eval_tree_path(&mut self, path: &str) -> String {
        let value = eval::value(self, &path);
        self.tree_path(&value)
    }

    /// Resolve a path string relative to the config dir.
    pub fn config_path(&self, path: &str) -> String {
        if std::path::PathBuf::from(path).is_absolute() {
            // Absolute path, nothing to do
            path.to_string()
        } else if let Some(dirname) = self.dirname.as_ref() {
            // Make path relative to the configuration's dirname
            let mut path_buf = dirname.to_path_buf();
            path_buf.push(path);

            path_buf.to_string_lossy().to_string()
        } else {
            self.tree_path(path)
        }
    }

    /// Evaluate and resolve a path string and relative to the config dir.
    pub fn eval_config_path(&self, path: &str) -> String {
        let value = eval::value(self, &path);

        self.config_path(&value)
    }

    /// Reset resolved variables
    pub fn reset_variables(&mut self) {
        for var in &self.variables {
            var.reset();
        }
        for env in &self.environment {
            env.reset();
        }
        for cmd in &self.commands {
            cmd.reset();
        }
        for tree in &self.trees {
            tree.reset_variables();
        }
    }

    /// Set the ConfigId from the Arena for this configuration.
    pub fn set_id(&mut self, id: ConfigId) {
        self.id = Some(id);
    }

    pub fn get_id(&self) -> Option<ConfigId> {
        self.id
    }

    /// Set the parent ConfigId from the Arena for this configuration.
    pub fn set_parent(&mut self, id: ConfigId) {
        self.parent_id = Some(id);
    }

    /// Set the config path and the dirname fields
    pub fn set_path(&mut self, path: std::path::PathBuf) {
        let mut dirname = path.clone();
        dirname.pop();

        self.dirname = Some(dirname);
        self.path = Some(path);
    }

    /// Get the config path if it is defined.
    pub fn get_path(&self) -> Result<&std::path::PathBuf, errors::GardenError> {
        self.path
            .as_ref()
            .ok_or_else(|| errors::GardenError::AssertionError("cfg.path is unset".into()))
    }

    /// Return true if the configuration contains the named graft.
    pub fn contains_graft(&self, name: &str) -> bool {
        let graft_name = syntax::trim(name);
        for graft in &self.grafts {
            if graft.get_name() == graft_name {
                return true;
            }
        }
        false
    }

    /// Return a graft by name.
    pub fn get_graft(&self, name: &str) -> Result<&Graft, errors::GardenError> {
        let graft_name = syntax::trim(name);
        for graft in &self.grafts {
            if graft.get_name() == graft_name {
                return Ok(graft);
            }
        }
        Err(errors::GardenError::ConfigurationError(format!(
            "{}: no such graft",
            name
        )))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Graft {
    id: Option<ConfigId>,
    name: String,
    pub root: String,
    pub config: String,
}

impl_display!(Graft);

impl Graft {
    pub fn new(name: String, root: String, config: String) -> Self {
        Graft {
            id: None,
            name,
            root,
            config,
        }
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_id(&self) -> &Option<ConfigId> {
        &self.id
    }

    pub fn set_id(&mut self, id: ConfigId) {
        self.id = Some(id);
    }
}

// TODO EvalContext
#[derive(Clone, Debug)]
pub struct EvalContext {
    pub config: ConfigId,
    pub tree: Option<TreeIndex>,
    pub garden: Option<GardenIndex>,
    pub group: Option<GroupIndex>,
}

impl_display_brief!(EvalContext);

impl EvalContext {
    /// Construct a new EvalContext.
    pub fn new(
        config: ConfigId,
        tree: Option<TreeIndex>,
        garden: Option<GardenIndex>,
        group: Option<GroupIndex>,
    ) -> Self {
        EvalContext {
            config,
            tree,
            garden,
            group,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TreeContext {
    pub tree: TreeIndex,
    pub config: Option<ConfigId>,
    pub garden: Option<GardenIndex>,
    pub group: Option<GroupIndex>,
}

impl_display_brief!(TreeContext);

impl TreeContext {
    /// Construct a new TreeContext.
    pub fn new(
        tree: TreeIndex,
        config: Option<ConfigId>,
        garden: Option<GardenIndex>,
        group: Option<GroupIndex>,
    ) -> Self {
        TreeContext {
            tree,
            config,
            garden,
            group,
        }
    }
}

#[derive(Debug, Default)]
pub struct TreeQuery {
    pub query: String,
    pub pattern: glob::Pattern,
    pub is_default: bool,
    pub is_garden: bool,
    pub is_group: bool,
    pub is_tree: bool,
    pub include_gardens: bool,
    pub include_groups: bool,
    pub include_trees: bool,
}

impl_display_brief!(TreeQuery);

impl TreeQuery {
    pub fn new(query: &str) -> Self {
        let mut is_default = false;
        let mut is_tree = false;
        let mut is_garden = false;
        let mut is_group = false;
        let mut include_gardens = true;
        let mut include_groups = true;
        let mut include_trees = true;

        if syntax::is_garden(query) {
            is_garden = true;
            include_groups = false;
            include_trees = false;
        } else if syntax::is_group(query) {
            is_group = true;
            include_gardens = false;
            include_trees = false;
        } else if syntax::is_tree(query) {
            is_tree = true;
            include_gardens = false;
            include_groups = false;
        } else {
            is_default = true;
        }
        let glob_pattern = syntax::trim(query);
        let pattern = glob::Pattern::new(glob_pattern).unwrap();

        TreeQuery {
            query: query.into(),
            is_default,
            is_garden,
            is_group,
            is_tree,
            include_gardens,
            include_groups,
            include_trees,
            pattern,
        }
    }
}

// Commands
#[derive(Clone, Debug)]
pub enum Command {
    Cmd,
    Custom(String),
    Exec,
    Eval,
    Grow,
    Help,
    Init,
    Inspect,
    List,
    Plant,
    Shell,
}

impl std::default::Default for Command {
    fn default() -> Self {
        Command::Help
    }
}

impl_display_brief!(Command);

impl std::str::FromStr for Command {
    type Err = (); // For the FromStr trait

    fn from_str(src: &str) -> Result<Command, ()> {
        match src {
            "cmd" => Ok(Command::Cmd),
            "exec" => Ok(Command::Exec),
            "eval" => Ok(Command::Eval),
            "grow" => Ok(Command::Grow),
            "help" => Ok(Command::Help),
            "init" => Ok(Command::Init),
            "inspect" => Ok(Command::Inspect),
            "list" => Ok(Command::List),
            "ls" => Ok(Command::List),
            "plant" => Ok(Command::Plant),
            "sh" => Ok(Command::Shell),
            "shell" => Ok(Command::Shell),
            _ => Ok(Command::Custom(src.into())),
        }
    }
}

// Is color enabled?
// --color=<auto,on,off> overrides the default "auto" value.

#[derive(Clone, Debug, PartialEq)]
pub enum ColorMode {
    Auto, // "auto" enables color when a tty is detected.
    Off,  // disable color
    On,   // enable color
}

impl ColorMode {
    pub fn is_enabled(&self) -> bool {
        match self {
            ColorMode::Auto => atty::is(atty::Stream::Stdout),
            ColorMode::Off => false,
            ColorMode::On => true,
        }
    }

    pub fn names() -> &'static str {
        "auto, true, false, 1, 0, [y]es, [n]o, on, off, always, never"
    }

    pub fn update(&mut self) {
        if *self == ColorMode::Auto {
            // Speedup future calls to is_enabled() by performing the "auto"
            // atty check once and caching the result.
            if self.is_enabled() {
                *self = ColorMode::On;
            } else {
                *self = ColorMode::Off;
            }
        }

        if *self == ColorMode::Off {
            yansi::Paint::disable();
        }
    }
}

impl std::default::Default for ColorMode {
    fn default() -> Self {
        ColorMode::Auto
    }
}

impl std::str::FromStr for ColorMode {
    type Err = (); // For the FromStr trait

    fn from_str(src: &str) -> Result<ColorMode, ()> {
        match src.to_lowercase().as_ref() {
            "auto" => Ok(ColorMode::Auto),
            "-1" => Ok(ColorMode::Auto),
            "0" => Ok(ColorMode::Off),
            "1" => Ok(ColorMode::On),
            "false" => Ok(ColorMode::Off),
            "true" => Ok(ColorMode::On),
            "never" => Ok(ColorMode::Off),
            "always" => Ok(ColorMode::Off),
            "off" => Ok(ColorMode::Off),
            "on" => Ok(ColorMode::On),
            "n" => Ok(ColorMode::Off),
            "y" => Ok(ColorMode::On),
            "no" => Ok(ColorMode::Off),
            "yes" => Ok(ColorMode::On),
            _ => Err(()),
        }
    }
}

// Color is an alias for yansi::Paint.
pub type Color<T> = yansi::Paint<T>;

pub fn display_missing_tree(tree: &Tree, path: &str, verbose: bool) -> String {
    if verbose {
        format!(
            "{} {}  {} {}",
            Color::black("#").bold(),
            Color::black(&tree.name).bold(),
            Color::black(&path).bold(),
            Color::black("(skipped)").bold()
        )
    } else {
        format!(
            "{} {} {}",
            Color::black("#").bold(),
            Color::black(&tree.name).bold(),
            Color::black("(skipped)").bold()
        )
    }
}

pub fn display_tree(tree: &Tree, path: &str, verbose: bool) -> String {
    if verbose {
        format!(
            "{} {}  {}",
            Color::cyan("#"),
            Color::blue(&tree.name).bold(),
            Color::blue(&path)
        )
    } else {
        format!("{} {}", Color::cyan("#"), Color::blue(&tree.name).bold())
    }
}

/// Print a tree if it exists, otherwise print a missing tree
pub fn print_tree(tree: &Tree, verbose: bool, quiet: bool) -> bool {
    if let Ok(path) = tree.path_as_ref() {
        // Sparse gardens/missing trees are ok -> skip these entries.
        if !std::path::PathBuf::from(&path).exists() {
            if !quiet {
                eprintln!("{}", display_missing_tree(&tree, &path, verbose));
            }
            return false;
        }

        print_tree_details(tree, verbose, quiet);
        return true;
    } else if !quiet {
        eprintln!("{}", display_missing_tree(&tree, "[invalid-path]", verbose));
    }

    false
}

/// Print a tree
pub fn print_tree_details(tree: &Tree, verbose: bool, quiet: bool) {
    if !quiet {
        if let Ok(path) = tree.path_as_ref() {
            eprintln!("{}", display_tree(&tree, &path, verbose));
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CommandOptions {
    pub args: Vec<String>,
    pub debug: Vec<String>,
    pub chdir: String,
    pub color_mode: ColorMode,
    pub filename: Option<std::path::PathBuf>,
    pub filename_str: String,
    pub keep_going: bool,
    pub quiet: bool,
    pub root: String,
    pub subcommand: Command,
    pub variables: Vec<String>,
    pub verbose: bool,
}

impl CommandOptions {
    pub fn new() -> Self {
        CommandOptions::default()
    }

    // Builder function to update verbosity.
    pub fn verbose(mut self, value: bool) -> Self {
        self.verbose = value;
        self
    }

    pub fn update(&mut self) {
        // Allow specifying the config file: garden --config <path>
        if !self.filename_str.is_empty() {
            let path = std::path::PathBuf::from(&self.filename_str);
            if path.exists() {
                let canon = path.canonicalize().unwrap_or(path);
                self.filename = Some(canon);
            } else {
                self.filename = Some(path);
            }
        }

        // Override garden.root: garden --root <path>
        if !self.root.is_empty() {
            // Resolve the "--root" option to an absolute path
            let root_path = std::path::PathBuf::from(&self.root);
            self.root = root_path
                .canonicalize()
                .unwrap_or(root_path)
                .to_string_lossy()
                .into();
        }

        // Change directories before searching for conifgs: garden --chdir <path>
        if !self.chdir.is_empty() {
            if let Err(err) = std::env::set_current_dir(&self.chdir) {
                error!("could not chdir to '{}': {}", self.chdir, err);
            }
        }

        self.color_mode.update();
    }

    pub fn is_debug(&self, name: &str) -> bool {
        self.debug.contains(&name.into())
    }
}

#[derive(Clone, Debug)]
pub struct ApplicationContext {
    pub options: CommandOptions,
    arena: Arena<Configuration>,
    root_id: ConfigId,
}

impl_display!(ApplicationContext);

impl ApplicationContext {
    pub fn new(config: Configuration, options: CommandOptions) -> Self {
        let mut arena = Arena::new();
        let root_id = arena.new_node(config);

        let mut app_context = ApplicationContext {
            arena,
            root_id,
            options,
        };
        // Record the ID in the configuration.
        app_context.get_root_config_mut().set_id(root_id);

        app_context
    }

    pub fn get_config(&self, id: ConfigId) -> &Configuration {
        self.arena.get(id).unwrap().get()
    }

    pub fn get_config_mut(&mut self, id: ConfigId) -> &mut Configuration {
        self.arena.get_mut(id).unwrap().get_mut()
    }

    pub fn get_root_id(&self) -> ConfigId {
        self.root_id
    }

    pub fn get_root_config(&self) -> &Configuration {
        self.get_config(self.get_root_id())
    }

    pub fn get_root_config_mut(&mut self) -> &mut Configuration {
        self.get_config_mut(self.get_root_id())
    }

    /// Add a child Configuration graft onto the parent ConfigId.
    pub fn add_graft(&mut self, parent: ConfigId, config: Configuration) -> ConfigId {
        let graft_id = self.arena.new_node(config); // Take ownership of config.
        parent.append(graft_id, &mut self.arena);

        self.get_config_mut(graft_id).set_id(graft_id);

        graft_id
    }
}
