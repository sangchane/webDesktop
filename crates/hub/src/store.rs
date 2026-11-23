//! 저장소 계층 — DO SQLite 쿼리·마이그레이션 (05 「저장소 설계」). 규칙은 service 에.
//! 한 요청 안에서 await 없이 이어 부른 쓰기는 DO 가 한 번에 커밋한다 — transactionSync 대신(SPEC 바닥선 6).
use crate::service::CodeRow;
use serde::Deserialize;
use worker::{SqlStorage, SqlStorageValue as V};

/// (판, SQL) — 순서대로. 적용한 파일은 고치지 않는다.
const MIGRATIONS: &[(i64, &str)] = &[(1, include_str!("../migrations/0001_init.sql"))];

/// 아직 적용하지 않은 마이그레이션만 (두 번째 실행은 0개).
pub fn pending(current: i64) -> impl Iterator<Item = &'static (i64, &'static str)> {
    MIGRATIONS.iter().filter(move |(v, _)| *v > current)
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub os: String,
    pub pubkey: String,
    pub perm: String,
    pub created_at: i64,
    pub last_seen_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

pub struct NewDevice<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub os: &'a str,
    pub pubkey: &'a str,
    pub agent_version: &'a str,
    pub now: i64,
}

pub struct Store(pub SqlStorage);

#[derive(Deserialize)]
struct Version {
    v: i64,
}

const DEVICE_COLS: &str = "id, name, os, pubkey, perm, created_at, last_seen_at, revoked_at";

impl Store {
    /// 적용한 마이그레이션 수를 돌려준다.
    pub fn migrate(&self) -> worker::Result<usize> {
        self.0.exec(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY)",
            None,
        )?;
        let cur = self
            .0
            .exec(
                "SELECT COALESCE(MAX(version), 0) AS v FROM schema_version",
                None,
            )?
            .one::<Version>()?
            .v;
        let mut n = 0;
        for (v, sql) in pending(cur) {
            self.0.exec(sql, None)?;
            self.0.exec(
                "INSERT INTO schema_version (version) VALUES (?)",
                vec![V::Integer(*v)],
            )?;
            n += 1;
        }
        Ok(n)
    }

    pub fn code_get(&self) -> worker::Result<Option<CodeRow>> {
        Ok(self
            .0
            .exec(
                "SELECT code_hash, install_token_hash, expires_at, tries FROM enroll_code WHERE id = 1",
                None,
            )?
            .to_array::<CodeRow>()?
            .pop())
    }

    /// 코드 만들기 = {기존 행 삭제 + 새 행}.
    pub fn code_replace(
        &self,
        code_hash: &str,
        install_hash: &str,
        expires_at: i64,
    ) -> worker::Result<()> {
        self.code_delete()?;
        self.0.exec(
            "INSERT INTO enroll_code (id, code_hash, install_token_hash, expires_at, tries) VALUES (1, ?, ?, ?, 0)",
            vec![code_hash.into(), install_hash.into(), expires_at.into()],
        )?;
        Ok(())
    }

    pub fn code_delete(&self) -> worker::Result<()> {
        self.0.exec("DELETE FROM enroll_code WHERE id = 1", None)?;
        Ok(())
    }

    pub fn code_set_tries(&self, tries: u32) -> worker::Result<()> {
        self.0.exec(
            "UPDATE enroll_code SET tries = ? WHERE id = 1",
            vec![V::Integer(tries.into())],
        )?;
        Ok(())
    }

    pub fn pubkey_taken(&self, pubkey: &str) -> worker::Result<bool> {
        Ok(!self
            .0
            .exec(
                "SELECT id FROM device WHERE pubkey = ?",
                vec![pubkey.into()],
            )?
            .to_array::<serde_json::Value>()?
            .is_empty())
    }

    pub fn device_insert(&self, d: &NewDevice) -> worker::Result<()> {
        self.0.exec(
            "INSERT INTO device (id, name, os, pubkey, agent_version, created_at) VALUES (?, ?, ?, ?, ?, ?)",
            vec![
                d.id.into(),
                d.name.into(),
                d.os.into(),
                d.pubkey.into(),
                d.agent_version.into(),
                d.now.into(),
            ],
        )?;
        Ok(())
    }

    pub fn device_get(&self, id: &str) -> worker::Result<Option<DeviceRow>> {
        Ok(self
            .0
            .exec(
                &format!("SELECT {DEVICE_COLS} FROM device WHERE id = ?"),
                vec![id.into()],
            )?
            .to_array::<DeviceRow>()?
            .pop())
    }

    /// E-02 — 사용 중 기기, 이름 순.
    pub fn devices(&self) -> worker::Result<Vec<DeviceRow>> {
        self.0
            .exec(
                &format!("SELECT {DEVICE_COLS} FROM device WHERE revoked_at IS NULL ORDER BY name"),
                None,
            )?
            .to_array()
    }

    /// last_seen_at 을 앞으로만 옮긴다.
    pub fn device_seen(&self, id: &str, at: i64) -> worker::Result<()> {
        self.0.exec(
            "UPDATE device SET last_seen_at = ? WHERE id = ? AND (last_seen_at IS NULL OR last_seen_at < ?)",
            vec![at.into(), id.into(), at.into()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn second_run_applies_nothing() {
        assert_eq!(super::pending(0).count(), super::MIGRATIONS.len());
        let last = super::MIGRATIONS.last().unwrap().0;
        assert_eq!(super::pending(last).count(), 0);
    }
}
