use std::{
    fs,
    io::{BufReader, Read, Write, stdin, stdout},
    path::PathBuf,
    str::FromStr,
};

use age::{Identity, Recipient};
use color_eyre::eyre::{Context, eyre};
use ignore::gitignore::GitignoreBuilder;
use owo_colors::OwoColorize;
use ssh_key::PrivateKey;

mod walk;

use crate::walk::walk_dir;

pub(crate) fn yes_no_prompt<S: ToString>(prompt: S) -> color_eyre::Result<bool> {
    print!("{}", prompt.to_string());
    stdout().flush()?;
    let mut buf = String::new();
    stdin().read_line(&mut buf)?;
    if buf.trim().to_lowercase().starts_with("y") {
        Ok(true)
    } else {
        Ok(false)
    }
}

pub struct SaveConfig {
    pub dry_run: bool,
    pub remote: String,
    pub branch: String,
    pub encrypted_data: String,
    pub commit_message: String,
    pub config: PathBuf,
    pub public_keys_url: Vec<String>,
    pub public_key: Vec<PathBuf>,
    pub private_key: Vec<PathBuf>,
    pub push: bool,
    pub force: bool,
}

pub fn gitenv_save(config: SaveConfig) -> color_eyre::Result<()> {
    let repo = git2::Repository::open_from_env()?;
    let signature = git2::Signature::now("git-env", "git-env@local")?;

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
                        .map_err(|error| {
                            eyre!("Failed to parse SSH public key from URL: {:?}", error)
                        })
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
    println!(
        "Using {} {} to encrypt...",
        recipients.len(),
        if recipients.len() == 1 { "key" } else { "keys" },
    );

    // Check if local branch exists
    if !config.dry_run
        && !config.force
        && repo
            .find_branch(&config.branch, git2::BranchType::Local)
            .is_ok()
    {
        if !yes_no_prompt(
            format!(
                "Local branch '{}' already exists. Overwrite? [y/N] ",
                config.branch
            )
            .yellow(),
        )? {
            return Err(eyre!("Operation aborted."));
        }
    }

    // Parse .gitignore and .gitenv files
    let mut gitignore_builder = GitignoreBuilder::new(".");
    gitignore_builder.add(".gitignore");
    let gitignore = gitignore_builder.build()?;
    let mut gitenv_builder = GitignoreBuilder::new(".");
    gitenv_builder.add(&config.config);
    let gitenv = gitenv_builder.build()?;

    // Populate files from .gitenv
    let mut files: Vec<PathBuf> = Vec::new();
    walk_dir(
        PathBuf::from_str(".")?,
        &mut files,
        &gitenv,
        &gitignore,
        config.force,
    )?;
    if files.is_empty() {
        return Err(eyre!("No files matched the git-env configuration"));
    }
    println!(
        "Encrypting {} {}...",
        files.len(),
        if files.len() == 1 { "file" } else { "files" },
    );

    // Create commit containing the archive
    if !config.dry_run {
        let mut treebuilder = repo.treebuilder(None)?;
        let mut blob_writer =
            repo.blob_writer(Some(&PathBuf::from_str(&config.encrypted_data)?))?;
        gitenv_encrypt(recipients, &mut blob_writer, files)?;
        treebuilder.insert(config.encrypted_data, blob_writer.commit()?, 0o100644)?;
        let commit = repo.commit(
            None,
            &signature,
            &signature,
            &config.commit_message,
            &repo.find_tree(treebuilder.write()?)?,
            &[],
        )?;
        repo.branch(&config.branch, &repo.find_commit(commit)?, true)?;
    }

    if config.push {
        if !config.dry_run {
            let status = std::process::Command::new("git")
                .args([
                    "fetch",
                    &config.remote,
                    &format!(
                        "refs/heads/{}:refs/remotes/{}/{}",
                        config.branch, config.remote, config.branch
                    ),
                ])
                .current_dir(repo.workdir().unwrap())
                .status()?;

            if !status.success() {
                return Err(eyre!(
                    "git fetch exited with status code {:?}",
                    status.code()
                ));
            }

            let remote_branch_name = format!("{}/{}", config.remote, config.branch);
            if !config.force
                && repo
                    .find_branch(&remote_branch_name, git2::BranchType::Remote)
                    .is_ok()
            {
                if !yes_no_prompt(
                    format!(
                        "Remote branch '{}' already exists. Overwrite? [y/N] ",
                        config.branch
                    )
                    .yellow(),
                )? {
                    return Err(eyre!("Operation aborted."));
                }
            }
        }
        if config.dry_run {
            println!(
                "{}",
                format!(
                    "Not pushing to remote branch '{}' due to dry run.",
                    config.branch
                )
                .cyan()
            );
        } else {
            let status = std::process::Command::new("git")
                .args(["push", "--force-with-lease", &config.remote, &config.branch])
                .current_dir(repo.workdir().unwrap())
                .status()?;
            if !status.success() {
                return Err(eyre!(
                    "git push exited with status code {:?}",
                    status.code()
                ));
            }
            println!(
                "{}",
                format!("Pushed to remote branch '{}'.", config.branch).green()
            );
        }
    } else if config.dry_run {
        println!(
            "{}",
            format!(
                "Not saving to local branch '{}' due to dry run.",
                config.branch
            )
            .cyan()
        );
    } else {
        println!(
            "{}",
            format!("Saved to local branch '{}'.", config.branch).green()
        );
    }

    Ok(())
}

