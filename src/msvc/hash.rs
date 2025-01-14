use std::hash::{ Hash, Hasher };

const FNV_OFFSET_BASIS: u64 = 0xCBF29CE484222325;
const FNV_PRIME: u64 = 0x100000001b3;

pub struct FNV1A(u64);

pub trait HasherInit {
    fn new() -> Self where Self: Sized;
    fn get_hash<H>(value: &H) -> u64 where H: Hash;
}

impl HasherInit for FNV1A {
    fn new() -> Self { Self(FNV_OFFSET_BASIS) }
    fn get_hash<H>(value: &H) -> u64 where H: Hash {
        let mut fnv1a = Self::new();
        value.hash(&mut fnv1a);
        fnv1a.finish()
    }
}

impl Hasher for FNV1A {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).overflowing_mul(FNV_PRIME).0
        }
    }
    fn finish(&self) -> u64 { self.0 }
}

#[cfg(test)]
pub mod tests {
    use crate::msvc::string::String;
    use super::{ HasherInit, FNV1A };
    use std::{
        error::Error,
        hash::{ Hash, Hasher }
    };

    type TestReturn = Result<(), Box<dyn Error>>;

    fn check_hash(s: &str, expected: u64) {
        let cpp_str = String::from_str(s);
        let mut hasher = FNV1A::new();
        cpp_str.hash(&mut hasher);
        let res = hasher.finish();
        assert_eq!(res, expected, "Hash values do not match for {}, should be {} instead of {}", s, expected, res);
    }

    #[test]
    pub fn hash_cpp_string() -> TestReturn { 
        check_hash("Player", 0x333DC56DDFFD8EA0);
        check_hash("Enemy0", 0xE24F0CA51E957E61);
        check_hash("Enemy1", 0xE24F0BA51E957CAE);
        check_hash("Enemy2", 0xE24F0AA51E957AFB);
        check_hash("Enemy3", 0xE24F09A51E957948);
        check_hash("Enemy4", 0xE24F10A51E95852D);
        check_hash("Chest", 0x4295BDDCA90BEC76);
        check_hash("Door", 0x37CF773608CE6C9);
        check_hash("Door2", 0x7A3F740D0F6C7C81);
        Ok(())
    }
}
