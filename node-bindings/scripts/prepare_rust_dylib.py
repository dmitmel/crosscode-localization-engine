#!/usr/bin/env python3
import argparse
import os
import shutil
import subprocess

if __name__ == "__main__":
  parser = argparse.ArgumentParser()
  parser.add_argument("--os")
  parser.add_argument("--rust_target_dir")
  parser.add_argument("--rust_build_profile")
  parser.add_argument("--rust_dylib_file")
  parser.add_argument("--product_dir")
  args = parser.parse_args()

  main_lib_src = os.path.join(args.rust_target_dir, args.rust_build_profile, args.rust_dylib_file)
  main_lib_dst = os.path.join(args.product_dir, args.rust_dylib_file)
  shutil.copy2(main_lib_src, main_lib_dst)
  if args.os == "win":
    shutil.copy2(main_lib_src + ".lib", main_lib_dst + ".lib")

  if args.os == "mac":
    subprocess.run(
      ["install_name_tool", "-id", "@rpath/" + args.rust_dylib_file, main_lib_dst],
      check=True,
    )

  os.utime(main_lib_dst, None)
