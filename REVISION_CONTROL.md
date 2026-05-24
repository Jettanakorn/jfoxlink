# JFOXLink 1.0 — Revision Control Setup

## Overview

This document describes the Git-based revision control system for JFOXLink, including branching strategy, commit conventions, and workflow procedures.

## Repository

**Remote**: https://github.com/Jettanakorn/jfoxlink.git

**Clone**:
```bash
git clone https://github.com/Jettanakorn/jfoxlink.git
cd jfoxlink
```

## Branching Strategy

JFOXLink uses **Git Flow** with the following branches:

### Main branches

1. **`master`** — Production-ready code
   - Only merge via pull request from `release/*`
   - Always points to latest stable release
   - Tagged with version numbers (v1.0.0, v1.0.1, etc.)
   - Status: Always passing CI/CD

2. **`develop`** — Integration branch for features
   - Base branch for feature development
   - Staging area for next release
   - Should be stable; breaking changes require review

### Supporting branches

3. **`feature/*`** — Feature development
   - Branch from: `develop`
   - Merge back to: `develop`
   - Naming: `feature/short-description` (e.g., `feature/adaptive-antijam`)
   - Lifetime: Until feature is complete and merged

4. **`bugfix/*`** — Bug fixes
   - Branch from: `develop`
   - Merge back to: `develop`
   - Naming: `bugfix/issue-number-description` (e.g., `bugfix/123-frame-parser`)
   - Lifetime: Until bug is fixed and verified

5. **`hotfix/*`** — Critical production fixes
   - Branch from: `master`
   - Merge back to: `master` and `develop`
   - Naming: `hotfix/issue-brief` (e.g., `hotfix/crypto-tag-overflow`)
   - Lifetime: Critical fixes only; merged immediately

6. **`release/*`** — Release preparation
   - Branch from: `develop`
   - Merge to: `master` and back to `develop`
   - Naming: `release/v1.0.0` (semantic versioning)
   - Lifetime: Until version released and tagged

## Commit Conventions

### Message format

Follow **Conventional Commits**:

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Type

- **feat**: New feature
- **fix**: Bug fix
- **docs**: Documentation update
- **style**: Code style (formatting, whitespace)
- **refactor**: Code restructuring without functional change
- **perf**: Performance improvement
- **test**: Adding or updating tests
- **ci**: CI/CD configuration
- **chore**: Maintenance (dependencies, build tools)

### Scope

Optional. Module or component affected. Examples:
- `feat(frame): add zero-copy parsing`
- `fix(crypto): correct AES-GCM tag handling`
- `docs(hal): add RFD900 driver documentation`

### Subject

- Imperative mood: "add", not "added" or "adds"
- No period at the end
- Lowercase except proper nouns
- Maximum 50 characters

### Body

- Wrapped at 72 characters
- Explain *what* and *why*, not *how*
- Reference issue numbers: "Fixes #123", "Relates to #456"

### Footer

Optional. Additional metadata:
- `Fixes #<issue>` — Closes this issue
- `Relates to #<issue>` — Related context
- `BREAKING CHANGE: <description>` — For major version changes

### Examples

```
feat(channel): add latency-aware failover voting

Implements proportional weighting of channel scores based on
measured latency. Lower latency channels are preferred during
failover arbitration.

Fixes #42
```

```
fix(crypto): correct GenericArray conversion in AES-GCM

The GCM tag was being cast incorrectly, causing tag mismatch
errors on decryption. Now using proper GenericArray::from()
conversion.

Related to #38
```

```
docs(user-manual): expand troubleshooting section

Added common RF interference issues and resolution steps.
```

## Workflow

### Starting a new feature

```bash
# Switch to develop and pull latest
git checkout develop
git pull origin develop

# Create feature branch
git checkout -b feature/my-feature

# Make your changes, test, and commit
git add <files>
git commit -m "feat(scope): description"

# Push to remote
git push origin feature/my-feature
```

### Creating a pull request

1. Visit https://github.com/Jettanakorn/jfoxlink
2. GitHub will suggest "Compare & pull request"
3. Fill in:
   - **Title**: Same as commit subject
   - **Description**: Link to relevant issues, test results, and context
   - **Reviewers**: Assign team members for code review
4. Ensure CI passes
5. Request review and address feedback

### Reviewing a pull request

1. Check code quality:
   ```bash
   cargo clippy --all-targets
   cargo fmt --check
   ```
2. Run tests:
   ```bash
   cargo test --all
   ```
3. Provide constructive feedback
4. Approve and request merge when ready

### Merging to develop

After review approval:

```bash
git checkout develop
git pull origin develop

# Merge with --no-ff to preserve branch history
git merge --no-ff feature/my-feature
git push origin develop

# Delete remote branch
git push origin --delete feature/my-feature

# Delete local branch
git branch -d feature/my-feature
```

