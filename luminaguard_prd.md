This is a Product Requirements Document (PRD) for **LuminaGuard**, the hypothetical tool proposed in our previous analysis. It is designed to solve the critical "usability vs. security" gap identified in the 2026 Agentic AI landscape.

---

# Product Requirements Document: LuminaGuard

**Version:** 1.0 (Draft)
**Status:** Proposal
**Codename:** "The Vibe Killer"

## 1. Executive Summary

**LuminaGuard** is a local-first Agentic AI runtime designed to replace the insecure "vibe coding" paradigm of OpenClaw with a rigorous "Agentic Engineering" approach.

It combines the **usability of OpenClaw** (easy setup, vast ecosystem) with the **security of Nanoclaw** (strict isolation). By utilizing Just-in-Time (JIT) Micro-VMs and the standard Model Context Protocol (MCP), LuminaGuard provides a secure, efficient, and auditable foundation for personal AI automation.

## 2. Problem Statement

The current landscape is polarized and dangerous:

* **The Security Crisis:** Market leader OpenClaw (formerly Clawdbot) runs agents with full shell access on the host machine. This has led to critical RCE vulnerabilities (CVE-2026-25253) and widespread malware distribution via its plugin ecosystem .
* **The Usability Gap:** Secure alternatives like Nanoclaw require complex Docker knowledge, alienating average users.


* **The Bloat:** "Vibe coding" (AI-generated codebases) has resulted in unmaintainable software bloat. OpenClaw requires ~200MB+ RAM and 8-12s startup times, whereas optimized tools like Nanobot require only ~45MB and 0.8s .

## 3. Product Principles

1. **Invisible Security:** The user should never see a Dockerfile. Isolation must happen automatically via JIT Micro-VMs.
2. **Trust, Don't Hope:** High-stakes actions (file deletion, financial transfers) require explicit human approval via the "Approval Cliff."
3. **Standardization over Proprietary:** No custom "AgentSkills." LuminaGuard is a native Model Context Protocol (MCP) client .
4. **Agentic Engineering:** The codebase is small, auditable, and deterministic.

## 4. Technical Architecture

### 4.1 The Core: "Rust Wrapper, Python Brain"

* **Orchestrator (Rust):** A lightweight Rust binary handles the CLI, memory management, and Micro-VM spawning. This ensures memory safety and near-instant startup.
* **Agent Logic (Python):** The agent's reasoning loop is a fork of the **Nanobot** core (`loop.py`), kept under 4,000 lines of code for auditability .

### 4.2 Just-in-Time (JIT) Micro-VMs

Instead of running on the host or requiring a permanent Docker daemon, LuminaGuard uses Firecracker-like Micro-VMs.

* **Session Lifecycle:** When a user asks, "Download this invoice," LuminaGuard spins up a stripped-down Linux VM in <200ms.
* **Execution:** The browser/tool runs *inside* this VM.
* **Disposal:** Once the task is done and the file is extracted, the VM is vaporized. Malware cannot persist because the computer it "infected" no longer exists.

### 4.3 Native MCP Support

LuminaGuard acts as a universal **MCP Client**.

* **Connectors:** It connects to any standard MCP Server (Google Drive, Slack, GitHub, Postgres).
* **Advantage:** This instantly gives LuminaGuard access to the thousands of enterprise connectors being built by the industry (Anthropic, Replit, etc.) without relying on a dangerous community plugin registry .

## 5. Key Features

### 5.1 The "Approval Cliff" UI

A dashboard that visualizes the agent's intent before execution.

* **Green Actions (Autonomous):** Reading files, searching the web, checking logs.
* **Red Actions (Paused):** Editing code, deleting files, sending emails, transferring crypto.
* **Interaction:** The agent pauses and presents a "Diff Card" (e.g., "I am about to delete these 3 files. Approve?").

### 5.2 Private Swarm Mesh

A secure alternative to the failed "Moltbook" social network.

* **Function:** Allows multiple local LuminaGuard instances to communicate over an encrypted local mesh network (e.g., WireGuard).
* **Use Case:** A "Researcher" agent on a user's Mac Mini can securely pass data to a "Coder" agent on their MacBook Pro without the data ever touching the public internet.

## 6. Competitive Comparison

| Feature | OpenClaw | Nanoclaw | **LuminaGuard (Proposed)** |
| --- | --- | --- | --- |
| **Execution Env** | Host OS (Unsafe) | Docker Container | **JIT Micro-VM** |
| **Plugin System** | Proprietary "Skills" | Custom Code | **Native MCP** |
| **Security Friction** | None (Dangerous) | High (Manual Docker) | **Zero (Automated)** |
| **Codebase** | "Vibe Coded" (Bloated) | Minimalist | **Engineered (Rust/Py)** |
| **Startup Time** | Slow (8s+) | Medium | **Instant (<200ms)** |

## 7. Roadmap

### Phase 1: The Foundation (Months 1-2)

* Fork **Nanobot** to establish the efficient Python reasoning loop .
* Build the **Rust Orchestrator** to manage standard MCP connections.
* Implement basic "Green Action" autonomy.

### Phase 2: The Fortress (Months 3-4)

* Integrate **Firecracker** for JIT Micro-VM spawning.
* Implement the **"Approval Cliff"** UI for file system writes.
* Beta release to security-conscious developers.

### Phase 3: The Swarm (Months 5-6)

* Develop the **Private Mesh** protocol for multi-agent collaboration.
* Release "LuminaGuard Desktop" (Electron-free, Rust-based GUI).

## 8. Success Metrics

* **Safety:** 0 reported RCEs or container escapes.
* **Performance:** <500ms startup time for new agent sessions.
* **Adoption:** 50+ verified community MCP servers working out-of-the-box.