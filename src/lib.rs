use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    pub schema_version: u32,
    pub name: String,
    /// Reusable workflow purpose, included in every action packet.
    pub purpose: Option<String>,
    pub start: String,
    /// Maximum number of accepted stage transitions, including completion.
    /// Required for conditional routes and for workflows containing cycles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stage_transitions: Option<u32>,
    /// Tracked project files whose baseline bytes and modes cannot change during a run.
    #[serde(default)]
    pub frozen_project_files: Vec<String>,
    pub roles: BTreeMap<String, Role>,
    pub stages: BTreeMap<String, Stage>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub kind: String,
    #[serde(default)]
    pub argv: Vec<String>,
    pub adapter: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub profile: Option<String>,
    /// Opt in to reusing this Codex Worker session for same-stage corrections
    /// during one uninterrupted controller invocation.
    #[serde(default)]
    pub resume_worker_session: bool,
    /// Per-action wall-clock limit; defaults to 1800 seconds.
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables whose values are confined action-owned scratch subdirectories.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scratch_env: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub worker: String,
    pub reviewer: String,
    pub unblocker: String,
    pub instructions: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Reviewed inputs that this stage must not change, relative to its starting checkpoint.
    #[serde(default)]
    pub protected_inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub output_schema: Option<String>,
    pub on_pass: String,
    /// Labels mapped to frozen, permitted destinations.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub routes: BTreeMap<String, String>,
    /// Select routes from the Worker response instead of the Reviewer response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_selector: Option<String>,
    /// Stop after review until the accepted checkpoint is explicitly approved.
    #[serde(default)]
    pub approval_required: bool,
    pub max_review_cycles: u32,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: u32,
    pub template: String,
    pub files: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_context: Option<String>,
    pub sha256: String,
}

fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
pub(crate) fn validate_relative_path(s: &str) -> Result<()> {
    ensure!(
        !s.is_empty()
            && Path::new(s)
                .components()
                .all(|c| matches!(c, Component::Normal(part) if !part.to_string_lossy().eq_ignore_ascii_case(".git"))),
        "path must be a confined relative path: {s:?}"
    );
    Ok(())
}
/// Validate workflow-authored bindings without interpreting them through a shell.
pub(crate) fn validate_scratch_env(bindings: &BTreeMap<String, String>) -> Result<()> {
    ensure!(
        bindings.len() <= 32,
        "scratch_env supports at most 32 variables"
    );
    for (name, path) in bindings {
        validate_scratch_env_name(name)?;
        ensure!(
            path.len() <= 1024
                && !path.is_empty()
                && !path.contains('\\')
                && path.split('/').all(|part| !part.is_empty()
                    && part != "."
                    && part != ".."
                    && !part.eq_ignore_ascii_case(".git")
                    && !part.chars().any(char::is_control)),
            "scratch_env path must be a confined relative directory: {path:?}"
        );
        validate_relative_path(path)?;
    }
    Ok(())
}

pub(crate) fn validate_scratch_env_name(name: &str) -> Result<()> {
    let mut bytes = name.bytes();
    ensure!(
        name.len() <= 128
            && bytes
                .next()
                .is_some_and(|c| c == b'_' || c.is_ascii_uppercase())
            && bytes.all(|c| c == b'_' || c.is_ascii_uppercase() || c.is_ascii_digit()),
        "invalid scratch_env variable name {name:?}; use uppercase ASCII environment names"
    );
    let reserved_prefix = [
        "SIMON_",
        "GIT_",
        "DYLD_",
        "LD_",
        "BASH_",
        "CODEX_",
        "CLAUDE_",
        "ANTHROPIC_",
        "OPENAI_",
    ];
    let reserved_names = [
        "PATH",
        "HOME",
        "SHELL",
        "ENV",
        "IFS",
        "CDPATH",
        "PWD",
        "OLDPWD",
        "SHLVL",
        "PYTHONPATH",
        "PYTHONHOME",
        "PYTHONSTARTUP",
        "NODE_OPTIONS",
        "NODE_PATH",
        "RUBYOPT",
        "PERL5OPT",
        "PERL5LIB",
        "JAVA_TOOL_OPTIONS",
        "JDK_JAVA_OPTIONS",
        "_JAVA_OPTIONS",
        "RUSTC",
        "RUSTDOC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
        "RUSTDOCFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
    ];
    ensure!(
        !reserved_names.contains(&name)
            && !reserved_prefix
                .iter()
                .any(|prefix| name.starts_with(prefix)),
        "scratch_env variable {name:?} is reserved"
    );
    Ok(())
}

