{
    "variables": {
        "rust_target_dir%": "<!(node -p \"require('path').normalize(process.argv[1])\" <(module_root_dir)/../target)",
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
                "<!@(node -p \"require('node-addon-api').include_dir\")",
                "<(module_root_dir)/../ffi",
            ],
            "libraries": ["-lcrosslocale"],
            "library_dirs": ["<(rust_target_dir)/<(rust_build_profile)"],
            "conditions": [
                # Taken from <https://github.com/greenheartgames/greenworks/blob/a7a698203b7fc43d156d83a66789c465fb4ae3e2/binding.gyp#L136-L141>
                ['OS=="linux"', {"ldflags": ["-Wl,-rpath,\\$$ORIGIN"]}],
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
                    "action": ["ln", "-sfT", "<@(_inputs)", "<@(_outputs)"],
                },
            ],
        },
    ],
}
