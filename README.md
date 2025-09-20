# MAIA: Modular AI Infrastructure Architecture
The protocol for personal intelligence networks

Status: Pre-Alpha (SDK foundation complete)  
License: MIT  
Contact: maia@maxijaxi.net

---

## Mission

Build an open protocol that lets anyone run their own AI infrastructure and optionally federate with others — putting privacy, control, and interoperability back into users’ hands.

MAIA is not a product or a chatbot. It’s a protocol and reference implementation that prioritizes:
- User sovereignty over data and compute
- Extensibility through modules
- Interoperability across networks
- Security and isolation by default

---

## What MAIA Is

- A minimal core (microkernel) plus modules that provide capabilities
- Message-passing between everything (no shared state)
- A developer-first foundation in Rust

What MAIA is not:
- A centralized cloud service
- A single assistant or app
- A walled garden

---

## Core Concepts

- Node: A single MAIA instance on one device
- Cluster: All nodes owned by one entity (your personal MAIA)
- Module: A functional unit providing capabilities (LLM, storage, HTTP, sensors, etc.)
- Capability: A named function a module provides (e.g., ai.nlp.generate)
- Federation: Optional connections between clusters with explicit trust and policies

---

## Design Principles

1) Minimal core, everything else is a module  
2) Message passing only (no shared state)  
3) Protocol over implementation  
4) Security and isolation by default  
5) Explicit trust boundaries (local, cluster, federated, public)  
6) Developer ergonomics without sacrificing safety  
7) Document everything public

Key decisions:
- Language: Rust (Tokio async)
- Serialization: JSON externally, bincode internally where appropriate
- Isolation order: Native first, then WASM, then other methods (process/container)
- Discovery: mDNS for local (later), DHT/federation for broader networks (later)
- Error handling: Temporary vs Fatal with rich context

---

## Architecture (High Level)

Microkernel responsibilities:
- Identity (future): cryptographic identity for clusters and nodes
- Module Runtime: load, run, and isolate modules
- Message Router: route requests to capability providers (local or remote)
- Discovery (future): find capabilities locally and across networks
- Federation (future): policy-driven connectivity between clusters

Modules do all the work. The core orchestrates and routes.

---

## Capabilities

Naming convention: namespace.category.action

Examples:
- ai.nlp.generate
- ai.vision.detect
- storage.kv.get
- network.http.post
- sensor.temperature.read
- cluster.internal.*
- federated.public.*

---

## Module Interface (SDK Snapshot)

The SDK defines an async trait with identity/metadata, capabilities, and request handling. A minimal shape:

```rust
#[async_trait]
pub trait MaiaModule {
    fn manifest(&self) -> ModuleManifest;
    fn capabilities(&self) -> Vec<Capability>;
    async fn handle_request(&mut self, req: Request) -> Result<Response>;
}
```

Error philosophy:
- Temporary errors are retryable
- Fatal errors require alternative handling
- Always include context and suggestions

---

## Development Status

Pre-Alpha. The SDK foundation is in place; core runtime and router are in progress.

- Done
  - Module trait and core types
  - Capability system (pattern matching)
  - Error handling (Temporary vs Fatal) with context
  - Project structure and developer docs (internal)
- In Progress
  - Message router (core functionality)
  - Module runtime (Native first; WASM next)
  - First example module (Echo) to validate end-to-end

---

## Roadmap (Broad)

Near term:
- Module runtime (native loader, lifecycle, registry)
- Local message routing and correlation
- Echo module as the first validation target

Next:
- Capability indexing and local discovery
- Basic CLI for dev workflows (later)
- Inter-node communication (gRPC) groundwork

Later:
- mDNS discovery for clusters
- Network identity (Ed25519), DIDs
- Federation protocol (auth, trust, policy, accounting)
- Example modules beyond Echo (storage, HTTP client, simple LLM wrapper)

Timelines are intentionally flexible at this phase.

---

## Contributing

This is early-stage work. If you want to help with the core, runtime, router, or early modules, reach out.

- Style: idiomatic Rust, no panics in production paths
- Testing: aim for 80%+ where feasible; prioritize critical paths
- Security: assume modules are untrusted; enforce capability boundaries

Contact: maia@maxijaxi.net

---

## License

MIT
