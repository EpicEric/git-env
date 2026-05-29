use std::path::PathBuf;

use color_eyre::eyre::eyre;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use owo_colors::OwoColorize;
use walkdir::WalkDir;

use crate::yes_no_prompt;

struct QueueItem {
    dir: PathBuf,
    gitignores: Vec<Gitignore>,
    matched: bool,
    gitignored: bool,
}

pub(crate) fn walk_dir(
    main_dir: PathBuf,
    files: &mut Vec<PathBuf>,
    gitenv: &Gitignore,
    gitignore: &Gitignore,
    force: bool,
) -> color_eyre::Result<()> {
    let mut queue: Vec<QueueItem> = vec![QueueItem {
        dir: main_dir,
        gitignores: vec![gitignore.clone()],
        matched: false,
        gitignored: false,
    }];

    while let Some(QueueItem {
        dir,
        gitignores,
        matched,
        gitignored,
    }) = queue.pop()
    {
        for entry in WalkDir::new(dir.clone()).min_depth(1).max_depth(1) {
            let entry = entry?;
            let raw_path = entry.path();
            let path = raw_path
                .strip_prefix("./")
                .unwrap_or(raw_path)
                .to_path_buf();
            let is_dir = path.is_dir();

            let is_matched = (matched && !gitenv.matched(&path, is_dir).is_whitelist())
                || gitenv.matched(&path, is_dir).is_ignore();

            let is_gitignored = (gitignored
                && !gitignores
                    .iter()
                    .any(|gi| gi.matched(&path, is_dir).is_whitelist()))
                || gitignores
                    .iter()
                    .any(|gi| gi.matched(&path, is_dir).is_ignore());

            if is_matched && !force && !is_gitignored {
                if !yes_no_prompt(
                    format!("File '{:?}' is not in a .gitignore. Continue? [y/N] ", path).yellow(),
                )? {
                    return Err(eyre!("Operation aborted."));
                }
            }
            if is_dir {
                let mut child_gitignores = Vec::with_capacity(gitignores.len() + 1);
                child_gitignores.extend(gitignores.iter().cloned());
                let gi_path = path.join(".gitignore");
                if gi_path.exists() {
                    let mut builder = GitignoreBuilder::new(&dir);
                    builder.add(&gi_path);
                    child_gitignores.push(builder.build().unwrap_or(Gitignore::empty()));
                }
                queue.push(QueueItem {
                    dir: path,
                    gitignores: child_gitignores,
                    matched: is_matched,
                    gitignored: is_gitignored,
                });
            } else if is_matched {
                println!("{}", format!("  {}", path.to_string_lossy()).dimmed());
                files.push(path);
            }
        }
    }
    Ok(())
}
