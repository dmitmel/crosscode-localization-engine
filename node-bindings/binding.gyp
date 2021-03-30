{
    "variables": {
        "rust_target_dir%": "<!(node scripts/normalize-path.js <(module_root_dir)/../target)",
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
            "library_dirs": ["<(rust_target_dir)/<(rust_build_profile)"],
            "conditions": [
                # Taken from <https://github.com/greenheartgames/greenworks/blob/a7a698203b7fc43d156d83a66789c465fb4ae3e2/binding.gyp#L136-L141>
                ['OS=="linux"', {"ldflags": ["-Wl,-rpath,\\$$ORIGIN"]}],
                ['OS=="linux" or OS=="mac"', {"libraries": ["-lcrosslocale"]}],
                ['OS=="win"', {"libraries": ["-lcrosslocale.dll.lib"]}],
            ],
        },
        {
            "target_name": "symlink_rust_dylib",
            "dependencies": ["crosslocale"],
            "type": "none",
            "actions": [
                {
                    "action_name": "symlink_rust_dylib",
                    "inputs": [
                        "<(rust_target_dir)/<(rust_build_profile)/<(rust_dylib_file)"
                    ],
                    "outputs": ["<(PRODUCT_DIR)/<(rust_dylib_file)"],
                    "action": [
                        "node",
                        "scripts/symlink.js",
                        "<@(_inputs)",
                        "<@(_outputs)",
                    ],
                },
            ],
        },
    ],
}
