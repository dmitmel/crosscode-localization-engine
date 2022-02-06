{
    "variables": {
        "rust_target_dir%": "<(module_root_dir)/../target",
        "rust_build_profile%": "release",
        "conditions": [
            ['OS=="linux"', {"rust_dylib_file": "libcrosslocale.so"}],
            ['OS=="mac"', {"rust_dylib_file": "libcrosslocale.dylib"}],
            ['OS=="win"', {"rust_dylib_file": "crosslocale.dll"}],
        ],
    },
    "targets": [
        {
            "target_name": "crosslocale",
            "dependencies": ["rust_dylib"],
            "cflags!": ["-fno-exceptions"],
            "cflags_cc!": ["-fno-exceptions"],
            "xcode_settings": {
                "GCC_ENABLE_CPP_EXCEPTIONS": "YES",
                "CLANG_CXX_LIBRARY": "libc++",
                "MACOSX_DEPLOYMENT_TARGET": "10.7",
            },
            "msvs_settings": {
                "VCCLCompilerTool": {
                    "ExceptionHandling": 1,
                },
            },
            "sources": ["addon.cc"],
            "include_dirs": [
                "<(module_root_dir)/node_modules/node-addon-api",
                "<(module_root_dir)/../ffi",
            ],
            "library_dirs": ["<(PRODUCT_DIR)"],
            "conditions": [
                # <https://stackoverflow.com/a/62877273/12005228>
                # <https://github.com/greenheartgames/greenworks/blob/a7a698203b7fc43d156d83a66789c465fb4ae3e2/binding.gyp#L136-L141>
                ['OS=="linux"', {"libraries": ["-Wl,-rpath,\\$$ORIGIN/"]}],
                ['OS=="mac"', {"libraries": ["-Wl,-rpath,@loader_path/"]}],
                ['OS=="linux" or OS=="mac"', {"libraries": ["-lcrosslocale"]}],
                ['OS=="win"', {"libraries": ["-lcrosslocale.dll.lib"]}],
            ],
        },
        {
            "target_name": "rust_dylib",
            "type": "none",
            "actions": [
                {
                    "action_name": "prepare",
                    "inputs": ["<(rust_target_dir)/<(rust_build_profile)/<(rust_dylib_file)"],
                    "outputs": ["<(PRODUCT_DIR)/<(rust_dylib_file)"],
                    "conditions": [
                        ['OS=="win"', {
                            "inputs": ["<(rust_target_dir)/<(rust_build_profile)/<(rust_dylib_file).lib"],
                            "outputs": ["<(PRODUCT_DIR)/<(rust_dylib_file).lib"],
                        }],
                    ],
                    "action": [
                        "python3",
                        "scripts/prepare_rust_dylib.py",
                        # The MSVS generator prepends ..\ to all arguments of
                        # an action for some reason, but, thankfully, it
                        # ignores the ones which look like flags or options.
                        "--os=<(OS)",
                        "--rust_target_dir=<(rust_target_dir)",
                        "--rust_build_profile=<(rust_build_profile)",
                        "--rust_dylib_file=<(rust_dylib_file)",
                        # The /. at the end is another workaround for the
                        # goddamn MSVS generator. Don't even ask.
                        "--product_dir=<(PRODUCT_DIR)/.",
                    ],
                },
            ],
        },
    ],
}
