# patchkc

A small, dependable command-line tool for patching a Linux kernel's
`.config` and managing kernel modules. It's built around a
format-preserving `.config` engine (so applying a patch produces a minimal,
human-reviewable diff against the original instead of a reformatted file),
takes a backup before every write, and prefers raw syscalls for module
load/unload with a fallback to the standard `modprobe`/`insmod`/`rmmod`
tools.

## Commands

```
patchkc [OPTIONS] <COMMAND>

Commands:
  diff     Show differences between the kernel's .config and a patch config
  apply    Apply a patch config's values onto the kernel's .config
  backup   Create a timestamped backup of the kernel .config
  restore  Restore the kernel .config from a backup
  module   Inspect and manage loaded kernel modules
  build    Drive kernel build steps (olddefconfig / modules / modules_install)

Options:
  -k, --kernel-config <PATH>  Path to the kernel's .config file [default: /usr/src/linux/.config]
  -n, --dry-run               Show what would happen without changing anything
  -y, --yes                   Assume "yes" to any confirmation prompt
  -v, --verbose...            Increase output verbosity (-v, -vv)
```

### `diff`

```
patchkc diff -c my-patch.config [--show-matched] [--only-modules] [--only-enabled]
```

Compares every `CONFIG_*` option in the patch config against the kernel's
current `.config`, reporting matches, differences, and patch-only options.
`--only-modules`/`--only-enabled` narrow the report to options the patch
would build as a loadable module (`=m`) or enable at all (`=y`/`=m`).

### `apply`

```
patchkc apply -c my-patch.config
```

Computes the same diff, then (after a confirmation prompt, unless `--yes`
or `--dry-run`) backs up the current `.config` and writes the patched
values in place -- preserving line order, comments, and section banners
that the patch doesn't touch. Requires root.

### `backup` / `restore`

```
patchkc backup
patchkc restore                       # most recent backup
patchkc restore -f <path/to/backup>   # a specific one
```

Backups live in `--backup-dir` (default `/var/backups/patchkc`) as
`<file>.<nanos>.bak`. `restore` itself snapshots the file it's about to
overwrite first, so a bad restore is also undoable.

### `module`

```
patchkc module list
patchkc module status <name>
patchkc module load <name|/path/to/mod.ko> [param=value ...]
patchkc module unload <name> [--force]
```

`list`/`status` read `/proc/modules` directly. `load`/`unload` use the
`finit_module(2)`/`delete_module(2)` syscalls directly when given a `.ko`
file or already-loaded module name, falling back to `modprobe`/`insmod`/
`rmmod` (which additionally resolve dependencies) when that's not possible.
Loading/unloading requires root.

### `build`

```
patchkc build --oldconfig --modules --install [-j8]
```

Runs `make -C <kernel_src> <target>` for each requested step. `--install`
asks for confirmation first, since it touches the live module tree.
Requires root.

## Safety model

* Every write to `.config` is preceded by a timestamped backup (unless
  `--no-backup` is passed to `apply`), and `restore` backs up before it
  overwrites, too.
* Destructive subcommands (`apply`, `backup`, `restore`, `module load`/
  `unload`, `build`) require root and, unless `--yes`/`--dry-run` is set,
  prompt for confirmation.
* `--dry-run` is honoured everywhere a write or external command would
  otherwise happen; nothing is touched, and the action that *would* run is
  logged instead.
* An empty patch config is rejected outright rather than silently reported
  as "no differences" -- it's almost always a wrong path, not a deliberate
  no-op.

## Building

```
cargo build --release
```

The bundled `build` script additionally installs the resulting binary; see
its contents for the install path it uses on this machine.

## Testing

```
cargo test
```

Unit tests cover the `.config` parser/serializer (including round-tripping
verbatim comments and section banners), the diff/apply logic, backup/restore,
and `/proc/modules` parsing.
