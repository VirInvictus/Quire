# Meson build wrapper for Quire.
#
# Thin orchestration layer around `cargo build --release` (the house
# GTK-app shape: Atrium, Colophon, Conservatory, Framework). The Rust
# workspace is the source of truth; meson handles the GNOME-shaped
# install layout: the binary into $bindir, the desktop file and
# AppStream metainfo into $datadir, the hicolor icon set, the
# shared-mime-info package for .quire sheets, and the GSettings
# schema (compiled at install time). The app itself stays
# self-contained: the fonts, the language spec, and the schemes
# extract to the user dirs at startup, with or without this install.
#
#   meson setup builddir --prefix=/usr
#   meson compile -C builddir
#   meson install -C builddir

project(
  'quire',
  'rust',
  # Single source of truth - the VERSION file (matched by the
  # workspace Cargo.toml per the release discipline).
  version: files('VERSION'),
  meson_version: '>= 0.59.0',
  license: 'MIT',
)

application_id = 'io.github.virinvictus.Quire'
prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')

gnome = import('gnome')

cargo = find_program('cargo')

# Cargo build target. `build_always_stale` lets cargo handle its own
# incremental logic without meson tracking the Rust source tree.
cargo_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
cargo_options += ['--release']
cargo_options += ['--package', 'quire']

custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: 'quire',
  console: true,
  install: true,
  install_dir: bindir,
  command: [
    'env',
    'CARGO_TARGET_DIR=' + meson.project_build_root() / 'cargo-target',
    cargo, 'build', cargo_options,
    '&&', 'cp', meson.project_build_root() / 'cargo-target' / 'release' / 'quire', '@OUTPUT@',
  ],
)

install_data(
  'data' / (application_id + '.desktop'),
  install_dir: datadir / 'applications',
)
install_data(
  'data' / (application_id + '.metainfo.xml'),
  install_dir: datadir / 'metainfo',
)
install_subdir('data/icons/hicolor', install_dir: datadir)
install_data(
  'data' / 'mime' / 'quire.xml',
  install_dir: datadir / 'mime' / 'packages',
)
# The same file the binary carries with include_bytes!: one source,
# installed for the desktop integration copy.
install_data(
  'crates/quire/resources' / (application_id + '.gschema.xml'),
  install_dir: datadir / 'glib-2.0' / 'schemas',
)

# Skips itself under DESTDIR (what packaging expects); the release
# job compiles the staged schemas by hand so the tarball is complete.
gnome.post_install(
  glib_compile_schemas: true,
  update_desktop_database: true,
  update_mime_database: true,
)
