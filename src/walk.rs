use std::path::PathBuf;

use color_eyre::eyre::eyre;
use ignore::gitignore::Gitignore;
use owo_colors::OwoColorize;
use walkdir::WalkDir;

use crate::yes_no_prompt;

pub(crate) fn walk_dir(
    main_dir: PathBuf,
    files: &mut Vec<PathBuf>,
    gitenv: &Gitignore,
    gitignore: &Gitignore,
    force: bool,
) -> color_eyre::Result<()> {
    let mut recursive_dirs: Vec<PathBuf> = vec![main_dir];
    let mut matched = false;

    while !recursive_dirs.is_empty() {
        for dir in recursive_dirs.split_off(0) {
            for entry in WalkDir::new(dir).min_depth(1).max_depth(1) {
                let entry = entry?;
                let path = entry.path();

                if gitenv.matched(path, path.is_dir()).is_whitelist() {
                    continue;
                }
                if matched || gitenv.matched(path, path.is_dir()).is_ignore() {
                    if !matched && !force {
                        // Compare files to .gitignore
                        if !gitignore.matched(path, path.is_dir()).is_ignore() {
                            if !yes_no_prompt(
                                format!(
                                    "File '{:?}' is not in the .gitignore. Continue? [y/N] ",
                                    path
                                )
                                .yellow(),
                            )? {
                                return Err(eyre!("Operation aborted."));
                            }
                        }
                    }
                    if path.is_dir() {
                        recursive_dirs.push(path.to_path_buf());
                    } else {
                        println!("{}", format!("  {:?}", path).dimmed());
                        files.push(path.to_path_buf());
                    }
                }
            }
        }
        matched = true;
    }
    Ok(())
}
