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
    /// Per-action wall-clock limit; defaults to 1800 seconds.
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub args: Vec<String>,
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
    pub outputs: Vec<String>,
    pub output_schema: Option<String>,
    pub on_pass: String,
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
fn relative(s: &str) -> Result<()> {
    ensure!(
        !s.is_empty()
            && Path::new(s)
                .components()
                .all(|c| matches!(c, Component::Normal(part) if !part.to_string_lossy().eq_ignore_ascii_case(".git"))),
        "path must be a confined relative path: {s:?}"
    );
    Ok(())
}
fn read_confined(root: &Path, name: &str) -> Result<String> {
    relative(name)?;
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
        let mut refs = BTreeSet::new();
        for (name, role) in &w.roles {
            ensure!(identifier(name), "invalid role name {name:?}");
            if let Some(seconds) = role.timeout_seconds {
                ensure!(
                    (1..=86400).contains(&seconds),
                    "role {name}: timeout_seconds must be 1..=86400"
                );
            }
            match role.kind.as_str() {
                "command" => ensure!(
                    !role.argv.is_empty()
                        && !role.argv[0].is_empty()
                        && role.adapter.is_none()
                        && role.model.is_none()
                        && role.effort.is_none()
                        && role.profile.is_none()
                        && role.args.is_empty(),
                    "invalid command role {name}"
                ),
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
            ensure!(
                (1..=100).contains(&stage.max_review_cycles),
                "stage {name}: max_review_cycles must be 1..=100"
            );
            ensure!(
                !stage.outputs.is_empty(),
                "stage {name}: at least one output is required"
            );
            for p in stage.inputs.iter().chain(stage.outputs.iter()) {
                relative(p).with_context(|| format!("stage {name}"))?;
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
        let mut next = w.start.as_str();
        while next != "complete" {
            ensure!(
                seen.insert(next.to_string()),
                "pass transition cycle at {next}"
            );
            next = &w.stages[next].on_pass;
        }
        ensure!(
            seen.len() == w.stages.len(),
            "unreachable stage(s): {:?}",
            w.stages
                .keys()
                .filter(|s| !seen.contains(*s))
                .collect::<Vec<_>>()
        );
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

pub mod routing;

pub mod store;

pub mod artifact;

pub mod provider;

pub mod git;

pub mod controller;

pub mod cleanup;
pub mod sandbox;
