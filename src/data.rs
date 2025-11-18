use std::{fmt, marker::PhantomData};

use rand::{distr::Distribution, rngs::StdRng};

/// A uniform data distribution set
#[derive(Debug)]
pub struct UniformData<T>(PhantomData<T>);

/// A trait for generalizing sorting data creation
pub trait Data<T: Sized + Ord + fmt::Debug> {
    /// Initialize a vector of the given size
    fn initialize(size: usize, rng: &mut StdRng) -> Vec<T>;
}

macro_rules! impl_for_integers {
    ($($type:ty),*) => {
        $(
            impl_for_integers!(@single $type);
        )*
    };
    (@single $type:ty) => {
        impl Data<$type> for UniformData<$type> {
            fn initialize(size: usize, rng: &mut StdRng) -> Vec<$type> {
                rand::distr::Uniform::new(<$type>::MIN, <$type>::MAX)
                    .unwrap()
                    .sample_iter(rng)
                    .take(size)
                    .collect()
            }
        }
    }
}

// Implement the Data trait for the default integer types
impl_for_integers!(u8, u16, u32, u64, u128);
