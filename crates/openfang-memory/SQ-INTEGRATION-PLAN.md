# SQ Daemon Integration Plan for OpenFang Memory

## Overview

Replace SQLite backend with SQ daemon-mode client for phext-native storage.

**Current:** OpenFang uses `rusqlite` with shared `Arc<Mutex<Connection>>`  
**Target:** OpenFang uses SQ daemon via Unix socket/named pipe  
**Approach:** Create abstraction layer allowing both backends

---

## Architecture

### Current Memory Substrate
```
MemorySubstrate
  ├─ Arc<Mutex<Connection>> (rusqlite)
  ├─ StructuredStore
  ├─ SemanticStore  
  ├─ KnowledgeStore
  ├─ SessionStore
  ├─ ConsolidationEngine
  └─ UsageStore
```

### Proposed Dual-Backend Architecture
```
MemorySubstrate<B: StorageBackend>
  ├─ B (backend trait)
  │   ├─ SqliteBackend (Arc<Mutex<Connection>>)
  │   └─ SqDaemonBackend (DaemonClient)
  ├─ StructuredStore<B>
  ├─ SemanticStore<B>
  ├─ KnowledgeStore<B>
  ├─ SessionStore<B>
  ├─ ConsolidationEngine<B>
  └─ UsageStore<B>
```

---

## Phase 1: Backend Abstraction (Week 1)

### 1.1 Define StorageBackend Trait
**File:** `crates/openfang-memory/src/backend.rs`

```rust
/// Abstract storage backend for OpenFang memory.
pub trait StorageBackend: Send + Sync {
    /// Read data from coordinate/key
    fn read(&self, key: &str) -> Result<Vec<u8>>;
    
    /// Write data to coordinate/key
    fn write(&self, key: &str, value: &[u8]) -> Result<()>;
    
    /// Delete data at coordinate/key
    fn delete(&self, key: &str) -> Result<()>;
    
    /// List all keys matching prefix
    fn list(&self, prefix: &str) -> Result<Vec<String>>;
    
    /// Range query (for session messages)
    fn read_range(&self, start: &str, end: &str) -> Result<Vec<(String, Vec<u8>)>>;
    
    /// Search (fallback to linear scan if not supported)
    fn search(&self, pattern: &str, query: &str) -> Result<Vec<(String, Vec<u8>)>>;
    
    /// Begin transaction
    fn begin_transaction(&self) -> Result<Transaction>;
}
```

### 1.2 Implement SqliteBackend (Compatibility Layer)
**File:** `crates/openfang-memory/src/backend/sqlite.rs`

```rust
pub struct SqliteBackend {
    conn: Arc<Mutex<Connection>>,
}

impl StorageBackend for SqliteBackend {
    // Implement trait methods wrapping existing SQLite calls
    // Maps keys to SQLite schema
}
```

### 1.3 Stub SqDaemonBackend
**File:** `crates/openfang-memory/src/backend/sq_daemon.rs`

```rust
use sq_client::DaemonClient;

pub struct SqDaemonBackend {
    client: DaemonClient,
    socket_path: PathBuf,
}

impl StorageBackend for SqDaemonBackend {
    // TODO: Implement all trait methods
    fn read(&self, key: &str) -> Result<Vec<u8>> {
        unimplemented!("Phase 2")
    }
    // ... etc
}
```

**Deliverable:** Compiles with both backends, tests pass with SqliteBackend

---

## Phase 2: SQ Daemon Client (Week 2)

### 2.1 Add SQ Dependency
**File:** `crates/openfang-memory/Cargo.toml`

```toml
[dependencies]
# Existing...
sq-client = { version = "0.5", optional = true }

[features]
default = ["sqlite-backend"]
sqlite-backend = []
sq-backend = ["sq-client"]
```

### 2.2 Implement SQ Socket Connection
**File:** `crates/openfang-memory/src/backend/sq_daemon.rs`

```rust
impl SqDaemonBackend {
    pub fn connect(socket_path: PathBuf) -> Result<Self> {
        let client = DaemonClient::connect(&socket_path)?;
        Ok(Self { client, socket_path })
    }
    
    pub fn ensure_daemon_running(&self) -> Result<()> {
        if !self.socket_exists() {
            // Launch SQ daemon
            Command::new("sq")
                .arg("--daemon")
                .arg("--socket")
                .arg(&self.socket_path)
                .spawn()?;
            
            // Wait for ready
            self.wait_for_socket(Duration::from_secs(5))?;
        }
        Ok(())
    }
}
```

### 2.3 Coordinate Mapping Design
**File:** `crates/openfang-memory/src/backend/coord_mapper.rs`

