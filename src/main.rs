use std::{env::set_current_dir, path::PathBuf};

use clap::{Parser, Subcommand};

use git_env::{RestoreConfig, SaveConfig, gitenv_restore, gitenv_save};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to the git repository.
    #[arg(short, long, value_name = "DIRECTORY")]
    cwdir: Option<PathBuf>,

    /// Which git remote to push to/fetch from.
    #[arg(short, long, value_name = "REMOTE", default_value = "origin")]
    remote: String,

    /// Which git branch to push to/fetch from.
    #[arg(short, long, value_name = "BRANCH")]
    branch: String,

    /// Name of the encrypted archive within the generated git branch.
    #[arg(short, long, value_name = "FILE", default_value = "gitenv-data")]
    encrypted_data: String,

    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Encrypt and backup the files specified by the .gitenv configuration.
    Save {
        /// Path containing the .gitenv configuration.
        #[arg(short = 'C', long, value_name = "FILE", default_value = ".gitenv")]
        config: PathBuf,

        /// Optional URL containing SSH public keys to encrypt the archive with.
        #[arg(short = 'u', long, value_name = "URL")]
        public_keys_url: Vec<String>,

        /// Optional public SSH keys to encrypt the archive with.
        #[arg(short, long, value_name = "FILE")]
        public_key: Vec<PathBuf>,

        /// Optional private SSH keys to encrypt the archive with.
        #[arg(short = 'i', long, value_name = "FILE")]
        private_key: Vec<PathBuf>,

        /// Whether git-env should automatically push the encrypted archive to the remote.
        #[arg(short, long)]
        push: bool,

        /// Skip all prompts when creating the archive.
        #[arg(long)]
        force: bool,
    },
    /// Recover and decrypt the data specified by the gitenv archive.
    Restore {
        /// Private SSH keys to attemtp to decrypt the archive with.
        #[arg(short = 'i', long, value_name = "FILE")]
        private_key: Vec<PathBuf>,

        /// Whether git-env should automatically fetch the encrypted archive from the remote.
        #[arg(short, long)]
        fetch: bool,

        /// Skip all prompts when unpacking the archive.
        #[arg(long)]
        force: bool,
    },
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    if let Some(cwdir) = cli.cwdir {
        set_current_dir(cwdir)?;
    }

    match cli.command {
        CliCommand::Save {
            config,
            public_keys_url,
            private_key,
            public_key,
            push,
            force,
        } => gitenv_save(SaveConfig {
            remote: cli.remote,
            branch: cli.branch,
            encrypted_data: cli.encrypted_data,
            config,
            public_keys_url,
            private_key,
            public_key,
            push,
            force,
        })?,
        CliCommand::Restore {
            private_key,
            fetch,
            force,
        } => gitenv_restore(RestoreConfig {
            remote: cli.remote,
            branch: cli.branch,
            encrypted_data: cli.encrypted_data,
            private_key,
            fetch,
            force,
        })?,
    }

    Ok(())
}
