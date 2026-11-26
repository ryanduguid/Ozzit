// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

fn main() -> std::process::ExitCode {
    // The arguments only select an output path, which the builder checks before
    // writing; the program name (argument 0) is skipped and never trusted.
    // nosemgrep: rust.lang.security.args.args
    let args = std::env::args()
        .skip(1)
        .map(std::ffi::OsString::from)
        .collect::<Vec<_>>();
    ozzit::practice_calculators::cli(&args)
}
