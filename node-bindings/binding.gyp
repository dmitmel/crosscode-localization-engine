{
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
            "variables": {
                "rust_target_dir%": "<(module_root_dir)/../target",
            },
            "configurations": {
                "Debug": {
                    "library_dirs": ["<(rust_target_dir)/debug"],
                },
                "Release": {
                    "library_dirs": ["<(rust_target_dir)/release"],
                },
            },
            "conditions": [
                [
                    'OS=="linux"',
                    {
                        # Taken from <https://github.com/greenheartgames/greenworks/blob/a7a698203b7fc43d156d83a66789c465fb4ae3e2/binding.gyp#L136-L141>
                        "ldflags": ["-Wl,-rpath,\\$$ORIGIN"],
                    },
                ]
            ],
        },
    ]
}
