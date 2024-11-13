#![feature(generic_const_exprs)]
#![feature(adt_const_params)]
#![allow(incomplete_features)]
#![allow(dead_code)]

#[derive(PartialEq, Eq)]
struct U;

struct S<const N: U>()
//~^ ERROR: `U` must implement `ConstParamTy` to be used as the type of a const generic parameter
where
    S<{ U }>:;

fn main() {}
