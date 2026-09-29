# The smoke tests need a display: run them under xvfb-run, or a desktop.

test('smoke-c',
  executable('smoke', 'smoke.c', dependencies: rotulus_dep),
  suite: 'display',
)

if host_machine.system() == 'linux'
  test('exports',
    find_program('python3'),
    args: [files('check-exports.py'), librotulus],
    depends: librotulus,
  )
endif

if gir.length() > 0
  gi_env = environment()
  gi_env.prepend('GI_TYPELIB_PATH', meson.project_build_root())
  gi_env.prepend('LD_LIBRARY_PATH', meson.project_build_root())
  gi_env.prepend('DYLD_LIBRARY_PATH', meson.project_build_root())

  python = find_program('python3')
  if run_command(python, '-c', 'import gi', check: false).returncode() == 0
    test('smoke-python', python,
      args: files('smoke.py'),
      env: gi_env,
      depends: gir,
      suite: 'display',
    )
  endif

  gjs = find_program('gjs', required: false)
  if gjs.found()
    test('smoke-gjs', gjs,
      args: ['-m', files('smoke.js')],
      env: gi_env,
      depends: gir,
      suite: 'display',
    )
  endif
endif
