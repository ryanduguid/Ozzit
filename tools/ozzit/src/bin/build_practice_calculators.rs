// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

fn main() -> std::process::ExitCode {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    ozzit::practice_calculators::cli(&args)
}
