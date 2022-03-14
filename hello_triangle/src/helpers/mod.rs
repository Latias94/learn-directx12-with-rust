mod memory_dbg_helper;
pub use memory_dbg_helper::*;

pub fn wstrlens(pwstr: &[u16]) -> usize {
    let mut len = 0;
    for &c in pwstr {
        if c == 0 {
            break;
        }
        len += 1;
    }
    len
}


