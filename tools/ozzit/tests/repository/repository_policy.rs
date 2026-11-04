//! Repository policy: contributor guidance, the verify workflow's commands,
//! Dependabot, the type-check pin, `.editorconfig`, the README and the
//! workbook's creator.
//!
//! The release-attestation, signature and citation checks stay in
//! `tools/tests/test_repository_policy.py`: they parse YAML with ruamel.yaml and
//! Python source with `ast`.
use super::{json, parts, read_raw, root, spaced};
use ozzit::{common, py};

const VERIFY_WORKFLOW: &str = ".github/workflows/verify.yml";

const EXPECTED_VERIFY_COMMANDS: [&str; 24] = [
    "python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-ruff.txt",
    "python -m ruff check .",
    "python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-mypy.txt",
    "python -m mypy --config-file mypy.ini",
    "curl --proto '=https' --proto-redir '=https' -sSLO https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.x86_64.tar.xz && echo \"8c3be12b05d5c177a04c29e3c78ce89ac86f1595681cab149b65b97c4e227198  shellcheck-v0.11.0.linux.x86_64.tar.xz\" | sha256sum --check && tar -xJf shellcheck-v0.11.0.linux.x86_64.tar.xz shellcheck-v0.11.0/shellcheck",
    "curl --proto '=https' --proto-redir '=https' -sSLO https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_amd64.tar.gz && echo \"8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8  actionlint_1.7.12_linux_amd64.tar.gz\" | sha256sum --check && tar -xzf actionlint_1.7.12_linux_amd64.tar.gz actionlint",
    "./actionlint -shellcheck ./shellcheck-v0.11.0/shellcheck",
    "python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-citation.txt",
    "python -m unittest discover -s tools/tests -v",
    "rustup toolchain install",
    "cargo fmt --manifest-path tools/ozzit/Cargo.toml --check",
    "cargo clippy --locked --manifest-path tools/ozzit/Cargo.toml --all-targets -- -D warnings",
    "cargo fetch --locked --manifest-path tools/ozzit/fuzz/Cargo.toml",
    "cargo test --locked --manifest-path tools/ozzit/Cargo.toml",
    "cargo build --locked --release --manifest-path tools/ozzit/Cargo.toml",
    "tools/ozzit/target/release/ozzit verify_workbook ozzit.xlsx",
    "tools/ozzit/target/release/ozzit verify_sources ozzit.xlsx src",
    "tools/ozzit/target/release/ozzit verify_signatures src",
    "tools/ozzit/target/release/ozzit verify_previous_names functions.csv",
    "tools/ozzit/target/release/ozzit verify_index ozzit.xlsx src functions.csv",
    "tools/ozzit/target/release/ozzit verify_cover ozzit.xlsx functions.csv CHANGELOG.md",
    "tools/ozzit/target/release/ozzit verify_afe ozzit.xlsx src",
    "tools/ozzit/target/release/ozzit verify_help_spills ozzit.xlsx",
    "tools/ozzit/target/release/ozzit verify_native_evidence",
];

const DEPENDABOT_CANONICAL: &str = r#"version: 2
updates:
  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
    open-pull-requests-limit: 2
    groups:
      # init and analyze are separate dependencies to Dependabot but must
      # move together: a job running one release against the other's config
      # fails outright. Grouped so the pair can never arrive split.
      codeql-action:
        patterns:
          - "github/codeql-action*"
    ignore:
      # The release-policy pin is an immutable full commit SHA, advanced only
      # by a reviewed re-pin of its workflow family. A Dependabot bump adds an
      # unapproved SHA to the portfolio audit without changing the workflow,
      # so re-pin by hand instead.
      - dependency-name: "ryanduguid/release-policy/.github/workflows/*"

  # pip is how the verify workflow installs its Python tools, from the
  # hash-pinned requirements-*.txt files in the repository root. Dependabot
  # reads those files and bumps their pins, such as ruff, mypy and coverage.
  # Grouped into one pull request a week, as the sibling repositories group
  # their Python updates. pyproject.toml carries tool configuration only and
  # declares no dependencies.
  - package-ecosystem: pip
    directory: /
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
    open-pull-requests-limit: 2
    groups:
      python-dependencies:
        patterns:
          - "*"

  - package-ecosystem: cargo
    directory: /tools/ozzit
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
    open-pull-requests-limit: 2
    groups:
      rust-dependencies:
        patterns:
          - "*"

  - package-ecosystem: cargo
    directory: /tools/ozzit/fuzz
    schedule:
      interval: weekly
    cooldown:
      default-days: 7
    open-pull-requests-limit: 2
    groups:
      rust-dependencies:
        patterns:
          - "*"