Or use GitHub "Squash and merge" for cleaner history.

### Creating a release

1. Prepare release branch:
   ```bash
   git checkout -b release/v1.0.1 develop
   ```

2. Update version numbers in all `Cargo.toml`:
   ```toml
   [package]
   version = "1.0.1"
   ```

3. Update `CHANGELOG.md`:
   ```markdown
   ## v1.0.1 (May 25, 2026)

   ### Fixed
   - Corrected AES-GCM tag handling (#38)
   - Improved failover latency (#42)

   ### Added
   - User manual documentation
   ```

4. Commit:
   ```bash
   git add Cargo.toml CHANGELOG.md
   git commit -m "chore(release): bump to v1.0.1"
   git push origin release/v1.0.1
   ```

5. Create pull request from `release/v1.0.1` → `master`
6. After merge, tag:
   ```bash
   git checkout master
   git pull origin master
   git tag -a v1.0.1 -m "Release v1.0.1"
   git push origin v1.0.1
   ```

7. Merge back to develop:
   ```bash
   git checkout develop
   git pull origin develop
   git merge --no-ff master
   git push origin develop
   ```

## Tags

Production releases are tagged with semantic versioning:

```bash
# List tags
git tag

# View tag details
git show v1.0.0

# Create annotated tag (preferred)
git tag -a v1.0.0 -m "Release v1.0.0"
git push origin v1.0.0
```

## CI/CD

(To be configured)

Recommended GitHub Actions:
1. **Test on push**: `cargo test --all`
2. **Lint**: `cargo clippy`, `cargo fmt --check`
3. **Release build**: `cargo build --release`
4. **Semantic versioning check**: Enforce v X.Y.Z format

## Collaboration guidelines

### Code review checklist

Before approving a PR:

- [ ] Code follows project style guide
- [ ] No `unsafe` blocks (except in `jfl-hal` drivers where needed)
- [ ] No panics in protocol-critical paths
- [ ] All tests pass
- [ ] Commit messages follow convention
- [ ] Documentation updated
- [ ] No unrelated changes in PR

### Handling merge conflicts

```bash
# During merge, conflicts may appear
git merge feature/conflicting-branch

# Resolve conflicts in your editor
# Then:
git add <resolved-files>
git commit -m "merge: resolve conflicts"
git push
```

### Reverting commits

```bash
# Revert a specific commit
git revert <commit-hash>

# This creates a NEW commit that undoes the changes
git push
```

Do NOT use `git reset` on shared branches; revert instead.

## Local setup

### Clone with SSH (recommended)

```bash
# Add SSH key to GitHub (one-time setup)
ssh-keygen -t ed25519 -C "your-email@example.com"
cat ~/.ssh/id_ed25519.pub  # Copy to GitHub → Settings → SSH Keys

# Clone via SSH
git clone git@github.com:Jettanakorn/jfoxlink.git
```

### Configure user

```bash
git config --global user.name "Your Name"
git config --global user.email "your-email@example.com"

# Or per-repository
cd jfoxlink
git config user.name "Your Name"
git config user.email "your-email@example.com"
```

### Useful aliases

Add to `~/.gitconfig`:

```ini
[alias]
    st = status
    co = checkout
    br = branch
    ci = commit
    unstage = reset HEAD --
    last = log -1 HEAD
    visual = log --graph --oneline --all
    feature = checkout -b
```

Usage:
```bash
git feature my-new-feature  # creates and checks out feature/my-new-feature
```

## Documentation

- **README.md**: Project overview and quick start
- **DEVELOPER.md**: Development guidelines and architecture
- **USER_MANUAL.md**: Operational procedures and configuration
- **CHANGELOG.md**: Version history and release notes (to be created)
- **LICENSE**: Legal terms

## Troubleshooting

### Accidental commit to master

```bash
# If not pushed yet
git reset --soft HEAD~1  # Undo commit, keep changes
git checkout develop
git commit -m "..."
git push

# If already pushed (requires admin)
# Contact repository administrator
```

### How to undo local changes

```bash
# Discard changes in working directory
git checkout -- <file>

# Discard all uncommitted changes
git reset --hard HEAD
```

### How to squash commits

```bash
# Interactive rebase to combine last 3 commits
git rebase -i HEAD~3

# In editor, change 'pick' to 'squash' for commits to combine
# Save and close, then amend message as needed
```

## Additional resources

- **Git Flow**: https://nvie.com/posts/a-successful-git-branching-model/
- **Conventional Commits**: https://www.conventionalcommits.org/
- **GitHub Docs**: https://docs.github.com/
- **Pro Git Book**: https://git-scm.com/book/en/v2

## Contact

For questions about revision control procedures, contact the development team or open an issue on GitHub.
