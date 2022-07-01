#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

trait Trait<const N: usize> {
    const ASSOC: usize = N + 1;
}

fn foo<const N: usize, T: Trait<N>>() -> [(); <T as Trait<N>>::ASSOC] {
    [(); N + 1]
    //~^ error: mismatched types
    //~^^ error: unconstrained generic constant
}

fn main() {}
