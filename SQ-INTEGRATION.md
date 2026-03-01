# SQ Daemon Mode Integration Plan

## Overview
Integrate SQ (11-dimensional phext database) as an alternative memory backend for OpenFang agents. SQ uses shared memory IPC for high-performance local communication and provides coordinate-based storage.

## Current Status
- ✅ `openfang-sq` crate exists with client implementation
- ✅ SqClient supports shared memory IPC
- ✅ SqStore provides high-level typed storage interface
- ❌ No kernel integration
- ❌ No daemon lifecycle management
- ❌ No API endpoints
- ❌ No configuration support

## Integration Components

### 1. Configuration (`KernelConfig`)
Add SQ daemon configuration to `openfang-types/src/config.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqConfig {
    /// Enable SQ daemon integration
    pub enabled: bool,
    /// Path to SQ binary (defaults to `sq` in PATH)
    pub binary_path: Option<String>,
    /// Phext file to load (defaults to "openfang.phext")
    pub phext_file: String,
    /// Namespace for this OpenFang instance (1-127)
    pub namespace: usize,
    /// Auto-start daemon if not running
    pub auto_start: bool,
}
```

### 2. Daemon Lifecycle Management
Add to `openfang-kernel`:
- `SqDaemon` struct to manage the SQ process
- Start daemon on kernel boot (if enabled and auto_start=true)
- Graceful shutdown on kernel shutdown
- Health checks and automatic restart

### 3. Memory Store Integration
Options:
- **A. Replace SQLite** — Use SqStore as primary memory backend
- **B. Supplement SQLite** — Use SqStore for agent memory, keep SQLite for metadata
- **C. Configurable** — Let users choose via config (recommended)

Coordinate allocation scheme (already designed in SqStore):
```
Library:    namespace (per-instance isolation)
Shelf:      data type (1=kv, 2=sessions, 3=agents, 4=knowledge, 5=semantic)
Series:     shard/partition

Collection: context ID (agent, session, etc.)
Volume:     sub-context
Book:       category

Chapter:    group
Section:    sub-group
Scroll:     item index
```

### 4. API Endpoints
Add to `openfang-api/src/routes.rs`:
```
GET  /api/sq/status          - SQ daemon status
GET  /api/sq/health          - Health check
POST /api/sq/write           - Write to coordinate
POST /api/sq/read            - Read from coordinate
POST /api/sq/delete          - Delete coordinate
GET  /api/sq/toc             - Table of contents
GET  /api/sq/delta           - Checksum delta tree
```

### 5. CLI Commands
Add to `openfang-cli`:
```bash
openfang sq status              # Show daemon status
openfang sq start               # Manually start daemon
openfang sq stop                # Stop daemon
openfang sq restart             # Restart daemon
openfang sq shell               # Interactive phext shell
openfang sq export <coord>      # Export coordinate to file
openfang sq import <coord> <f>  # Import file to coordinate
```

### 6. Dashboard Integration
Add SQ tab to Tauri dashboard:
- Daemon status indicator
- Coordinate browser
- Memory usage by namespace/shelf
- Real-time phext visualization

## Implementation Order

### Phase 1: Daemon Management ✅ CURRENT
1. Add `SqConfig` to `KernelConfig`
2. Implement `SqDaemon` lifecycle manager
3. Wire into kernel boot/shutdown
4. Add health checks

### Phase 2: Memory Integration
1. Add `SqStore` as optional memory backend
2. Implement agent message storage via coordinates
3. Add session persistence
4. Benchmarking vs SQLite

### Phase 3: API & CLI
1. Add REST API endpoints
2. Implement CLI commands
3. Add health monitoring
4. Error handling & logging

### Phase 4: Dashboard
1. Add SQ status panel
2. Coordinate browser UI
3. Memory usage charts
4. Phext visualization

## Testing Strategy

### Unit Tests
- SQ config serialization
- Coordinate allocation schemes
- Store operations (with mocked client)

### Integration Tests
- Daemon start/stop
- IPC roundtrip (write/read)
- Multi-instance isolation (different namespaces)
- Crash recovery

### Load Tests
- 1M message writes
- Concurrent access from multiple agents
- Memory usage under load
- Compare to SQLite baseline

## Security Considerations

1. **Shared Memory Access** — Only OpenFang processes should access `.sq/link` and `.sq/work`
2. **Path Traversal** — Validate phext file paths
3. **Namespace Isolation** — Ensure agents can't access other namespaces
4. **Daemon Crashes** — Graceful degradation if SQ dies
5. **Data Persistence** — Regular saves to disk

## Migration Path

For existing OpenFang users:
1. SQ disabled by default (opt-in)
2. `openfang migrate --to-sq` command to copy SQLite → SQ
3. Dual-write mode for transition period
4. Export/import for backups

## Dependencies

Already in workspace:
- `shared_memory` — IPC
- `raw_sync` — Event synchronization
- `libphext` — Coordinate handling

New dependencies:
- None (all covered by openfang-sq)

## Performance Goals

- **Cold start**: < 500ms for daemon boot
- **Write latency**: < 1ms for small scrolls
- **Read latency**: < 0.5ms for cached scrolls
- **Throughput**: > 10K ops/sec on modern hardware
- **Memory overhead**: < 50MB for typical workloads

## Success Criteria

- [ ] SQ daemon starts automatically with kernel
- [ ] Agent messages persist to phext coordinates
- [ ] Sessions survive kernel restarts
- [ ] API endpoints work end-to-end
- [ ] CLI commands functional
- [ ] Dashboard shows SQ status
- [ ] Performance meets goals
- [ ] Zero data loss on clean shutdown
- [ ] Recovery from daemon crashes

## Next Steps

**Immediate:**
1. Add `SqConfig` to `openfang-types`
2. Implement `SqDaemon` in `openfang-kernel`
3. Wire into `OpenFangKernel::boot_with_config()`
4. Test daemon start/stop/health

**Week 1:**
- Complete Phase 1 (Daemon Management)
- Basic integration tests
- Documentation

**Week 2:**
- Phase 2 (Memory Integration)
- Benchmark against SQLite
- Production testing on ranch

**Week 3:**
- Phase 3 (API & CLI)
- Phase 4 (Dashboard)
- User documentation

---

**Author:** Phex (wbic16)  
**Date:** 2026-02-28  
**Coordinate:** 1.5.2/3.7.3/9.1.1 (Phex home base)
