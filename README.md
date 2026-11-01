# Simon Says

Simon Says is an early implementation of a local, model-configured workflow engine. **The workflow runner is not implemented.** The current CLI validates a proposed version-1 TOML definition, writes a snapshot of the definition and referenced instructions/profiles/schemas, and inspects a snapshot. It does not invoke agents or commands, validate output artifacts, create runs or Git checkpoints, or perform recovery. Do not use it to execute workflows.

```sh
cargo test
cargo run -- validate examples/basic/workflow.toml
cargo run -- freeze examples/basic/workflow.toml /tmp/simon-snapshot.json
cargo run -- inspect /tmp/simon-snapshot.json
```

The example is a template validation fixture, not an executable workflow. The validator checks version, names, required roles, one terminating reachable pass route, path syntax, referenced files and JSON Schema compilation. It rejects symlinked authority references, external schema references and `.git` path components. It does not validate runtime inputs, provider argument policy or domain output. JSON Schema validation of actual artifacts is not yet implemented.

Snapshots contain authoritative file contents and a SHA-256 digest. The digest detects accidental changes but is stored in the same editable file; it is not an authentication mechanism, and an attacker can change the contents and recompute it. No execution authority or independent digest anchor exists yet. Configuration is trusted local input; it is not a sandbox.

The library also contains a **pure routing prototype** with Worker, Reviewer, and Unblocker outcomes, bounded corrections and one Unblocker detour. It performs no Git, I/O, persistence or artifact validation, so it must not be treated as a controller. The proposed requirements and acceptance matrix are in [docs](docs/README.md); current progress and baseline evidence are in [implementation status](docs/implementation/status.md).