"#;

/// The number of leading characters `str.lstrip()` removes.
fn indent(line: &str) -> usize {
    line.chars().take_while(|c| py::is_space(*c)).count()
}

fn rstrip(line: &str) -> &str {
    line.trim_end_matches(py::is_space)
}

/// The block scalar's body: its lines from `content_indent`, until a line indented less.
fn block_body(lines: &[&str], mut cursor: usize, content_indent: usize) -> (Vec<String>, usize) {
    let mut body = vec![];
    while let Some(line) = lines.get(cursor) {
        let blank = py::strip(line).is_empty();
        if !blank && indent(line) < content_indent {
            break;
        }
        if !blank {
            body.push(rstrip(&line.chars().skip(content_indent).collect::<String>()).to_owned());
        }
        cursor += 1;
    }
    (body, cursor)
}

/// Every scalar and block `run` gate, read without a YAML dependency.
pub fn workflow_commands(workflow: &str) -> Vec<String> {
    // The aggregate job checks job results, not the repository, so its scripts
    // are not contributor commands; nor are the CI-only coverage jobs after it.
    let end = common::re(r"(?m)^  (?:\w|-)+-gates:$")
        .find(workflow)
        .expect("regular expression execution")
        .map_or(workflow.len(), |m| m.start());
    let lines = py::splitlines(&workflow[..end]);
    let mut commands = vec![];
    let mut index = 0;
    while index < lines.len() {
        let Some(groups) = super::search(r"^\s*(?:-\s+)?run:\s*(.*?)\s*$", lines[index]) else {
            index += 1;
            continue;
        };
        let value = &groups[1];
        if !common::fullmatch(r"[|>][+-]?", value) {
            commands.push(value.clone());
            index += 1;
            continue;
        }
        let at = lines[index].find("run:").expect("a run key");
        let key_indent = lines[index][..at].chars().count();
        let mut cursor = index + 1;
        while cursor < lines.len() && py::strip(lines[cursor]).is_empty() {
            cursor += 1;
        }
        if cursor == lines.len() {
            commands.push(String::new());
            break;
        }
        let content_indent = indent(lines[cursor]);
        if content_indent <= key_indent {
            commands.push(String::new());
            index = cursor;
            continue;
        }
        let (body, next) = block_body(&lines, cursor, content_indent);
        commands.push(body.join(if value.starts_with('|') { "\n" } else { " " }));
        index = next;
    }
    commands
}

#[test]
fn a_block_run_at_the_end_of_the_file_is_empty() {
    assert_eq!(workflow_commands("      - run: |\n\n"), [""]);
}

#[test]
fn a_block_run_without_an_indented_body_is_empty() {
    let workflow = "      - run: |\n      - name: next\n        run: python b.py\n";
    assert_eq!(workflow_commands(workflow), ["", "python b.py"]);
}

#[test]
fn a_block_run_ends_at_the_first_line_indented_less() {
    let workflow = concat!(
        "      - run: |\n          echo a\n\n          echo b\n",
        "      - run: >-\n          python c.py\n          --check\n",
        "      - name: next\n",
    );
    assert_eq!(
        workflow_commands(workflow),
        ["echo a\necho b", "python c.py --check"]
    );
}

