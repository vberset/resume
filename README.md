# Résumé

Changelog generation and aggregation from git logs.

## Introduction

*résumé* traverses trees or partial-trees of git log to build a changelog based on commit message using the
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0) convention
and [git trailers](https://git-scm.com/docs/git-interpret-trailers).

## Usage

### Résume a local repository

```shell
$ resume repository <repository path>
```

### Résume *projects*

```shell
$ resume projects 
```
## Configuration

By default, the `projects` subcommand load configuration from the `resume.yaml` file in the current folder.

The file must contains a `projects` root attribute with any number of project objects, made of:
* a name
* an origin's url
* a list of branches to watch


Example:
```yaml
projects:
  - name: resume
    origin: https://github.com/vberset/resume.git
    branches:
      - master
```

## Git Configuration

To take advantage of the filtering feature, you can configure git to add the required trailer on each commit
automatically.

### User Setup

1. Configure git to handle both `<token>: <value>` and `<token> #<value>` trailer syntaxes:
   ```shell
   $ git config --global trailer.separators ":#"
   ```

2. Define the `team` trailer command:
   ```shell
   $ git config --global trailer.team.cmd "git config user.team"
   $ git config --global trailer.team.ifexists addIfDifferent
   ```

3. Define globally your team membership:
   ````shell
   $ git config --global user.team "<your team tag>"
   ````

   or per project:
   ````shell
   $ git config user.team "<your team tag>"
   
   ````

### Repository Setup

Add commit message hook to your repository. Create the file `.git/hooks/commit-msg` with this content:

```shell
#! /bin/sh

if [ ! -z "`grep -v '^#\|^\s*$'` $1" ]; then # Add the trailer only if the message is not empty
   git interpret-trailers --in-place --trim-empty --trailer team $1
fi
```

Make it executable:

```shell
$ chmod  +x .git/hooks/commit-msg
```

