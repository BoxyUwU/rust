// FIXME(generic_const_exprs) this should work but doesnt because `N` doesnt
// get turned into an abstract const wheras the anon const for `<() as Trait<N>>::ASSOC` does

#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

trait Trait<const N: usize> {
    const ASSOC: usize;
}

impl<const N: usize> Trait<N> for () {
    const ASSOC: usize = N;
}

fn foo<const N: usize>() -> [(); <() as Trait<N>>::ASSOC] {
    [(); N]
    //~^ error: unconstrained generic constant
}

fn main() {}