#[test]
fn cross_runtime_contributor_guidance_preserves_workbook_authority() {
    for name in ["AGENTS.md", "CLAUDE.md", "CONTRIBUTING.md"] {
        assert!(root().join(name).is_file(), "{name} is required");
    }
    let agents = read_raw("AGENTS.md");
    let contributing = read_raw("CONTRIBUTING.md");
    assert!(agents.contains("`ozzit.xlsx` is the shipped authority"));
    assert!(agents.contains("Never fabricate Excel recalculation or cached-value evidence."));
    assert!(agents.contains("native Excel"));
    assert!(agents.contains("no macros"));
    assert!(agents.contains("RELEASING.md"));
    for document in [&agents, &contributing] {
        let normalised = spaced(document);
        assert!(normalised.contains("`tools/postbuild/README.md`"));
        assert!(normalised.contains("sync the AFE store after any `src/` change"));
        assert!(normalised.contains("sanitise the workbook last"));
    }
    assert_eq!(read_raw("CLAUDE.md"), "@AGENTS.md\n");
    assert!(contributing.contains("`ozzit.xlsx` is the shipped authority"));
    assert!(
        contributing
            .replace('\n', " ")
            .contains("`src/*.txt`, the AFE store and `functions.csv`")
    );
    assert!(contributing.contains(
        "Cached formula results may change only with Excel-backed recalculation evidence."
    ));
}

#[test]
fn the_contributor_guidance_names_every_verify_command() {
    let workflow = read_raw(VERIFY_WORKFLOW);
    let commands = workflow_commands(&workflow);
    assert_eq!(commands, EXPECTED_VERIFY_COMMANDS);
    let mutated = workflow.replacen(
        "tools/ozzit/target/release/ozzit verify_afe ozzit.xlsx src",
        "tools/ozzit/target/release/ozzit verify_afe ozzit.xlsx changed-src",
        1,
    );
    assert_ne!(commands, workflow_commands(&mutated));
    for document in ["AGENTS.md", "CONTRIBUTING.md"] {
        let text = read_raw(document);
        for command in &commands {
            assert!(
                text.contains(command.as_str()),
                "{document} does not name {command:?}"
            );
        }
    }
}

#[test]
fn workflow_command_parser_detects_non_python_and_multiline_gates() {
    let workflow = read_raw(VERIFY_WORKFLOW);
    let marker = "        run: tools/ozzit/target/release/ozzit verify_native_evidence";
    let added = concat!(
        "\n      - name: Check PowerShell policy\n",
        "        run: pwsh -File tools/check_policy.ps1\n",
        "      - name: Check frontend assets\n",
        "        run: |\n",
        "          npm ci\n",
        "          npm test",
    );
    let mutated = workflow.replacen(marker, &format!("{marker}{added}"), 1);
    let commands = workflow_commands(&mutated);
    assert_eq!(
        commands[commands.len() - 2..],
        ["pwsh -File tools/check_policy.ps1", "npm ci\nnpm test"]
    );
}

#[test]
fn security_and_releasing_docs_exist_and_name_the_reporting_path() {
    let security = read_raw("SECURITY.md");
    let releasing = read_raw("RELEASING.md");
    assert!(root().join("SECURITY.md").is_file());
    assert!(root().join("RELEASING.md").is_file());
    assert!(
        security
            .to_lowercase()
            .contains("private vulnerability reporting")
    );
    assert!(security.to_lowercase().contains("synthetic"));
    assert!(releasing.contains("SHA256SUMS"));
    assert!(releasing.to_lowercase().contains("git archive"));
    assert!(releasing.contains("exactly 3 files"));
    assert!(releasing.contains("tools/ozzit/target/release/ozzit prepare_release_bundle create"));
    assert!(releasing.contains("tools/ozzit/target/release/ozzit prepare_release_bundle verify"));
    assert!(!releasing.contains("Ozzit-<version>-source.zip"));
    assert!(!releasing.contains("Ozzit-<version>-verification.txt"));
}

#[test]
fn workflow_and_dependabot_match_reviewed_controls() {
    let workflow = read_raw(VERIFY_WORKFLOW);
    for control in [
        "persist-credentials: false",
        "fetch-depth: 0",
        "tools/ozzit/target/release/ozzit verify_workbook ozzit.xlsx",
        "tools/ozzit/target/release/ozzit verify_sources ozzit.xlsx src",
        "python -m unittest discover -s tools/tests -v",
    ] {
        assert!(workflow.contains(control), "{control}");
    }
    assert_eq!(
        read_raw(".github/dependabot.yml").replace("\r\n", "\n"),
        DEPENDABOT_CANONICAL
    );
}

