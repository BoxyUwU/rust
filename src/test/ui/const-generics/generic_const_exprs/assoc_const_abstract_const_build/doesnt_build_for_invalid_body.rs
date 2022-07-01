// check-pass

// this should compile even though the body of `<() as Trait<N>>::ASSOC`
// cannot be lowered to an abstract const.

#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

trait Trait<const N: usize> {
    const ASSOC: usize;
}

impl<const N: usize> Trait<N> for () {
    const ASSOC: usize = {
        let a = |a: String| todo!();
        let mut b = 10;
        b = 13;
        N
    };
}

fn foo<const N: usize>() -> [(); <() as Trait<N>>::ASSOC] {
    [(); <() as Trait<N>>::ASSOC]
}

fn main() {}
