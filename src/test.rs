//! Contains various structs intended for testing purposes

use rand::{Rng as _, SeedableRng};

/// A unit struct that returns a random ordering when compared
#[derive(Debug, Clone)]
pub struct RandomOrdered(std::rc::Rc<std::cell::RefCell<rand::rngs::SmallRng>>);

impl RandomOrdered {
    /// Create a new array of RandomOrdered, created with a shared [`rand::rngs::SmallRng`]
    pub fn new_array<const SIZE: usize>(seed: u64) -> [Self; SIZE] {
        let rng = std::rc::Rc::new(std::cell::RefCell::new(
            rand::rngs::SmallRng::seed_from_u64(seed),
        ));
        std::array::from_fn(|_| RandomOrdered(rng.clone()))
    }
}

impl PartialEq for RandomOrdered {
    fn eq(&self, _other: &Self) -> bool {
        self.0.borrow_mut().random()
    }
}

impl Eq for RandomOrdered {}

impl PartialOrd for RandomOrdered {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RandomOrdered {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        match rand::random_range(0..3) {
            0 => std::cmp::Ordering::Less,
            1 => std::cmp::Ordering::Equal,
            2 => std::cmp::Ordering::Greater,
            _ => unreachable!(),
        }
    }
}

/// A struct that panicks with the likelihood of `1 / LIKELIHOOD` when compared.
#[derive(Debug, Clone)]
pub struct MaybePanickingOrdered<const LIKELIHOOD: usize, T: Ord>(
    std::rc::Rc<std::cell::RefCell<rand::rngs::SmallRng>>,
    T,
);

impl<const LIKELIHOOD: usize, T: Ord> MaybePanickingOrdered<LIKELIHOOD, T> {
    /// Map an array of `T` to an array of `MaybePanickingOrdered<T>` with a shared
    /// [`rand::rngs::SmallRng`]
    pub fn new_array<const SIZE: usize>(array: [T; SIZE], seed: u64) -> [Self; SIZE] {
        let rng = std::rc::Rc::new(std::cell::RefCell::new(
            rand::rngs::SmallRng::seed_from_u64(seed),
        ));
        array.map(|element| Self(rng.clone(), element))
    }
}

impl<const LIKELIHOOD: usize, T: Ord> PartialEq for MaybePanickingOrdered<LIKELIHOOD, T> {
    fn eq(&self, other: &Self) -> bool {
        match self.0.borrow_mut().random_range(0..LIKELIHOOD) {
            0 => panic!("MaybePanickingOrdered panicked during comparison"),
            _ => self.1.eq(&other.1),
        }
    }
}

impl<const LIKELIHOOD: usize, T: Ord> Eq for MaybePanickingOrdered<LIKELIHOOD, T> {}

impl<const LIKELIHOOD: usize, T: Ord> PartialOrd for MaybePanickingOrdered<LIKELIHOOD, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<const LIKELIHOOD: usize, T: Ord> Ord for MaybePanickingOrdered<LIKELIHOOD, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.0.borrow_mut().random_range(0..LIKELIHOOD) {
            0 => panic!("MaybePanickingOrdered panicked during comparison"),
            _ => self.1.cmp(&other.1),
        }
    }
}
