#![feature(adt_const_params, generic_const_exprs)]
#![allow(incomplete_features)]

// Regression test for #125564 where we would wind up having erased lifetimes in type system
// constants in MIR without them ever being replaced with region variables during mir typeck.

const fn concat_strs() -> &'static str {
    //~^ ERROR: mismatched types
    const fn concat_arr<const M: usize, const N: usize>(a: [u8; M], b: [u8; N]) -> [u8; M + N] {}
    //~^ ERROR: mismatched types

    impl<const A: &'static str, const B: &'static str> Inner<A, B>
    //~^ ERROR: cannot find type `Inner` in this scope
    //~| ERROR: `&'static str` can't be used as a const parameter type
    //~| ERROR: `&'static str` can't be used as a const parameter type
    where
        [(); A.len()]:,
        [(); B.len()]:,
        [(); A.len() + B.len()]:,
    {
        const ABSTR: &'static str = unsafe {
            std::str::from_utf8_unchecked(&concat_arr(
                A.as_ptr().cast().read(),
                //~^ WARN: type annotations needed
                //~| WARN: this is accepted in the current edition (Rust 2015) but is a hard error in Rust 2018!
                B.as_ptr().cast().read(),
                //~^ WARN: type annotations needed
                //~| WARN: this is accepted in the current edition (Rust 2015) but is a hard error in Rust 2018!
            ))
        };
    }
}

fn main() {}
