use rusqlite::{Connection, OpenFlags};

const KEY: &str = "x'00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff'";
const BAD: &str = "x'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff'";

fn main() {
    let mut fails = 0;
    let mut check = |name: &str, ok: bool, detail: String| {
        println!("{} {} {}", if ok { "PASS" } else { "FAIL" }, name, detail);
        if !ok { fails += 1; }
    };

    let c = Connection::open_with_flags("fixtures/synthetic-encrypted.db", OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    c.pragma_update(None, "key", KEY).unwrap();
    let n: rusqlite::Result<i64> = c.query_row("SELECT count(*) FROM firearms", [], |r| r.get(0));
    check("open DB encrypted by the Electron library", n.as_ref().ok() == Some(&2), format!("{:?}", n));
    let ver: String = c.query_row("PRAGMA cipher_version", [], |r| r.get(0)).unwrap_or_default();
    check("cipher_version present", !ver.is_empty(), ver);

    let bad = Connection::open_with_flags("fixtures/synthetic-encrypted.db", OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    bad.pragma_update(None, "key", BAD).unwrap();
    let r: rusqlite::Result<i64> = bad.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0));
    check("wrong key rejected", r.is_err(), String::new());

    let path = std::env::temp_dir().join("s0-new.db");
    let _ = std::fs::remove_file(&path);
    {
        let w = Connection::open(&path).unwrap();
        w.pragma_update(None, "key", KEY).unwrap();
        w.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1),(2),(3);").unwrap();
    }
    let head = std::fs::read(&path).unwrap();
    check("new file is encrypted", &head[..15] != b"SQLite format 3", String::new());
    let r2 = Connection::open(&path).unwrap();
    r2.pragma_update(None, "key", KEY).unwrap();
    let n2: i64 = r2.query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
    check("new file reopens with key", n2 == 3, n2.to_string());

    println!("{}", if fails == 0 { "S0-WIN-SQLCIPHER PASS" } else { "S0-WIN-SQLCIPHER FAIL" });
    std::process::exit(if fails == 0 { 0 } else { 1 });
}
