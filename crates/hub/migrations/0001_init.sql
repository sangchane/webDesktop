-- 0001 — 기기·등록 코드·세션 (05 ERD). 적용한 뒤에는 고치지 않는다. VPN·업데이트 표는 뒤 마이그레이션.
-- 키·해시는 TEXT(base64·hex)로 둔다 — DO SQL 의 BLOB 왕복(ArrayBuffer ↔ serde)을 피함.
CREATE TABLE device (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  os TEXT NOT NULL CHECK (os IN ('windows', 'macos')),
  pubkey TEXT NOT NULL UNIQUE,
  agent_version TEXT NOT NULL,
  perm TEXT NOT NULL DEFAULT 'ok' CHECK (perm IN ('ok', 'need_screen', 'need_access', 'need_localnet', 'need_multi')),
  created_at INTEGER NOT NULL,
  last_seen_at INTEGER,
  revoked_at INTEGER
);
CREATE TABLE enroll_code (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  code_hash TEXT NOT NULL,
  install_token_hash TEXT NOT NULL,
  expires_at INTEGER NOT NULL,
  tries INTEGER NOT NULL DEFAULT 0 CHECK (tries BETWEEN 0 AND 5)
);
CREATE TABLE session (
  id TEXT PRIMARY KEY,
  device_id TEXT NOT NULL REFERENCES device (id),
  viewer_id TEXT NOT NULL,
  viewer_ua TEXT,
  path TEXT CHECK (path IN ('direct', 'relay')),
  click_to_first_frame_ms INTEGER,
  latency_p50_ms INTEGER,
  reconnect_count INTEGER NOT NULL DEFAULT 0,
  reconnect_ms_max INTEGER,
  reconnect_ms_json TEXT,
  restarts INTEGER NOT NULL DEFAULT 0,
  new_session_used INTEGER NOT NULL DEFAULT 0 CHECK (new_session_used IN (0, 1)),
  encoder TEXT CHECK (encoder IN ('hw', 'sw')),
  end_reason TEXT,
  started_at INTEGER NOT NULL,
  ended_at INTEGER
);
CREATE INDEX idx_session_open ON session (device_id, ended_at);
CREATE INDEX idx_session_started ON session (started_at, id);
