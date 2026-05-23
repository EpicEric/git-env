use std::{io::Write, path::PathBuf};

use age::{Encryptor, Recipient};
use tar::Builder;

pub(crate) fn gitenv_encrypt<W: Write>(
    recipients: Vec<Box<dyn Recipient>>,
    destination: W,
    files: Vec<PathBuf>,
) -> color_eyre::Result<()> {
    let encryptor =
        Encryptor::with_recipients(recipients.iter().map(|recipient| recipient.as_ref()))?;
    let writer = encryptor.wrap_output(destination)?;

    let mut archive = Builder::new(writer);
    for file in files {
        archive.append_path(&file)?;
    }
    archive.into_inner()?.finish()?;

    Ok(())
}
