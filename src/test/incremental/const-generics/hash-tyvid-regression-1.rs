// revisions: cfail
#![feature(const_generics, generic_const_exprs)]
#![allow(incomplete_features)]
// regression test for #77650
fn c<T, const N: std::num::NonZeroUsize>()
where
    [T; N.get()]: Sized,
{
    use std::convert::TryFrom;
    <[T; N.get()]>::try_from(())
    //~^ error: the trait bound
    //~^^ error: mismatched types
}

fn main() {}
