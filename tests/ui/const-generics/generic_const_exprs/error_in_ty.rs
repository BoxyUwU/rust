//@ compile-flags: -Znext-solver=coherence

#![feature(generic_const_exprs)]
#![allow(incomplete_features)]

pub struct A<const z: [usize; x]> {}
//~^ ERROR: cannot find value `x` in this scope

impl A<2> {
    pub const fn B() {}
    //~^ ERROR: duplicate definitions
}

impl A<2> {
    pub const fn B() {}
}

fn main() {}
