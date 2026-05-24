# git-env

CLI to encrypt and decrypt sensitive data in a git repository.

Simply drop a `.gitenv` file containing your gitignore'd secrets (with the same format as `.gitignore`), specify your encryption keys, and a branch containing your archive will be created for you.

You can also push and fetch from your git remote, making it easy to share your secrets with your different machines or even with coworkers!

## Installation

git-env requires Cargo.

```bash
cargo install --locked --git https://github.com/EpicEric/git-env.git
```

## Quickstart

```bash
echo ".gitenv" >> .gitignore
echo ".env" > .gitenv

git-env save -r origin -b gitenv/my-secrets -k ~/.ssh/id_ed25519.pub -p

git-env restore -r origin -b gitenv/my-secrets -i ~/.ssh/id_ed25519 -f
```

## Creating an archive

```
$ git-env save --help
Encrypt and backup the files specified by the .gitenv configuration

Usage: git-env save [OPTIONS] --branch <BRANCH>

Options:
  -c, --cwdir <DIRECTORY>      Path to the git repository
      --dry-run                Don't make changes, simply print to console
  -r, --remote <REMOTE>        Which git remote to push to/fetch from [default: origin]
  -b, --branch <BRANCH>        Which git branch to push to/fetch from
  -e, --encrypted-data <FILE>  Name of the encrypted archive within the generated git branch [default: gitenv-data]
  -C, --config <FILE>          Path containing the .gitenv configuration [default: .gitenv]
  -u, --public-keys-url <URL>  Optional URL(s) containing SSH public key(s) to encrypt the archive with
  -k, --public-key <FILE>      Optional public SSH key(s) to encrypt the archive with
  -i, --private-key <FILE>     Optional private SSH key(s) to encrypt the archive with
  -p, --push                   Whether git-env should automatically push the encrypted archive to the remote
      --force                  Skip all prompts when creating the archive
  -h, --help                   Print help
```

## Restoring an archive

```
$ git-env restore --help
Recover and decrypt the data specified by the gitenv archive

Usage: git-env restore [OPTIONS] --branch <BRANCH>

Options:
  -c, --cwdir <DIRECTORY>      Path to the git repository
      --dry-run                Don't make changes, simply print to console
  -r, --remote <REMOTE>        Which git remote to push to/fetch from [default: origin]
  -b, --branch <BRANCH>        Which git branch to push to/fetch from
  -e, --encrypted-data <FILE>  Name of the encrypted archive within the generated git branch [default: gitenv-data]
  -i, --private-key <FILE>     Private SSH key(s) to attempt to decrypt the archive with
  -f, --fetch                  Whether git-env should automatically fetch the encrypted archive from the remote
      --force                  Skip all prompts when unpacking the archive
  -h, --help                   Print help
```
