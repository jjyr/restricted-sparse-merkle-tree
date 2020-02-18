use crate::H256;

pub trait Hasher {
    fn write_h256(&mut self, h: &H256);
    fn finish(self) -> H256;
}
