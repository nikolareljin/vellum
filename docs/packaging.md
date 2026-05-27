# Packaging — DEB and Homebrew

This document covers how to build, install, and publish vellum as a native
system package on Debian/Ubuntu (`.deb`) and macOS/Linux via Homebrew.

---

## DEB Package (Debian / Ubuntu)

### What gets built

`cargo-deb` reads `[package.metadata.deb]` in `Cargo.toml` and produces a
standard `.deb` archive containing:

| Path | Content |
|------|---------|
| `/usr/bin/vellum` | Release binary (stripped) |
| `/usr/share/doc/vellum/README.md` | Upstream README |

### Build locally

```bash
# 1. Install cargo-deb (one-time)
cargo install cargo-deb --locked

# 2. Build the .deb
cargo deb

# Output: target/debian/vellum_<version>_<arch>.deb
```

### Install the package

```bash
sudo dpkg -i target/debian/vellum_0.5.0_amd64.deb
# or
sudo apt install ./target/debian/vellum_0.5.0_amd64.deb   # resolves deps
```

### Uninstall

```bash
sudo apt remove vellum
```

### CI / GitHub Actions

The `deb-build` workflow (`.github/workflows/deb-build.yml`) triggers
automatically on any semver tag push (`0.*.*`, `0.*.*-rc*`).

It:
1. Checks out the repo with full history + tags
2. Installs `cargo-deb` via `cargo install`
3. Runs `cargo deb` → produces `target/debian/*.deb`
4. Uploads the `.deb` as a workflow artifact named `deb-packages`

To trigger manually: **Actions → deb-build → Run workflow**.

### Package metadata

Configured in `Cargo.toml` under `[package.metadata.deb]`:

```toml
[package.metadata.deb]
name            = "vellum"
maintainer      = "Nik Reljin <nikola.reljin@gmail.com>"
copyright       = "2026, Nik Reljin"
license-file    = ["LICENSE", "4"]
extended-description = "..."
depends         = "$auto"          # auto-detect shared-library deps
section         = "utils"
priority        = "optional"
assets          = [
  ["target/release/vellum", "usr/bin/vellum", "755"],
  ["README.md",             "usr/share/doc/vellum/README.md", "644"],
]
```

`$auto` tells `cargo-deb` to run `dpkg-shlibdeps` and fill in the
`shlibs:Depends` field automatically.

### `debian/` directory

A minimal `debian/` tree is also present for use with the standard
`dpkg-buildpackage` toolchain (e.g., PPA submissions, `sbuild` chroots):

```
debian/
  changelog    # Debian-format changelog (dch-compatible)
  control      # Source + binary package metadata
  copyright    # DEP-5 machine-readable copyright
  compat       # debhelper compat level (13)
  rules        # Build rules (overrides dh_auto_build → cargo build --release)
```

To build with `dpkg-buildpackage` instead of `cargo-deb`:

```bash
# Requires: debhelper, devscripts, rustc, cargo, libssl-dev
sudo apt install debhelper devscripts rustc cargo libssl-dev
dpkg-buildpackage -us -uc
# .deb lands in ../
```

---

## Homebrew Formula (macOS / Linux)

### What gets built

The formula installs `vellum` binary via `cargo install`, using the
source tarball from the GitHub release tag.

### Install (once published to a tap)

```bash
# Add the tap (one-time)
brew tap nikolareljin/vellum https://github.com/nikolareljin/homebrew-vellum

# Install
brew install vellum

# Upgrade
brew upgrade vellum

# Uninstall
brew uninstall vellum
```

> **Note:** A Homebrew tap repo (`nikolareljin/homebrew-vellum`) has not yet
> been created. Until then, install from the formula file directly (see below).

### Install from local formula

```bash
brew install --formula packaging/homebrew/vellum.rb
```

### Build / test the formula locally

```bash
# Audit formula for style
brew audit --new packaging/homebrew/vellum.rb

# Install + run test block
brew install --build-from-source packaging/homebrew/vellum.rb
brew test vellum
```

### Formula structure

The formula lives at `packaging/homebrew/vellum.rb`:

```ruby
class Vellum < Formula
  desc     "Rich Markdown viewer for the terminal"
  homepage "https://github.com/nikolareljin/vellum"
  url      "https://github.com/nikolareljin/vellum/archive/refs/tags/0.5.0.tar.gz"
  sha256   "<sha256 of source tarball>"
  version  "0.5.0"
  license  "MIT"

  depends_on "rust" => :build   # build-only dep; not installed at runtime

  def install
    system "cargo", "install", "--locked", "--root", prefix, "--path", "."
  end

  test do
    assert_match "vellum", shell_output("#{bin}/vellum --help")
  end
end
```

### `VERSION` file

`homebrew-package.yml` reads `VERSION` from the repo root to determine the
current version when building the formula tarball. Keep it in sync with
`Cargo.toml`:

```
0.5.0
```

### CI / GitHub Actions

The `homebrew-package` workflow (`.github/workflows/homebrew-package.yml`)
triggers on semver tag pushes.

It:
1. Reads `VERSION` to determine the release version
2. Builds a source tarball via `build_brew_tarball.sh`
3. Generates `packaging/homebrew/vellum.rb` with correct SHA-256 via
   `gen_brew_formula.sh`
4. Uploads formula + tarball as the `homebrew-artifacts` artifact
5. (Optional) Publishes the formula to a tap repo when `publish: "true"` is
   set and `TAP_TOKEN` secret is configured

To trigger manually: **Actions → homebrew-package → Run workflow**.

### Publishing to a tap (future)

1. Create repo `nikolareljin/homebrew-vellum` on GitHub
2. Add `TAP_TOKEN` secret (PAT with `repo` scope) to the vellum repo
3. In `homebrew-package.yml`, set:
   ```yaml
   publish: "true"
   tap_repo: "nikolareljin/homebrew-vellum"
   ```
4. The workflow will commit the updated formula to the tap automatically.

---

## Release checklist

When cutting a new release:

1. Bump `version` in `Cargo.toml`
2. Update `VERSION` file to match
3. Add entry to `CHANGELOG.md`
4. Update `debian/changelog` (use `dch -v <version>-1` or edit manually)
5. Commit: `git commit -m "chore: bump to <version>"`
6. Tag: `git tag <version>` (no `v` prefix — see tagging convention)
7. Push tag: `git push origin <version>`
8. CI triggers: `rust.yml`, `rust-scan.yml`, `release.yml`, `deb-build.yml`, `homebrew-package.yml`
