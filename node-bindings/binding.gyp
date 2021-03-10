{
    "targets": [
        {
            "target_name": "crosslocale",
            "cflags!": ["-fno-exceptions"],
            "cflags_cc!": ["-fno-exceptions"],
            "sources": ["addon.cc"],
            "include_dirs": [
                "<!@(node -p \"require('node-addon-api').include_dir\")",
                "<(module_root_dir)/../ffi",
            ],
            "libraries": ["-lcrosslocale"],
            "defines": ["NAPI_DISABLE_CPP_EXCEPTIONS"],
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
        },
    ],
}
