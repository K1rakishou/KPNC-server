use sha3::{Digest, Sha3_512};
use sha3::digest::FixedOutput;

pub trait Sha3_512_Hashable<T> {
    fn sha3_512(&self) -> String;
}

impl Sha3_512_Hashable<&str> for &str {
    fn sha3_512(&self) -> String {
        let mut hasher = Sha3_512::new();
        hasher.update(self);
        return format!("{:x}", hasher.finalize_fixed());
    }
}

impl Sha3_512_Hashable<&String> for &String {
    fn sha3_512(&self) -> String {
        let mut hasher = Sha3_512::new();
        hasher.update(self);
        return format!("{:x}", hasher.finalize_fixed());
    }
}