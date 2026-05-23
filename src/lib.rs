use std::{
    fs,
    io::{BufReader, stdin},
    path::PathBuf,
    str::FromStr,
};

use age::{Identity, Recipient};
use color_eyre::eyre::{Context, OptionExt, eyre};
use ignore::{
    WalkBuilder,
    gitignore::{GitignoreBuilder, gitconfig_excludes_path},
};
use owo_colors::OwoColorize;
use ssh_key::PrivateKey;

use crate::{decrypt::gitenv_decrypt, encrypt::gitenv_encrypt};

mod decrypt;
mod encrypt;

pub(crate) fn yes_no_prompt() -> color_eyre::Result<bool> {
    let mut buf = String::new();
    stdin().read_line(&mut buf)?;
    if buf.to_lowercase() == "y" {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub struct SaveConfig {
    pub remote: String,
    pub branch: String,
    pub encrypted_data: String,
    pub config: PathBuf,
    pub public_keys_url: Vec<String>,
    pub public_key: Vec<PathBuf>,
    pub private_key: Vec<PathBuf>,
    pub push: bool,
    pub force: bool,
}

pub fn gitenv_save(config: SaveConfig) -> color_eyre::Result<()> {
    let repo = git2::Repository::open_from_env()?;
    let git_config = git2::Config::open_default()?;
    let signature = git2::Signature::now(
        git_config.get_str("user.name")?,
        git_config.get_str("user.email")?,
    )?;

    // Get age recipients
    let mut recipients: Vec<Box<dyn Recipient>> = Vec::new();
    let public_key_recipients: color_eyre::Result<Vec<_>> = config
        .public_key
        .into_iter()
        .map(|key| {
            let key_string =
                fs::read_to_string(key).with_context(|| "Failed to open public key")?;
            let recipient = age::ssh::Recipient::from_str(key_string.trim())
                .map_err(|error| eyre!("Failed to parse SSH public key: {:?}", error))?;
            Ok(Box::new(recipient) as Box<dyn Recipient>)
        })
        .collect();
    recipients.extend(public_key_recipients?.into_iter());
    let private_key_recipients: color_eyre::Result<Vec<_>> = config
        .private_key
        .into_iter()
        .map(|key| {
            let key_string =
                fs::read_to_string(key).with_context(|| "Failed to open private key")?;
            let private_key = PrivateKey::from_openssh(key_string.trim())?;
            let recipient = age::ssh::Recipient::from_str(&private_key.public_key().to_openssh()?)
                .map_err(|error| {
                    eyre!(
                        "Failed to parse SSH public key from private key: {:?}",
                        error
                    )
                })?;
            Ok(Box::new(recipient) as Box<dyn Recipient>)
        })
        .collect();
    recipients.extend(private_key_recipients?.into_iter());
    let public_keys_url_recipients: color_eyre::Result<Vec<Box<dyn Recipient>>> = config
        .public_keys_url
        .into_iter()
        .map(|url| -> color_eyre::Result<Vec<Box<dyn Recipient>>> {
            let body = reqwest::blocking::get(url)
                .with_context(|| "Failed to fetch URL")?
                .text()
                .with_context(|| "Failed to read response body")?;

            body.trim()
                .lines()
                .map(|key| {
                    age::ssh::Recipient::from_str(key.trim())
                        .map(|recipient| Box::new(recipient) as Box<dyn Recipient>)
                        .map_err(|error| eyre!("Failed to parse SSH public key: {:?}", error))
                })
                .collect()
        })
        .flat_map(|result| match result {
            Ok(recipients) => recipients.into_iter().map(Ok).collect::<Vec<_>>(),
            Err(e) => vec![Err(e)],
        })
        .collect();
    recipients.extend(public_keys_url_recipients?.into_iter());
    if recipients.is_empty() {
        return Err(eyre!("No keys provided."));
    }

    // Check if local branch exists
    if !config.force
        && repo
            .find_branch(&config.branch, git2::BranchType::Local)
            .is_ok()
    {
        print!(
            "{}",
            format!(
                "Local branch '{}' already exists. Overwrite? [y/N] ",
                config.branch
            )
            .yellow()
        );
        if !yes_no_prompt()? {
            return Err(eyre!("Operation aborted."));
        }
    }

    let gitignore =
        GitignoreBuilder::new(gitconfig_excludes_path().ok_or_eyre("Missing .gitignore file")?)
            .build()?;
    let gitenv = GitignoreBuilder::new(config.config).build()?;

    // Get files from config
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in WalkBuilder::hidden(&mut WalkBuilder::new("."), false).build() {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        if gitenv.matched(path, false).is_ignore() {
            if !config.force {
                if !gitignore.matched(path, false).is_ignore() {
                    print!(
                        "{}",
                        format!(
                            "File '{:?}' is not in the .gitignore. Continue? [y/N] ",
                            path
                        )
                        .yellow()
                    );
                    if !yes_no_prompt()? {
                        return Err(eyre!("Operation aborted."));
                    }
                }
            }
            files.push(path.to_path_buf());
        }
    }
    if files.is_empty() {
        return Err(eyre!("No files matched the git-env configuration"));
    }

    // Compare files to .gitignore

    // Create commit containing the archive
    let mut treebuilder = repo.treebuilder(None)?;
    let mut blob_writer = repo.blob_writer(Some(&PathBuf::from_str(&config.encrypted_data)?))?;
    gitenv_encrypt(recipients, &mut blob_writer, files)?;
    treebuilder.insert(config.encrypted_data, blob_writer.commit()?, 0o100644)?;
    let commit = repo.commit(
        None,
        &signature,
        &signature,
        "git-env save",
        &repo.find_tree(treebuilder.write()?)?,
        &[],
    )?;
    repo.branch(&config.branch, &repo.find_commit(commit)?, true)?;

    // Push branch to remote
    if config.push {
        // Check if branch exists in remote
        let mut remote = repo.find_remote(&config.remote)?;
        if !config.force
            && repo
                .find_branch(&config.branch, git2::BranchType::Remote)
                .is_ok()
        {
            print!(
                "{}",
                format!(
                    "Remote branch '{}' already exists. Overwrite? [y/N] ",
                    config.branch
                )
                .yellow()
            );
            if !yes_no_prompt()? {
                return Err(eyre!("Operation aborted."));
            }
        }
        remote.push(
            &[format!(
                "refs/heads/{}:refs/heads/{}",
                config.branch, config.branch
            )],
            None,
        )?;
        println!(
            "{}",
            format!("Pushed to remote branch '{}'.", config.branch).green()
        );
    } else {
        println!(
            "{}",
            format!("Saved to local branch '{}'.", config.branch).green()
        );
    }

    Ok(())
}

pub struct RestoreConfig {
    pub remote: String,
    pub branch: String,
    pub encrypted_data: String,
    pub private_key: Vec<PathBuf>,
    pub fetch: bool,
    pub force: bool,
}

pub fn gitenv_restore(config: RestoreConfig) -> color_eyre::Result<()> {
    let repo = git2::Repository::open_from_env()?;

    // Get age identities
    let identities: color_eyre::Result<Vec<_>> = config
        .private_key
        .into_iter()
        .map(|key| {
            let identity = age::ssh::Identity::from_buffer(
                BufReader::new(fs::File::open(&key)?),
                key.to_str().map(|str| str.to_string()),
            )?;
            Ok(Box::new(identity) as Box<dyn Identity>)
        })
        .collect();
    let identities = identities?;
    if identities.is_empty() {
        return Err(eyre!("No keys provided."));
    }

    let commit = if config.fetch {
        // Fetch branch from remote
        let mut remote = repo.find_remote(&config.remote)?;

        remote.fetch(
            &[format!(
                "refs/heads/{}:refs/remotes/{}/{}",
                config.branch, config.remote, config.branch
            )],
            None,
            None,
        )?;

        let branch = repo.find_branch(&config.branch, git2::BranchType::Remote)?;
        branch.into_reference().peel_to_commit()?
    } else {
        // Use local branch
        let branch = repo.find_branch(&config.branch, git2::BranchType::Local)?;
        branch.into_reference().peel_to_commit()?
    };

    // Decrypt archive from commit
    let tree_entry = commit
        .tree()?
        .get_path(&PathBuf::from_str(&config.encrypted_data)?)?;
    gitenv_decrypt(
        identities,
        repo.find_blob(tree_entry.id())?.content(),
        config.force,
    )?;

    Ok(())
}
