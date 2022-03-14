// from https://github.com/Connicpu/dxgi-rs/blob/master/src/helpers/mod.rs
pub struct MemoryDbgHelper(pub u64);

impl std::fmt::Debug for MemoryDbgHelper {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        static LEVELS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB", "EB"];

        let mut amount = self.0 as f64;
        let mut level = 0;
        for _ in 0..LEVELS.len() {
            if amount < 1024.0 {
                break;
            }

            level += 1;
            amount /= 1024.0;
        }

        if level > 0 && amount < 10.0 {
            write!(fmt, "{:.2}{}", amount, LEVELS[level])
        } else if level > 0 && amount < 100.0 {
            write!(fmt, "{:.1}{}", amount, LEVELS[level])
        } else {
            write!(fmt, "{:.0}{}", amount, LEVELS[level])
        }
    }
}

#[test]
fn memory_dbg_helper() {
    assert_eq!(format!("{:?}", MemoryDbgHelper(1024u64.pow(0) * 1)), "1B");
    assert_eq!(format!("{:?}", MemoryDbgHelper(1024u64.pow(0) * 10)), "10B");
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(0) * 100)),
        "100B"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(1) * 1)),
        "1.00KB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(1) * 10)),
        "10.0KB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(1) * 100)),
        "100KB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(2) * 1)),
        "1.00MB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(2) * 10)),
        "10.0MB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(2) * 100)),
        "100MB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(3) * 1)),
        "1.00GB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(3) * 10)),
        "10.0GB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(3) * 100)),
        "100GB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(4) * 1)),
        "1.00TB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(4) * 10)),
        "10.0TB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(4) * 100)),
        "100TB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(5) * 1)),
        "1.00PB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(5) * 10)),
        "10.0PB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(5) * 100)),
        "100PB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(6) * 1)),
        "1.00EB"
    );
    assert_eq!(
        format!("{:?}", MemoryDbgHelper(1024u64.pow(6) * 10)),
        "10.0EB"
    );
    assert_eq!(format!("{:?}", MemoryDbgHelper(std::u64::MAX)), "16.0EB");
}