#[test]
fn production_type_check_is_pinned_and_rejects_untyped_definitions() {
    let workflow = read_raw(VERIFY_WORKFLOW);
    let config = read_raw("mypy.ini");
    assert!(workflow.contains(
        "python -m pip install --only-binary ':all:' --require-hashes --requirement requirements-mypy.txt"
    ));
    assert!(read_raw("requirements-mypy.txt").contains(r"mypy==2.4.0 \"));
    assert!(workflow.contains("python -m mypy --config-file mypy.ini"));
    assert!(config.contains("files = tools"));
    assert!(config.contains(r"exclude = ^tools[/\\]tests[/\\]"));
    assert!(config.contains("disallow_untyped_defs = True"));
    assert!(config.contains("check_untyped_defs = True"));
    assert!(!config.contains("disable_error_code"));
}

#[test]
fn editorconfig_exempts_the_bytes_the_afe_gate_compares() {
    // ozzit verify_afe compares src/*.txt with the workbook's store byte for byte.
    // Under the [*] rules, saving a module in any editorconfig-honouring editor
    // rewrites exactly those bytes and fails the gate with no hint why.
    let config = read_raw(".editorconfig");
    let scoped = config
        .find("[src/*.txt]")
        .expect(".editorconfig does not scope src/*.txt");
    let after = &config[scoped + "[src/*.txt]".len()..];
    let next = common::re(r"(?m)^\[")
        .find(after)
        .expect("regular expression execution");
    let section = &after[..next.map_or(after.len(), |m| m.start())];
    assert!(scoped > config.find("[*]").expect("a [*] section"));
    assert!(section.contains("trim_trailing_whitespace = false"));
    assert!(section.contains("insert_final_newline = false"));
    let modules: Vec<String> = common::txt_files(&root().join("src"))
        .unwrap_or_else(|e| panic!("{e}"))
        .iter()
        .map(|path| common::read_raw(path).unwrap_or_else(|e| panic!("{e}")))
        .collect();
    assert!(modules.len() >= 6, "{} modules", modules.len());
    let padded = modules
        .iter()
        .flat_map(|module| module.split('\n'))
        .filter(|line| *line != rstrip(line))
        .count();
    assert!(
        padded > 1000,
        "the exemption is only needed while padding is"
    );
    assert!(
        modules.iter().any(|module| !module.ends_with('\n')),
        "at least one module ends without a final newline"
    );
}

#[test]
fn readme_links_the_disclaimer() {
    // The disclaimer states what the library is not advice about. A reader who
    // never reaches it is the reader it was written for, so the README links it.
    assert!(
        root().join("DISCLAIMER.md").is_file(),
        "DISCLAIMER.md is required"
    );
    assert!(read_raw("README.md").contains("[DISCLAIMER.md](DISCLAIMER.md)"));
}

/// `f"{n:,}"` for a whole number.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

#[test]
fn readme_publishes_the_tracked_workbook_digest_as_well_as_the_release_asset() {
    // The release asset and the tracked workbook are different files whenever
    // main is ahead of the tag. Publishing only the asset digest left readers
    // unable to check the file they clone, so the README states both and this
    // test keeps the tracked one equal to the manifest the gates already check.
    let base = json(&read_raw("release/workbook-base.json"));
    let readme = read_raw("README.md");
    let digest = base["sha256"].as_str().expect("a digest");
    let size = base["size"].as_u64().expect("a size in bytes");
    assert!(readme.contains(&format!("`{digest}` ({} bytes)", thousands(size))));
    assert!(readme.contains("release/workbook-base.json"));
}

#[test]
fn workbook_creator_credits_the_project() {
    let core = &parts("ozzit.xlsx")["docProps/core.xml"];
    let creator = b"<dc:creator>Ozzit project</dc:creator>";
    assert!(core.windows(creator.len()).any(|window| window == creator));
}
