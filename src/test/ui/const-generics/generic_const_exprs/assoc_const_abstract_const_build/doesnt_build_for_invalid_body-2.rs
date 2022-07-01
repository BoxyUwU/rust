// this shouldnt compile even though the returned value of
// `<() as Trait<N>>::ASSOC` is always `N` which should unify with `N`

#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

trait Trait<const N: usize> {
    const ASSOC: usize;
}

impl<const N: usize> Trait<N> for () {
    const ASSOC: usize = {
        let b = |a: String| todo!();
        N
    };
}

fn foo<const N: usize>() -> [(); <() as Trait<N>>::ASSOC] {
    [(); N]
    //~^ error: mismatched types
}

fn main() {}
