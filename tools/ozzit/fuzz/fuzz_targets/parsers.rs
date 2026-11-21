// Copyright 2026 Ryan Duguid
// SPDX-License-Identifier: Apache-2.0

#![no_main]

#[path = "../harness.rs"]
mod harness;

libfuzzer_sys::fuzz_target!(|data: &[u8]| harness::test_one_input(data));
