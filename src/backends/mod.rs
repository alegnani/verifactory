//! Back-end used to convert the IR into a z3 model
mod proofs;
mod z3;

pub use self::proofs::{Printable, Z3Proofs};
pub use self::z3::Z3Backend;
