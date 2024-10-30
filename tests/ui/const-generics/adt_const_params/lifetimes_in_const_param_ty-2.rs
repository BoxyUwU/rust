#![allow(incomplete_features)]
#![feature(adt_const_params, generic_const_exprs)]
#![allow(dead_code)]

// Regression test for #125564 where we would wind up having erased lifetimes in type system
// constants in MIR without them ever being replaced with region variables during mir typeck.

const fn catone<const M: usize>(_a: &[u8; M]) -> [u8; M + 1]
where
    [(); M + 1]:,
{
    unimplemented!()
}

struct Catter<const A: &'static [u8]>;
//~^ ERROR: `&'static [u8]` can't be used as a const parameter type
impl<const A: &'static [u8]> Catter<A>
//~^ ERROR: `&'static [u8]` can't be used as a const parameter type
where
    [(); A.len() + 1]:,
{
    const ZEROS: &'static [u8; A.len()] = &[0_u8; A.len()];
    const R: &'static [u8] = &catone(Self::ZEROS);
}

fn main() {}