```rust
/// Maps OpenFang entities to phext coordinates
pub struct CoordMapper;

impl CoordMapper {
    /// Session → Library.Shelf.Series
    /// Format: session-{agent_id}-{session_id} → {lib}.{shelf}.{series}
    pub fn session_to_coord(agent_id: AgentId, session_id: SessionId) -> PhextCoord {
        // Hash agent_id → Library (1-9)
        let lib = (agent_id.0 % 9) + 1;
        // Hash session_id → Shelf.Series (1-9.1-9)
        let shelf = ((session_id.0 / 9) % 9) + 1;
        let series = (session_id.0 % 9) + 1;
        PhextCoord::new(lib, shelf, series, 1, 1, 1, 1, 1, 1)
    }
    
    /// Message index → Collection.Volume.Book
    pub fn message_index_to_coord(base: PhextCoord, index: usize) -> PhextCoord {
        let col = ((index / (9 * 9)) % 9) + 1;
        let vol = ((index / 9) % 9) + 1;
        let book = (index % 9) + 1;
        base.with_collection(col).with_volume(vol).with_book(book)
    }
}
```

### 2.4 Implement Read/Write Operations
```rust
impl StorageBackend for SqDaemonBackend {
    fn write(&self, key: &str, value: &[u8]) -> Result<()> {
        let coord = CoordMapper::key_to_coord(key)?;
        self.client.write(coord, value)?;
        Ok(())
    }
    
    fn read(&self, key: &str) -> Result<Vec<u8>> {
        let coord = CoordMapper::key_to_coord(key)?;
        self.client.read(coord)
    }
}
```

**Deliverable:** Basic read/write working via SQ daemon

---

## Phase 3: Store Migration (Week 3)

### 3.1 Migrate SessionStore
**File:** `crates/openfang-memory/src/session.rs`

**Before:**
```rust
pub struct SessionStore {
    conn: Arc<Mutex<Connection>>,
}
```

**After:**
```rust
pub struct SessionStore<B: StorageBackend> {
    backend: Arc<B>,
}

impl<B: StorageBackend> SessionStore<B> {
    pub fn new(backend: Arc<B>) -> Self {
        Self { backend }
    }
    
    pub fn save(&self, session: &Session) -> Result<()> {
        let key = format!("session/{}/{}", session.agent_id, session.id);
        let data = serde_json::to_vec(session)?;
        self.backend.write(&key, &data)
    }
}
```

### 3.2 Migrate Other Stores
- StructuredStore
- SemanticStore
- KnowledgeStore
- ConsolidationEngine
- UsageStore

**Deliverable:** All stores generic over StorageBackend

---

## Phase 4: Testing & Validation (Week 4)

### 4.1 Unit Tests
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_sqlite_backend() {
        let backend = SqliteBackend::open_in_memory().unwrap();
        // ... existing tests
    }
    
    #[tokio::test]
    async fn test_sq_daemon_backend() {
        let backend = SqDaemonBackend::connect("/tmp/test-sq.sock").unwrap();
        backend.ensure_daemon_running().unwrap();
        // ... same tests as SQLite
    }
}
```

### 4.2 Integration Tests
- Full agent workflow (create session, save messages, retrieve)
- Migration from SQLite to SQ
- Performance benchmarks (latency, throughput)

### 4.3 Performance Targets
- Write latency: <500μs (daemon mode)
- Read latency: <200μs
- Range query (1000 messages): <5ms

**Deliverable:** All tests passing with both backends

---

## Phase 5: Production Hardening (Week 5-6)

### 5.1 Daemon Lifecycle
- Auto-start on first use
- Health monitoring
- Graceful restart on crash
- Clean shutdown

### 5.2 Error Handling
- Socket connection failures
- Daemon unavailable
- Coordinate mapping conflicts
- Data corruption recovery

### 5.3 Documentation
- Migration guide (SQLite → SQ)
- Configuration examples
- Performance tuning
- Troubleshooting

**Deliverable:** Production-ready SQ backend

---

## Deferred Features (Phase 6+)

### Vector Embeddings
**Two options:**

**A) SQ-native vectors**
- Store embeddings at offset coordinates
- Client-side cosine similarity
- OR extend SQ with vector ops

**B) Separate vector store (interim)**
- Keep SemanticStore on SQLite temporarily
- Migrate later when SQ vector support ready

**Recommendation:** Start with B, research A in parallel

### Full-Text Search
**Options:**
- Extend SQ with search API
- Client-side linear scan (acceptable for small datasets)
- External search index (violates hard scaling law)

---

## Open Questions for SQ Team

1. **Range queries:** Does SQ daemon support coordinate range scans?
2. **Transactions:** Can multiple writes be atomic?
3. **Protocol:** Is there a Rust client library or do we implement wire protocol?
4. **Performance:** Expected latency for localhost daemon read/write?
5. **Concurrency:** How many concurrent connections supported?

---

## Success Criteria

- [ ] All 1,767 OpenFang tests pass with SQ backend
- [ ] <500μs write latency (daemon mode)
- [ ] Zero data loss during migration
- [ ] Feature parity with SQLite backend
- [ ] Documentation complete

---

## Timeline Summary

**Week 1:** Backend abstraction + SqliteBackend  
**Week 2:** SQ daemon client + coordinate mapping  
**Week 3:** Migrate all stores to generic backend  
**Week 4:** Testing & validation  
**Week 5-6:** Production hardening  

**Total: 5-6 weeks to production-ready SQ backend**

---

*Plan created: 2026-03-01*  
*Author: Verse (Shell of Nine)*  
*Project: OpenFang+SQ Integration*
