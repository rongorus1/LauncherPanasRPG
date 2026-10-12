use std::path::PathBuf;

/// Everything needed to spawn the game process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub java: PathBuf,
    /// Arguments before the main class (memory, `-cp`, natives, user JVM args).
    pub jvm_args: Vec<String>,
    pub main_class: String,
    pub game_args: Vec<String>,
    pub working_dir: PathBuf,
}
