use std::{fs, io::Read};

use age::{Decryptor, Identity};
use color_eyre::eyre::eyre;
use owo_colors::OwoColorize;
use tar::Archive;

use crate::yes_no_prompt;

pub(crate) fn gitenv_decrypt<R: Read>(
    identities: Vec<Box<dyn Identity>>,
    source: R,
    force: bool,
) -> color_eyre::Result<usize> {
    let decryptor = Decryptor::new(source)?;
    let reader = decryptor.decrypt(identities.iter().map(|identity| identity.as_ref()))?;

    let mut count = 0usize;

    for entry in Archive::new(reader).entries()? {
        let mut entry = entry?;
        println!("  {:?} ...", entry.path());
        if !force && fs::exists(entry.path()?)? {
            print!("{}", "File already exists. Overwrite? [y/N] ".yellow());
            if !yes_no_prompt()? {
                return Err(eyre!("Operation aborted."));
            }
        }
        entry.unpack_in(".")?;
        count += 1;
    }

    Ok(count)
}
