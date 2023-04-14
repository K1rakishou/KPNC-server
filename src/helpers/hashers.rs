use sha3::{Digest, Sha3_512};
use sha3::digest::FixedOutput;

pub trait Sha512Hashable<T> {
    fn sha3_512(&self, iterations: usize) -> String;
}

impl Sha512Hashable<&str> for &str {
    fn sha3_512(&self, iterations: usize) -> String {
        return sha3_512_internal(self, iterations);
    }
}

impl Sha512Hashable<&String> for &String {
    fn sha3_512(&self, iterations: usize) -> String {
        return sha3_512_internal(self.as_str(), iterations);
    }
}

fn sha3_512_internal(str: &str, iterations: usize) -> String {
    let mut hash = String::from(str);

    for _ in 0..iterations {
        let mut hasher = Sha3_512::new();
        hasher.update(hash);
        hash = format!("{:x}", hasher.finalize_fixed());
    }

    return hash;
}