fn read_confined(root: &Path, name: &str) -> Result<String> {
    validate_relative_path(name)?;
    let root = root
        .canonicalize()
        .context("canonicalize template directory")?;
    let mut current = root.clone();
    for part in Path::new(name).components() {
        current.push(part);
        let metadata = fs::symlink_metadata(&current)
            .with_context(|| format!("missing authoritative file {name}"))?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "symlink in authoritative path {name}"
        );
    }
    ensure!(
        current.is_file(),
        "authoritative path is not a file: {name}"
    );
    fs::read_to_string(&current).with_context(|| format!("read authoritative file {name}"))
}
impl Workflow {
    pub fn parse_and_validate(
        template: &str,
        root: &Path,
    ) -> Result<(Self, BTreeMap<String, String>)> {
        let w: Self = toml::from_str(template).context("parse workflow TOML")?;
        ensure!(
            w.schema_version == 1,
            "unsupported schema_version {}",
            w.schema_version
        );
        ensure!(identifier(&w.name), "invalid workflow name");
        if let Some(purpose) = &w.purpose {
            ensure!(
                !purpose.trim().is_empty() && purpose.len() <= 16384,
                "workflow purpose must be nonempty and at most 16384 bytes"
            );
        }
        ensure!(
            !w.stages.is_empty() && w.stages.contains_key(&w.start),
            "start stage does not exist"
        );
        if let Some(max) = w.max_stage_transitions {
            ensure!(
                (1..=1000).contains(&max),
                "max_stage_transitions must be 1..=1000"
            );
        }
        let mut frozen_paths = BTreeSet::new();
        for path in &w.frozen_project_files {
            validate_relative_path(path).context("invalid frozen project path")?;
            ensure!(
                !path.chars().any(|c| c.is_control()),
                "frozen project path contains control characters"
            );
            ensure!(
                frozen_paths.insert(path),
                "duplicate frozen project path {path}"
            );
        }
        let mut refs = BTreeSet::new();
        for (name, role) in &w.roles {
            ensure!(identifier(name), "invalid role name {name:?}");
            validate_scratch_env(&role.scratch_env).with_context(|| format!("role {name}"))?;
            if let Some(seconds) = role.timeout_seconds {
                ensure!(
                    (1..=86400).contains(&seconds),
                    "role {name}: timeout_seconds must be 1..=86400"
                );
            }
            if role.resume_worker_session {
                ensure!(
                    role.kind == "agent" && role.adapter.as_deref() == Some("codex"),
                    "role {name}: resume_worker_session currently requires a Codex agent"
                );
                ensure!(
                    !role.args.iter().any(|arg| arg
                        .split_once('=')
                        .map_or(arg.as_str(), |(flag, _)| flag)
                        == "--ephemeral"),
                    "role {name}: resume_worker_session is incompatible with --ephemeral"
                );
            }
            match role.kind.as_str() {
                "command" => {
                    ensure!(
                        !role.argv.is_empty() && !role.argv[0].is_empty(),
                        "role {name}: command requires a nonempty argv array and program"
                    );
                    for (field, present) in [
                        ("adapter", role.adapter.is_some()),
                        ("model", role.model.is_some()),
                        ("effort", role.effort.is_some()),
                        ("profile", role.profile.is_some()),
                        ("args", !role.args.is_empty()),
                    ] {
                        ensure!(
                            !present,
                            "role {name}: command must omit agent-only field {field}; use argv for command arguments"
                        );
                    }
                }
                "agent" => {
                    ensure!(
                        matches!(role.adapter.as_deref(), Some("codex" | "claude")),
                        "unsupported agent adapter for role {name}"
                    );
                    ensure!(
                        role.argv.is_empty(),
                        "agent role {name} cannot specify argv"
                    );
                    let adapter = role.adapter.as_deref().unwrap();
                    provider::validate_native_args(adapter, &role.args)?;
                    if let Some(effort) = &role.effort {
                        let valid = match adapter {
                            "codex" => matches!(
                                effort.as_str(),
                                "none"
                                    | "minimal"
                                    | "low"
                                    | "medium"
                                    | "high"
                                    | "xhigh"
                                    | "max"
                                    | "ultra"
                            ),
                            "claude" => matches!(
                                effort.as_str(),
                                "low" | "medium" | "high" | "xhigh" | "max" | "ultracode"
                            ),
                            _ => false,
                        };
                        ensure!(valid, "invalid effort for role {name}");
                    }
                    refs.insert(
                        role.profile
                            .as_ref()
                            .context(format!("agent role {name} needs a profile"))?
                            .clone(),
                    );
                }
                _ => bail!("unknown role kind in {name}"),
            }
        }
        for (name, stage) in &w.stages {
            ensure!(
                identifier(name) && name != "complete",
                "invalid stage name {name:?}"
            );
            for role in [&stage.worker, &stage.reviewer, &stage.unblocker] {
                ensure!(
                    w.roles.contains_key(role),
                    "stage {name}: unknown role {role}"
                );
            }
            ensure!(
                stage.on_pass == "complete" || w.stages.contains_key(&stage.on_pass),
                "stage {name}: unknown transition {}",
                stage.on_pass
            );
            if let Some(selector) = &stage.route_selector {
                ensure!(
                    selector == "worker" || selector == "reviewer",
                    "stage {name}: route_selector must be worker or reviewer"
                );
                ensure!(
                    !stage.routes.is_empty(),
                    "stage {name}: route_selector requires routes"
                );
            }
            if !stage.routes.is_empty() {
                ensure!(
                    w.max_stage_transitions.is_some(),
                    "stage {name}: routes require max_stage_transitions"
                );
                ensure!(
                    stage.routes.values().any(|v| v == &stage.on_pass),
                    "stage {name}: on_pass must be one of the declared route destinations"
                );
                for (label, destination) in &stage.routes {
                    ensure!(
                        identifier(label),
                        "stage {name}: invalid route label {label:?}"
                    );
                    ensure!(
                        destination == "complete" || w.stages.contains_key(destination),
                        "stage {name}: route {label} has unknown destination {destination}"
                    );
                }
            }
            ensure!(
                (1..=100).contains(&stage.max_review_cycles),
                "stage {name}: max_review_cycles must be 1..=100"
            );
            ensure!(
                !stage.outputs.is_empty(),
                "stage {name}: at least one output is required"
            );
            for p in stage.inputs.iter().chain(stage.outputs.iter()) {
                validate_relative_path(p).with_context(|| format!("stage {name}"))?;
            }
            let mut protected_inputs = BTreeSet::new();
            for path in &stage.protected_inputs {
                ensure!(
                    stage.inputs.contains(path),
                    "stage {name}: protected input {path} must be declared in inputs"
                );
                ensure!(
                    !stage.outputs.contains(path),
                    "stage {name}: protected input {path} cannot also be an output"
                );
                ensure!(
                    protected_inputs.insert(path),
                    "stage {name}: duplicate protected input {path}"
                );
                ensure!(
                    !path.chars().any(|c| c.is_control()),
                    "stage {name}: protected input path contains control characters"
                );
            }
            for p in &stage.outputs {
                ensure!(
                    !frozen_paths.contains(p),
                    "stage {name}: output {p} is a frozen project file"
                );
            }
            refs.insert(stage.instructions.clone());
            if let Some(schema) = &stage.output_schema {
                ensure!(
                    stage.outputs.len() == 1,
                    "stage {name}: output_schema requires exactly one output"
                );
                refs.insert(schema.clone());
            }
        }
        let mut seen = BTreeSet::new();
        let mut pending = vec![w.start.as_str()];
        let mut reaches_complete = false;
        while let Some(next) = pending.pop() {
            if next == "complete" {
                reaches_complete = true;
                continue;
            }
            if !seen.insert(next.to_string()) {
                continue;
            }
            let stage = &w.stages[next];
            if stage.routes.is_empty() {
                pending.push(&stage.on_pass);
            } else {
                pending.extend(stage.routes.values().map(String::as_str));
            }
        }
        ensure!(
            seen.len() == w.stages.len(),
            "unreachable stage(s): {:?}",
            w.stages
                .keys()
                .filter(|s| !seen.contains(*s))
                .collect::<Vec<_>>()
        );
        ensure!(reaches_complete, "workflow has no route to complete");
        // Legacy definitions have exactly one successor. Retain their acyclic
        // validation unless the author explicitly supplies a durable bound.
        if w.max_stage_transitions.is_none() {
            let mut linear_seen = BTreeSet::new();
            let mut next = w.start.as_str();
            while next != "complete" {
                ensure!(linear_seen.insert(next), "pass transition cycle at {next}");
                next = &w.stages[next].on_pass;
            }
        }
        let mut files = BTreeMap::new();
        for name in refs {
            let content = read_confined(root, &name)?;
            if w.stages
                .values()
                .any(|s| s.output_schema.as_ref() == Some(&name))
            {
                let schema: serde_json::Value = serde_json::from_str(&content)
                    .with_context(|| format!("invalid JSON schema {name}"))?;
                validate_schema(&schema).with_context(|| format!("invalid JSON schema {name}"))?;
            }
            files.insert(name, content);
        }
        Ok((w, files))
    }
}
fn validate_schema(schema: &serde_json::Value) -> Result<()> {
    if let Some(draft) = schema.get("$schema") {
        ensure!(
            draft
                .as_str()
                .is_some_and(|v| v == "http://json-schema.org/draft-07/schema#"
                    || v == "http://json-schema.org/draft-07/schema"
                    || v == "https://json-schema.org/draft-07/schema#"
                    || v == "https://json-schema.org/draft-07/schema"),
            "only JSON Schema draft-07 is supported"
        );
    }
    fn check_refs(value: &serde_json::Value) -> Result<()> {
        match value {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    ensure!(
                        !matches!(
                            k.as_str(),
                            "prefixItems"
                                | "unevaluatedItems"
                                | "unevaluatedProperties"
                                | "dependentSchemas"
                                | "dependentRequired"
                                | "minContains"
                                | "maxContains"
                                | "$defs"
                                | "$vocabulary"
                                | "$dynamicAnchor"
                                | "$recursiveAnchor"
                                | "contentSchema"
                        ),
                        "unsupported later-draft schema keyword or property: {k}"
                    );
                    ensure!(
                        !matches!(k.as_str(), "$ref" | "$dynamicRef" | "$recursiveRef"),
                        "JSON Schema references are not supported"
                    );
                    check_refs(v)?;
                }
            }
            serde_json::Value::Array(items) => {
                for v in items {
                    check_refs(v)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    check_refs(schema)?;
    jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .compile(schema)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

fn digest(template: &str, files: &BTreeMap<String, String>, run_context: Option<&str>) -> String {
    let mut hash = Sha256::new();
    for (name, content) in
        std::iter::once(("", template)).chain(files.iter().map(|(k, v)| (k.as_str(), v.as_str())))
    {
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update((content.len() as u64).to_be_bytes());
        hash.update(content.as_bytes());
    }
    if let Some(context) = run_context {
        hash.update(b"\xffsimon-run-context-v1\0");
        hash.update((context.len() as u64).to_be_bytes());
        hash.update(context.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
impl Snapshot {
    pub fn create(path: &Path) -> Result<Self> {
        let metadata =
            fs::symlink_metadata(path).with_context(|| format!("inspect {}", path.display()))?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "workflow template must be a regular, non-symlink file"
        );
        let template =
            fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let (_, files) = Workflow::parse_and_validate(
            &template,
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )?;
        let sha256 = digest(&template, &files, None);
        Ok(Self {
            schema_version: 1,
            template,
            files,
            run_context: None,
            sha256,
        })
    }
    /// Bind run-specific user context into the same frozen authority digest.
    pub fn with_context(mut self, path: &Path) -> Result<Self> {
        const MAX_CONTEXT_BYTES: u64 = 32 * 1024;
        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("inspect context {}", path.display()))?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "run context must be a regular, non-symlink file"
        );
        ensure!(
            metadata.len() <= MAX_CONTEXT_BYTES,
            "run context exceeds 32 KiB"
        );
        use std::io::Read;
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options
            .open(path)
            .with_context(|| format!("open context {}", path.display()))?;
        ensure!(
            file.metadata()?.is_file(),
            "run context must be a regular file"
        );
        let mut content = String::new();
        file.take(MAX_CONTEXT_BYTES + 1)
            .read_to_string(&mut content)
            .with_context(|| format!("read UTF-8 context {}", path.display()))?;
        ensure!(
            !content.trim().is_empty() && content.len() <= MAX_CONTEXT_BYTES as usize,
            "run context must be nonempty and at most 32 KiB"
        );
        self.run_context = Some(content);
        self.sha256 = digest(&self.template, &self.files, self.run_context.as_deref());
        Ok(self)
    }
    pub fn verify(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "unsupported snapshot version");
        ensure!(
            self.sha256 == digest(&self.template, &self.files, self.run_context.as_deref()),
            "snapshot integrity mismatch"
        );
        Ok(())
    }
    pub fn write_new(&self, path: &Path) -> Result<()> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(|| format!("create {}", path.display()))?;
        file.write_all(&serde_json::to_vec_pretty(self)?)?;
        file.sync_all()?;
        Ok(())
    }
}
pub fn load_snapshot(path: &Path) -> Result<Snapshot> {
    let snapshot: Snapshot = serde_json::from_slice(&fs::read(path)?)?;
    snapshot.verify()?;
    Ok(snapshot)
}
pub fn template_path(path: &str) -> PathBuf {
    PathBuf::from(path)
}

mod git_process;

pub mod routing;

pub mod store;

pub mod artifact;

pub mod provider;

pub mod git;

pub mod adopt_worker;
pub mod controller;

pub mod cleanup;