pub(crate) fn gitenv_encrypt<W: Write>(
    recipients: Vec<Box<dyn Recipient>>,
    destination: W,
    files: Vec<PathBuf>,
) -> color_eyre::Result<()> {
    let encryptor =
        age::Encryptor::with_recipients(recipients.iter().map(|recipient| recipient.as_ref()))?;
    let writer = encryptor.wrap_output(destination)?;

    let mut archive = tar::Builder::new(writer);
    for file in files {
        archive.append_path(&file)?;
    }
    archive.into_inner()?.finish()?;

    Ok(())
}

pub struct RestoreConfig {
    pub dry_run: bool,
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
    println!(
        "Trying to decrypt with {} {}...",
        identities.len(),
        if identities.len() == 1 { "key" } else { "keys" },
    );

    let commit = if config.fetch {
        let status = std::process::Command::new("git")
            .args([
                "fetch",
                &config.remote,
                &format!(
                    "refs/heads/{}:refs/remotes/{}/{}",
                    config.branch, config.remote, config.branch
                ),
            ])
            .current_dir(repo.workdir().unwrap())
            .status()?;

        if !status.success() {
            return Err(eyre!(
                "git fetch exited with status code {:?}",
                status.code()
            ));
        }

        let remote_branch_name = format!("{}/{}", config.remote, config.branch);
        let branch = repo.find_branch(&remote_branch_name, git2::BranchType::Remote)?;
        branch.into_reference().peel_to_commit()?
    } else {
        let branch = repo.find_branch(&config.branch, git2::BranchType::Local)?;
        branch.into_reference().peel_to_commit()?
    };

    let tree_entry = commit
        .tree()?
        .get_path(&PathBuf::from_str(&config.encrypted_data)?)?;
    let decrypt_count = gitenv_decrypt(
        identities,
        repo.find_blob(tree_entry.id())?.content(),
        config.force,
        config.dry_run,
    )?;

    if config.dry_run {
        println!(
            "{}",
            format!(
                "Not decrypting {} {} due to dry run.",
                decrypt_count,
                if decrypt_count == 1 { "file" } else { "files" },
            )
            .cyan()
        );
    } else {
        println!(
            "{}",
            format!(
                "Decrypted {} {}.",
                decrypt_count,
                if decrypt_count == 1 { "file" } else { "files" },
            )
            .green()
        );
    }

    Ok(())
}

pub(crate) fn gitenv_decrypt<R: Read>(
    identities: Vec<Box<dyn Identity>>,
    source: R,
    force: bool,
    dry_run: bool,
) -> color_eyre::Result<usize> {
    let decryptor = age::Decryptor::new(source)?;
    let reader = decryptor.decrypt(identities.iter().map(|identity| identity.as_ref()))?;

    let mut count = 0usize;

    for entry in tar::Archive::new(reader).entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        println!("{}", format!("  {}", path.to_string_lossy()).dimmed());
        if !dry_run && !force && fs::exists(path)? {
            if !yes_no_prompt("File already exists. Overwrite? [y/N] ".yellow())? {
                return Err(eyre!("Operation aborted."));
            }
        }
        if !dry_run {
            entry.unpack_in(".")?;
        }
        count += 1;
    }

    Ok(count)
}
