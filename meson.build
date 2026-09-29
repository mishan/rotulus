project('rotulus', 'c',
  version: '0.1.0',
  license: 'LGPL-2.1-or-later',
  meson_version: '>= 1.1',
  default_options: ['buildtype=debugoptimized', 'warning_level=2'],
)

# The shared library's name carries the API version, and its soname the
# ABI version, so a future rotulus-2 installs beside this one.
api_version = '1'
abi_version = '0'
lib_name = 'rotulus-' + api_version

gnome = import('gnome')
pkg = import('pkgconfig')
fs = import('fs')

cc = meson.get_compiler('c')
gtk = dependency('gtk4', version: '>= 4.10')

# ---- the Rust archive -------------------------------------------------

cargo = find_program('cargo')
# See the capi feature in crates/rotulus/Cargo.toml.
features = ['capi']
# GtkAccessibleText, so a screen reader can read the transcript.
if gtk.version().version_compare('>= 4.14')
  features += 'v4_14'
endif
release = get_option('optimization') not in ['0', 'g']

cargo_args = [
  files('build-aux/cargo-build.py'),
  '--cargo', cargo,
  '--source-root', meson.project_source_root(),
  '--target-dir', meson.project_build_root() / 'cargo-target',
  '--output', '@OUTPUT@',
]
if release
  cargo_args += '--release'
endif
cargo_args += ['--features', ','.join(features)]

rust_archive = custom_target('rust-archive',
  output: 'librotulus_rust.a',
  command: [find_program('python3')] + cargo_args,
  console: true,
  # Cargo knows what is stale; it is quick when nothing is.
  build_always_stale: true,
)

# ---- the shared library -----------------------------------------------

# What the Rust standard library links against.
rust_deps = [
  dependency('threads'),
  cc.find_library('dl', required: false),
  cc.find_library('m', required: false),
]

# Export the C API and nothing else: the archive carries the Rust
# standard library and every dependency, whose symbols must not leak into
# the application's namespace.
link_args = []
link_depends = []
version_script = meson.project_source_root() / 'build-aux/rotulus.map'
exported_list = meson.project_source_root() / 'build-aux/rotulus.exp'
if host_machine.system() == 'darwin'
  link_args += ['-Wl,-exported_symbols_list,' + exported_list, '-Wl,-dead_strip']
  link_depends += exported_list
elif cc.has_link_argument('-Wl,--version-script,' + version_script)
  link_args += ['-Wl,--version-script,' + version_script, '-Wl,--gc-sections']
  link_depends += version_script
else
  warning('This linker takes no version script; every symbol will be exported.')
endif

header = files('crates/rotulus/include/rotulus.h')
inc = include_directories('crates/rotulus/include')

librotulus = shared_library(lib_name,
  link_whole: rust_archive,
  dependencies: [gtk] + rust_deps,
  link_args: link_args,
  link_depends: link_depends,
  version: meson.project_version(),
  soversion: abi_version,
  install: true,
)

rotulus_dep = declare_dependency(
  link_with: librotulus,
  include_directories: inc,
  dependencies: gtk,
)
meson.override_dependency(lib_name, rotulus_dep)

install_headers(header, subdir: lib_name)

pkg.generate(librotulus,
  name: 'rotulus',
  filebase: lib_name,
  description: 'A GTK4 scrollback view for text-stream chat',
  subdirs: lib_name,
  requires: 'gtk4',
)

# ---- introspection ----------------------------------------------------

gir = []
introspection = get_option('introspection').require(
  dependency('gobject-introspection-1.0', required: get_option('introspection')).found(),
  error_message: 'introspection needs gobject-introspection',
)
if introspection.allowed()
  gir = gnome.generate_gir(librotulus,
    sources: header,
    namespace: 'Rotulus',
    nsversion: api_version,
    identifier_prefix: 'Rotulus',
    symbol_prefix: 'rotulus',
    includes: ['Gtk-4.0'],
    header: 'rotulus.h',
    include_directories: inc,
    export_packages: lib_name,
    install: true,
    fatal_warnings: get_option('werror'),
  )

  vapigen = find_program('vapigen', required: get_option('vapi'))
  if get_option('vapi').allowed() and vapigen.found()
    gnome.generate_vapi(lib_name,
      sources: gir[0],
      packages: ['gtk4'],
      install: true,
    )
  endif
endif

# ---- translations -----------------------------------------------------

subdir('crates/rotulus/po')

# ---- tests ------------------------------------------------------------

if get_option('tests')
  subdir('tests')
endif
